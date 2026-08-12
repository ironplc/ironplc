//! Implements the command line behavior.

use codespan_reporting::{
    diagnostic::{Diagnostic as CodeSpanDiagnostic, Label as CodeSpanLabel, LabelStyle, Severity},
    files::SimpleFiles,
    term::{
        self,
        termcolor::{ColorChoice, StandardStream},
    },
};
use ironplc_dsl::{
    core::FileId,
    diagnostic::{Diagnostic, Label},
};
use ironplc_plc2plc::write_to_string;
use ironplc_problems::Problem;
use log::{error, trace};
use std::{
    collections::{HashMap, HashSet},
    fs::{canonicalize, metadata},
    ops::Range,
    path::{Path, PathBuf},
};

use ironplc_dsl::common::Library;
use ironplc_parser::options::CompilerOptions;
use ironplc_sources::LibraryName;

use ironplc_project::tokenizer;
use ironplc_project::{FileBackedProject, Project};

// Checks specified files.
pub fn check(
    paths: &[PathBuf],
    compiler_options: CompilerOptions,
    libraries: &[LibraryName],
    suppress_output: bool,
) -> Result<(), String> {
    let (mut project, had_discovery_error) =
        create_project(paths, compiler_options, libraries, suppress_output);

    // Analyze the set
    if let Err(err) = project.semantic() {
        trace!("Errors {err:?}");
        handle_diagnostics(&err, Some(&project), suppress_output);
        return Err(String::from("Error during analysis"));
    }

    if had_discovery_error {
        return Err(String::from("Error enumerating or reading source files"));
    }

    Ok(())
}

pub fn echo(
    paths: &[PathBuf],
    compiler_options: CompilerOptions,
    suppress_output: bool,
) -> Result<(), String> {
    // Echo renders parsed source; it runs no analysis, so no library activation.
    let (mut project, had_discovery_error) =
        create_project(paths, compiler_options, &[], suppress_output);

    // Collect the results and output after because getting the results may change
    // the project itself
    let mut results = vec![];
    for src in project.sources_mut() {
        results.push(src.library());
    }

    let mut has_error = false;

    for result in results {
        match result {
            Ok(library) => {
                let output = write_to_string(library).map_err(|e| {
                    handle_diagnostics(&e, None, suppress_output);
                    String::from("Error echo source")
                })?;

                print!("{output}");
            }
            Err(diagnostics) => {
                let diagnostics: Vec<Diagnostic> = diagnostics;
                // TODO this needs to be improved but will wait for changes to source
                handle_diagnostics(&diagnostics, None, suppress_output);

                print!("Syntax error");

                has_error = true;
            }
        }
    }

    match (has_error, had_discovery_error) {
        (true, _) => Err("Tokenize error".to_owned()),
        (false, true) => Err(String::from("Error enumerating or reading source files")),
        (false, false) => Ok(()),
    }
}

pub fn tokenize(
    paths: &[PathBuf],
    compiler_options: CompilerOptions,
    suppress_output: bool,
) -> Result<(), String> {
    // Tokenize only lexes each source; library activation is irrelevant here.
    let (project, had_discovery_error) =
        create_project(paths, compiler_options, &[], suppress_output);

    for src in project.sources() {
        tokenizer::tokenize_source(src, &project, suppress_output, &handle_diagnostics)?;
    }

    if had_discovery_error {
        return Err(String::from("Error enumerating or reading source files"));
    }

    Ok(())
}

/// Compiles source files into a bytecode container (.iplc) file.
///
/// Parses the source files, runs full analysis (type resolution + semantic
/// checks), and generates bytecode.
pub fn compile(
    paths: &[PathBuf],
    output: &Path,
    compiler_options: CompilerOptions,
    libraries: &[LibraryName],
    suppress_output: bool,
) -> Result<(), String> {
    let (mut project, had_discovery_error) =
        create_project(paths, compiler_options, libraries, suppress_output);

    // Refuse to write the container over a loaded source file. `File::create`
    // truncates immediately, so this must run before any output is opened to
    // avoid replacing a source with container bytes.
    if output_conflicts_with_source(&project, output) {
        let diagnostics = diagnostic(
            Problem::OutputPathConflictsWithInput,
            output,
            String::from("Choose an output path that is not an input source file"),
        );
        handle_diagnostics(&diagnostics, Some(&project), suppress_output);
        return Err(String::from("Output path conflicts with an input file"));
    }

    // Parse all sources and merge into a single library
    let mut combined = Library::new();
    for src in project.sources_mut() {
        match src.library() {
            Ok(library) => {
                combined.elements.extend(library.elements.iter().cloned());
            }
            Err(diagnostics) => {
                handle_diagnostics(&diagnostics, None, suppress_output);
                return Err(String::from("Error parsing source files"));
            }
        }
    }

    // Load any activated compatibility libraries and inject them ahead of user
    // source (base stdlib -> library -> user), so their declarations resolve
    // under their exact vendor names. A library that fails to load (unshipped
    // name or malformed manifest) is a hard error for compilation.
    let (compat_libraries, compat_diagnostics) = project.load_activated_libraries();
    if !compat_diagnostics.is_empty() {
        handle_diagnostics(&compat_diagnostics, Some(&project), suppress_output);
        return Err(String::from("Error activating compatibility libraries"));
    }
    let analyze_input: Vec<&Library> = compat_libraries
        .iter()
        .chain(std::iter::once(&combined))
        .collect();

    // Run full analysis: type resolution + semantic checks (e.g. undeclared
    // function calls, type mismatches). This must happen before codegen so
    // that semantic errors are reported with proper problem codes.
    let (analyzed, context) = ironplc_analyzer::stages::analyze(&analyze_input, &compiler_options)
        .map_err(|errs| {
            handle_diagnostics(&errs, Some(&project), suppress_output);
            String::from("Error during analysis")
        })?;

    // Report semantic diagnostics before attempting codegen. Without this
    // check, semantic errors (e.g. P4017 undeclared function) would surface
    // as misleading codegen errors (e.g. P9999).
    if context.has_diagnostics() {
        handle_diagnostics(context.diagnostics(), Some(&project), suppress_output);
        return Err(String::from("Error during analysis"));
    }

    // Generate bytecode, skipping user-defined functions not reachable from
    // the PROGRAM root to reduce container size.
    let codegen_options = ironplc_codegen::CodegenOptions {
        system_uptime_global: compiler_options.allow_system_uptime_global,
    };
    // Build a SourceLookup that hands codegen the exact bytes the
    // parser saw for each FileId. The container's debug section
    // SOURCE_FILE_TABLE (tag 6) records a BLAKE3 hash over these so a
    // debugger can detect drift between the .iplc and the user's
    // working copy.
    let mut source_bytes: std::collections::HashMap<ironplc_dsl::core::FileId, Vec<u8>> =
        std::collections::HashMap::new();
    for src in project.sources() {
        source_bytes.insert(src.file_id().clone(), src.as_string().as_bytes().to_vec());
    }
    let source_lookup = HashMapSourceLookup(source_bytes);

    let container = ironplc_codegen::compile(&analyzed, &context, &codegen_options, &source_lookup)
        .map_err(|err| {
            handle_diagnostics(&[err], Some(&project), suppress_output);
            String::from("Error during code generation")
        })?;

    // Write the container to the output file
    let mut out_file =
        std::fs::File::create(output).map_err(|e| format!("Failed to create output file: {e}"))?;
    container
        .write_to(&mut out_file)
        .map_err(|e| format!("Failed to write output file: {e}"))?;

    if had_discovery_error {
        return Err(String::from("Error enumerating or reading source files"));
    }

    Ok(())
}

/// Codegen [`SourceLookup`](ironplc_codegen::SourceLookup) backed by an
/// in-memory map populated from the project's loaded sources. The map
/// owns the bytes so the lookup can outlive any borrow on the project.
struct HashMapSourceLookup(std::collections::HashMap<ironplc_dsl::core::FileId, Vec<u8>>);

impl ironplc_codegen::SourceLookup for HashMapSourceLookup {
    fn source_bytes(&self, file_id: &ironplc_dsl::core::FileId) -> Option<&[u8]> {
        self.0.get(file_id).map(Vec::as_slice)
    }
}

/// Builds a project from `paths`, running discovery and loading every
/// resolvable file.
///
/// The returned `bool` is `true` when discovery produced any diagnostic
/// (an unresolvable `.plcproj` entry, a referenced-but-unbundled
/// compatibility library, or an unreadable source file) -- these are
/// already printed via [`handle_diagnostics`] before this returns. The
/// project itself still contains every file that DID resolve: callers
/// must still run their normal work (analysis, codegen, ...) against it
/// so real diagnostics in the resolvable files are not hidden behind a
/// discovery-time problem, then fold the returned `bool` into their own
/// final result so the overall command still fails when it's `true`.
fn create_project(
    paths: &[PathBuf],
    compiler_options: CompilerOptions,
    libraries: &[LibraryName],
    suppress_output: bool,
) -> (FileBackedProject, bool) {
    trace!("Reading paths {paths:?}");
    let mut files: Vec<PathBuf> = vec![];
    // Explicit `--library` activation, plus any libraries discovered project
    // files reference. The explicit set is applied first so it takes
    // precedence in ordering; discovered libraries are appended, deduplicated.
    let mut activated_libraries: Vec<LibraryName> = libraries.to_vec();
    let mut had_error = false;

    for path in paths {
        let (mut resolved, discovered_libraries, diagnostics) = enumerate_files(path);
        files.append(&mut resolved);
        for library in discovered_libraries {
            if !activated_libraries.contains(&library) {
                activated_libraries.push(library);
            }
        }
        if !diagnostics.is_empty() {
            handle_diagnostics(&diagnostics, None, suppress_output);
            had_error = true;
        }
    }

    // Create the project
    let mut project = FileBackedProject::with_options(compiler_options);
    project.set_activated_libraries(activated_libraries);
    let mut errors: Vec<Diagnostic> = vec![];

    for file_path in files {
        let res = project.push(FileId::from_path(&file_path));
        match res {
            Ok(_) => {}
            Err(err) => {
                errors.push(err);
            }
        }
    }

    if !errors.is_empty() {
        handle_diagnostics(&errors, Some(&project), suppress_output);
        had_error = true;
    }

    (project, had_error)
}

/// Enumerates all files at the path.
///
/// If the path is a file, then returns the file. If the path is a directory,
/// then uses project discovery to detect the project structure and return
/// the appropriate set of files.
///
/// Discovery problems that shouldn't stop the rest of the project from
/// being enumerated (e.g. a `.plcproj` `<Compile Include="...">` entry
/// that doesn't resolve to a real file) are returned alongside whatever
/// files DID resolve, rather than aborting enumeration entirely -- but
/// they are still genuine errors: the caller must still fail the overall
/// command if this returns any diagnostics.
fn enumerate_files(path: &PathBuf) -> (Vec<PathBuf>, Vec<LibraryName>, Vec<Diagnostic>) {
    // Get the canonical path so that error messages are unambiguous
    let path = match canonicalize(path) {
        Ok(path) => path,
        Err(e) => {
            return (
                vec![],
                vec![],
                diagnostic(
                    Problem::CannotCanonicalizePath,
                    path,
                    format!("{}, {}", path.display(), e),
                ),
            );
        }
    };

    // Determine what kind of path we have.
    let metadata = match metadata(&path) {
        Ok(metadata) => metadata,
        Err(e) => {
            return (
                vec![],
                vec![],
                diagnostic(Problem::CannotReadMetadata, &path, e.to_string()),
            );
        }
    };
    if metadata.is_dir() {
        return match ironplc_sources::discovery::discover(&path) {
            Ok(project) => {
                // Auto-activate the libraries a discovered project file
                // references, alongside any files it declares. Referenced but
                // unshipped libraries contribute a diagnostic naming them.
                let (libraries, library_diagnostics) =
                    ironplc_sources::libraries::LibraryRegistry::bundled()
                        .resolve_references(&project.library_references);
                let mut diagnostics = project.errors;
                diagnostics.extend(library_diagnostics);
                (project.files, libraries, diagnostics)
            }
            Err(e) => (vec![], vec![], vec![e]),
        };
    }
    if metadata.is_file() {
        return (vec![path.to_path_buf()], vec![], vec![]);
    }
    if metadata.is_symlink() {
        return (
            vec![],
            vec![],
            diagnostic(Problem::SymlinkUnsupported, &path, String::from("")),
        );
    }
    (vec![], vec![], vec![])
}

/// Converts an IronPLC diagnostic into the
fn handle_diagnostics(
    diagnostics: &[Diagnostic],
    project: Option<&FileBackedProject>,
    suppress_output: bool,
) {
    if !suppress_output {
        let writer = StandardStream::stderr(ColorChoice::Always);
        let config = codespan_reporting::term::Config::default();

        let mut files: SimpleFiles<String, &str> = SimpleFiles::new();

        let mut unique_files: HashSet<&FileId> = HashSet::new();
        for diagnostic in diagnostics {
            for file_id in diagnostic.file_ids() {
                unique_files.insert(file_id);
            }
        }

        let mut files_to_ids: HashMap<&FileId, usize> = HashMap::new();
        let empty_source = &"".to_owned();
        match project {
            Some(set) => {
                for file_id in unique_files {
                    if let Some(content) = set.get(file_id) {
                        let id = files.add(file_id.to_string(), content.as_string());
                        files_to_ids.insert(file_id, id);
                    } else {
                        let id = files.add(file_id.to_string(), empty_source);
                        files_to_ids.insert(file_id, id);
                    }
                }
            }
            None => {
                for file_id in unique_files {
                    let id = files.add(file_id.to_string(), empty_source);
                    files_to_ids.insert(file_id, id);
                }
            }
        }

        diagnostics.iter().for_each(|d| {
            let diagnostic = map_diagnostic(d, &files_to_ids);

            let _ = term::emit_to_write_style(&mut writer.lock(), &config, &files, &diagnostic)
                .map_err(|err| {
                    error!("Failed writing to terminal: {err}");
                    1usize
                });
        });
    }
}

/// Builds the documentation URL for a diagnostic, tagged with the channel it
/// was surfaced through.
///
/// The URL is a working docs link regardless; `channel=cli` marks the origin
/// and `version` carries the client version (which the out-of-date banner in
/// docs/_static/version-check.js reads), so we can also see where and on which
/// version people reach these pages. `file`/`line` (the Rust source location
/// that raised the diagnostic) are appended when present so a maintainer can see
/// what a remote user hit.
fn problem_help_url(diagnostic: &Diagnostic) -> String {
    let version = env!("CARGO_PKG_VERSION");
    let mut url = format!(
        "https://www.ironplc.com/reference/{section}/problems/{code}.html?version={version}&channel=cli",
        section = ironplc_dsl::diagnostic::docs_section(&diagnostic.code),
        code = diagnostic.code,
    );
    if let Some(ref file) = diagnostic.source_file {
        url.push_str(&format!("&file={file}"));
    }
    if let Some(line) = diagnostic.source_line {
        url.push_str(&format!("&line={line}"));
    }
    url
}

fn map_diagnostic(
    diagnostic: &Diagnostic,
    file_to_id: &HashMap<&FileId, usize>,
) -> CodeSpanDiagnostic<usize> {
    let description = diagnostic.description();

    // Set the primary labels
    let mut labels = vec![map_label(
        &diagnostic.primary,
        LabelStyle::Primary,
        file_to_id,
    )];

    // Add any secondary labels
    labels.extend(
        diagnostic
            .secondary
            .iter()
            .map(|lbl| map_label(lbl, LabelStyle::Secondary, file_to_id)),
    );

    // Existing help notes, then a trailing link to the problem-code docs so a
    // CLI user can follow the same reference page the editor and playground link
    // to (and so that follow-through is attributable to the CLI in analytics).
    let mut notes = diagnostic.help().to_vec();
    notes.push(format!("Learn more: {}", problem_help_url(diagnostic)));

    CodeSpanDiagnostic::new(Severity::Error)
        .with_code(diagnostic.code.clone())
        .with_message(description)
        .with_labels(labels)
        .with_notes(notes)
}

fn map_label(
    label: &Label,
    style: LabelStyle,
    file_to_id: &HashMap<&FileId, usize>,
) -> CodeSpanLabel<usize> {
    let range = Range {
        start: label.location.start,
        end: label.location.end,
    };
    let id = file_to_id.get(&label.file_id);
    CodeSpanLabel::new(style, *id.unwrap_or(&0), range).with_message(&label.message)
}

/// Returns `true` when `output` refers to the same file as any loaded source.
///
/// Paths are canonicalized before comparison so relative-vs-absolute
/// differences and symbolic links resolve to the same target. When `output`
/// does not yet exist its canonicalization fails, so there is no conflict.
fn output_conflicts_with_source(project: &FileBackedProject, output: &Path) -> bool {
    let Ok(output) = canonicalize(output) else {
        return false;
    };
    project.sources().iter().any(|source| {
        canonicalize(PathBuf::from(source.file_id().to_string()))
            .map(|resolved| resolved == output)
            .unwrap_or(false)
    })
}

fn diagnostic(problem: Problem, path: &Path, message: String) -> Vec<Diagnostic> {
    vec![Diagnostic::problem(
        problem,
        Label::file(FileId::from_path(path), message),
    )]
}

#[cfg(test)]
mod tests {
    use ironplc_test::shared_resource_path;

    use ironplc_parser::options::CompilerOptions;

    use crate::{cli::check, cli::compile, cli::echo, cli::tokenize, test_helpers::resource_path};

    #[test]
    fn check_first_steps_when_invalid_syntax_then_error() {
        let paths = vec![shared_resource_path("first_steps_semantic_error.st")];
        let result = check(&paths, CompilerOptions::default(), &[], true);
        assert!(result.is_err())
    }

    #[test]
    fn problem_help_url_when_diagnostic_then_url_tagged_for_cli() {
        use ironplc_dsl::core::SourceSpan;
        use ironplc_dsl::diagnostic::{Diagnostic, Label};
        use ironplc_problems::Problem;

        let diag = Diagnostic::problem(
            Problem::SyntaxError,
            Label::span(SourceSpan::default(), "some error".to_string()),
        )
        .with_source("compiler/analyzer/src/rule_example.rs", 42);

        let url = super::problem_help_url(&diag);
        assert!(url.contains("/reference/compiler/problems/"));
        assert!(url.contains("?version="));
        assert!(url.contains("&channel=cli"));
        assert!(url.contains("&file=compiler/analyzer/src/rule_example.rs"));
        assert!(url.contains("&line=42"));
    }

    #[test]
    fn check_first_steps_when_valid_syntax_then_ok() {
        let paths = vec![shared_resource_path("first_steps.st")];
        let result = check(&paths, CompilerOptions::default(), &[], true);
        assert!(result.is_ok())
    }

    #[test]
    fn check_first_steps_dir_when_valid_syntax_then_ok() {
        let paths = vec![resource_path("set")];
        let result = check(&paths, CompilerOptions::default(), &[], true);
        assert!(result.is_ok())
    }

    #[test]
    fn check_when_plcproj_references_missing_file_then_error() {
        // Maintainer feedback on the PR that introduced discovery
        // continuing past one bad `.plcproj` entry: the missing
        // reference itself must still fail the overall command, even
        // though discovery no longer aborts because of it.
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("project.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="MISSING.TcPOU" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let paths = vec![dir.path().to_path_buf()];
        let result = check(&paths, CompilerOptions::default(), &[], true);
        assert!(result.is_err())
    }

    #[test]
    fn check_when_plcproj_has_valid_and_missing_entries_then_semantic_error_still_surfaces() {
        // The valid entry must still be loaded and checked -- not just
        // that the overall command fails (which a discovery-time-only
        // failure would also produce), but specifically that
        // project.semantic() actually ran against it. A.st here has a
        // real semantic error (an undeclared variable): if analysis
        // never ran, check() would still return Err, but with the
        // discovery-stage message ("Error enumerating..."), not the
        // analysis-stage one -- this is what distinguishes "aborted
        // before checking" from "checked, and found a real bug".
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("A.st"),
            "PROGRAM A\nVAR\n    x : INT;\nEND_VAR\n    x := UNDECLARED_VAR;\nEND_PROGRAM",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("project.plcproj"),
            r#"<Project>
  <ItemGroup>
    <Compile Include="A.st" />
    <Compile Include="MISSING.TcPOU" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let paths = vec![dir.path().to_path_buf()];
        let result = check(&paths, CompilerOptions::default(), &[], true);
        assert_eq!(result, Err(String::from("Error during analysis")));
    }

    #[test]
    fn check_when_plcproj_references_unbundled_library_then_still_runs_analysis() {
        // A project referencing a library IronPLC doesn't bundle (P6011)
        // must not prevent analysis of the project's own files -- same
        // reasoning as the missing-.plcproj-entry case above, for the
        // other kind of discovery-time diagnostic.
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("A.st"),
            "PROGRAM A\nVAR\n    x : INT;\nEND_VAR\n    x := UNDECLARED_VAR;\nEND_PROGRAM",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("project.plcproj"),
            r#"<Project xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <ItemGroup>
    <Compile Include="A.st" />
    <PlaceholderReference Include="NotBundled" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let paths = vec![dir.path().to_path_buf()];
        let result = check(&paths, CompilerOptions::default(), &[], true);
        assert_eq!(result, Err(String::from("Error during analysis")));
    }

    #[test]
    fn echo_first_steps_when_invalid_syntax_then_error() {
        let paths = vec![shared_resource_path("first_steps_syntax_error.st")];
        let result = check(&paths, CompilerOptions::default(), &[], true);
        assert!(result.is_err())
    }

    #[test]
    fn echo_first_steps_when_valid_syntax_then_ok() {
        let paths = vec![shared_resource_path("first_steps.st")];
        let result = echo(&paths, CompilerOptions::default(), true);
        assert!(result.is_ok())
    }

    #[test]
    fn tokenize_first_steps_when_valid_syntax_then_ok() {
        let paths = vec![shared_resource_path("first_steps.st")];
        let result = echo(&paths, CompilerOptions::default(), true);
        assert!(result.is_ok())
    }

    #[test]
    fn tokenize_xml_when_valid_syntax_then_ok() {
        let paths = vec![resource_path("simple.xml")];
        let result = tokenize(&paths, CompilerOptions::default(), true);
        assert!(result.is_ok())
    }

    #[test]
    fn compile_when_steel_thread_then_creates_output() {
        let paths = vec![shared_resource_path("steel_thread.st")];
        let output = tempfile::NamedTempFile::new().unwrap();
        let result = compile(&paths, output.path(), CompilerOptions::default(), &[], true);
        assert!(result.is_ok());
        assert!(output.path().metadata().unwrap().len() > 0);
    }

    #[test]
    fn compile_when_syntax_error_then_error() {
        let paths = vec![shared_resource_path("first_steps_syntax_error.st")];
        let output = tempfile::NamedTempFile::new().unwrap();
        let result = compile(&paths, output.path(), CompilerOptions::default(), &[], true);
        assert!(result.is_err());
    }

    #[test]
    fn compile_when_plcproj_references_unbundled_library_and_unused_then_still_produces_container()
    {
        // A discovery-time diagnostic (P6011, unbundled library) must not
        // stop the compile pipeline from running: the program here never
        // calls anything from the unbundled library, so compilation is
        // genuinely valid -- compile() must still produce a real
        // container (proving analysis/codegen ran to completion) while
        // still reporting overall failure via its Err return.
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("A.st"),
            "PROGRAM A\nVAR\n    x : INT;\nEND_VAR\n    x := 1;\nEND_PROGRAM",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("project.plcproj"),
            r#"<Project xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <ItemGroup>
    <Compile Include="A.st" />
    <PlaceholderReference Include="NotBundled" />
  </ItemGroup>
</Project>"#,
        )
        .unwrap();

        let paths = vec![dir.path().to_path_buf()];
        let output = tempfile::NamedTempFile::new().unwrap();
        let result = compile(&paths, output.path(), CompilerOptions::default(), &[], true);

        assert!(result.is_err());
        assert!(output.path().metadata().unwrap().len() > 0);
    }

    #[test]
    fn compile_when_output_is_valid_container_then_roundtrips() {
        let paths = vec![shared_resource_path("steel_thread.st")];
        let output = tempfile::NamedTempFile::new().unwrap();
        compile(&paths, output.path(), CompilerOptions::default(), &[], true).unwrap();

        // Verify the output is a valid container by reading it back
        let mut file = std::fs::File::open(output.path()).unwrap();
        let container = ironplc_container::Container::read_from(&mut file).unwrap();
        assert_eq!(container.header.num_variables, 2);
        assert_eq!(container.header.num_functions, 2);
    }

    #[test]
    fn compile_when_structured_variable_then_error() {
        use std::io::Write;

        let mut source = tempfile::NamedTempFile::new().unwrap();
        write!(
            source,
            "FUNCTION MY_FUNC : BOOL
  VAR_INPUT
      x : BYTE;
  END_VAR
      IF setup.EXTENDED_ASCII THEN
          MY_FUNC := TRUE;
      ELSE
          MY_FUNC := FALSE;
      END_IF;
  END_FUNCTION
  PROGRAM main
  VAR
      result : BOOL;
  END_VAR
      result := MY_FUNC(x := BYTE#65);
  END_PROGRAM"
        )
        .unwrap();

        let output = tempfile::NamedTempFile::new().unwrap();
        let result = compile(
            &[source.path().to_path_buf()],
            output.path(),
            CompilerOptions::default(),
            &[],
            true,
        );
        assert!(result.is_err());
    }
}
