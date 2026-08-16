//! The compile pipeline: parse -> analyze -> codegen.
//!
//! This module owns "compile a project". Every front end (the CLI's `compile`
//! command, the MCP `compile` tool, the browser playground) calls
//! [`compile`] rather than composing the stages itself, so the order of the
//! stages, the rule for when codegen may run, and the shape of the resulting
//! diagnostics have exactly one definition.
//!
//! What a front end owns is what happens on either side of this function:
//! where the sources came from, and what to do with the container -- exit
//! codes and a file on disk, a JSON response and a cache handle, or a base64
//! string handed back to JavaScript.

use ironplc_codegen::{CodegenOptions, SourceLookup};
use ironplc_container::Container;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_parser::options::CompilerOptions;
use log::debug;

use crate::project::Project;

/// What the compile pipeline produced.
pub struct CompileOutput {
    /// Every diagnostic the pipeline collected, in stage order: the
    /// caller-supplied ones first, then parsing and analysis, then codegen.
    ///
    /// Empty means the project is clean.
    pub diagnostics: Vec<Diagnostic>,

    /// The generated container.
    ///
    /// `Some` only when no stage reported a problem, so a caller can treat
    /// this as "there is something worth writing out". A failing compile never
    /// yields a container, because a failing command must not leave behind a
    /// deployable artifact.
    pub container: Option<Container>,
}

/// Runs the full compile pipeline -- parse, semantic analysis, codegen -- over
/// `project`.
///
/// `diagnostics` seeds the collection with problems the caller found before
/// the pipeline ran (the CLI's project-discovery and output-path problems).
/// Seeding rather than merging afterwards is what makes those problems block
/// codegen while still letting analysis run: a discovery-time problem must not
/// hide a real bug in the files that did resolve, but it must still prevent a
/// container from being produced.
///
/// Analysis always runs. Codegen runs only when nothing at all -- seeded,
/// parse, or analysis -- reported a problem.
///
/// `source_lookup` hands codegen the exact bytes the parser saw for each file,
/// which the container's debug section hashes so a debugger can detect drift.
/// Callers with no source bytes to offer pass
/// [`EmptyLookup`](ironplc_codegen::EmptyLookup).
///
/// The analyzed library and semantic context stay cached on `project`, so a
/// caller that needs them (for task/program metadata, or to build a symbol
/// map) reads them back through [`Project::analyzed_library`] and
/// [`Project::semantic_context`] after this returns.
pub fn compile(
    project: &mut dyn Project,
    compiler_options: &CompilerOptions,
    source_lookup: &dyn SourceLookup,
    mut diagnostics: Vec<Diagnostic>,
) -> CompileOutput {
    // Parse and analyze. Analysis goes as far as it can and reports everything
    // it found; it never short-circuits on the seeded diagnostics.
    diagnostics.extend(project.semantic());

    if !diagnostics.is_empty() {
        debug!("Skipping codegen, {} problem(s) found", diagnostics.len());
        return CompileOutput {
            diagnostics,
            container: None,
        };
    }

    let (Some(library), Some(context)) = (project.analyzed_library(), project.semantic_context())
    else {
        // A clean analysis always caches its artifacts, so this is a compiler
        // defect rather than a problem with the input.
        diagnostics.push(Diagnostic::internal_error(file!(), line!()));
        return CompileOutput {
            diagnostics,
            container: None,
        };
    };

    // Generate bytecode, skipping user-defined functions not reachable from
    // the PROGRAM root to reduce container size.
    let codegen_options = CodegenOptions {
        system_uptime_global: compiler_options.allow_system_uptime_global,
    };

    match ironplc_codegen::compile(library, context, &codegen_options, source_lookup) {
        Ok(container) => CompileOutput {
            diagnostics,
            container: Some(container),
        },
        Err(err) => {
            diagnostics.push(err);
            CompileOutput {
                diagnostics,
                container: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ironplc_codegen::EmptyLookup;
    use ironplc_dsl::core::{FileId, SourceSpan};
    use ironplc_dsl::diagnostic::{Diagnostic, Label};
    use ironplc_parser::options::CompilerOptions;
    use ironplc_problems::Problem;

    use super::compile;
    use crate::project::{MemoryBackedProject, Project};

    /// Stands in for a problem the caller found before the pipeline ran, the
    /// way project discovery does for the CLI.
    fn seeded_diagnostic() -> Vec<Diagnostic> {
        vec![Diagnostic::problem(
            Problem::CannotReadFile,
            Label::span(SourceSpan::default(), "discovery problem"),
        )]
    }

    const VALID_PROGRAM: &str = r#"
PROGRAM Main
VAR
  x : INT;
END_VAR
  x := 1;
END_PROGRAM
"#;

    fn project_with(content: &str) -> MemoryBackedProject {
        let mut project = MemoryBackedProject::new(CompilerOptions::default());
        project.add_source(FileId::from_string("main.st"), content.to_owned());
        project
    }

    #[test]
    fn compile_when_valid_program_then_container_and_no_diagnostics() {
        let mut project = project_with(VALID_PROGRAM);
        let output = compile(
            &mut project,
            &CompilerOptions::default(),
            &EmptyLookup,
            vec![],
        );

        assert!(
            output.diagnostics.is_empty(),
            "expected a clean compile, got: {:?}",
            output.diagnostics
        );
        assert!(output.container.is_some());
    }

    #[test]
    fn compile_when_syntax_error_then_no_container() {
        let mut project = project_with("PROGRAM END_PROGRAM");
        let output = compile(
            &mut project,
            &CompilerOptions::default(),
            &EmptyLookup,
            vec![],
        );

        assert!(!output.diagnostics.is_empty());
        assert!(output.container.is_none());
    }

    #[test]
    fn compile_when_semantic_error_then_no_container() {
        let mut project =
            project_with("PROGRAM Main VAR x : INT; END_VAR x := undeclared_var; END_PROGRAM");
        let output = compile(
            &mut project,
            &CompilerOptions::default(),
            &EmptyLookup,
            vec![],
        );

        assert!(!output.diagnostics.is_empty());
        assert!(output.container.is_none());
    }

    /// A problem the caller found before the pipeline ran suppresses the
    /// container even though the project itself analyzes cleanly. This is the
    /// contract the CLI's `compile_when_plcproj_references_unbundled_library_
    /// then_error_and_no_container` depends on.
    #[test]
    fn compile_when_seeded_diagnostic_then_no_container() {
        let mut project = project_with(VALID_PROGRAM);

        let output = compile(
            &mut project,
            &CompilerOptions::default(),
            &EmptyLookup,
            seeded_diagnostic(),
        );

        assert!(output.container.is_none());
        assert_eq!(output.diagnostics.len(), 1);
    }

    /// Analysis still runs against the files that did resolve, so a real
    /// problem in them is reported alongside the seeded one rather than being
    /// hidden behind it (#1360).
    #[test]
    fn compile_when_seeded_diagnostic_then_analysis_still_runs() {
        let mut project =
            project_with("PROGRAM Main VAR x : INT; END_VAR x := undeclared_var; END_PROGRAM");

        let output = compile(
            &mut project,
            &CompilerOptions::default(),
            &EmptyLookup,
            seeded_diagnostic(),
        );

        assert!(
            output.diagnostics.len() >= 2,
            "expected the seeded problem AND the semantic problem, got: {:?}",
            output.diagnostics
        );
    }

    #[test]
    fn compile_when_clean_then_analyzed_artifacts_remain_available() {
        let mut project = project_with(VALID_PROGRAM);
        let output = compile(
            &mut project,
            &CompilerOptions::default(),
            &EmptyLookup,
            vec![],
        );

        assert!(output.container.is_some());
        // Callers read metadata back off the project after compiling.
        assert!(project.analyzed_library().is_some());
        assert!(project.semantic_context().is_some());
    }

    // The "clean analysis cached no artifacts" arm has no test: it guards an
    // invariant the `Project` implementations cannot break, so reaching it
    // means a compiler defect. Proving it with a stub `Project` costs more
    // uncovered scaffolding than the five lines it would cover.
}
