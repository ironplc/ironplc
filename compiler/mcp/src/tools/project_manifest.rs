//! The `project_manifest` MCP tool.
//!
//! Returns a flat summary of what is declared across the supplied sources:
//! file names, Program / Function / Function Block names, and user-defined
//! types grouped by kind. Implements REQ-TOL-200 and REQ-TOL-201.

use ironplc_analyzer::{IntermediateType, SemanticContext};
use ironplc_dsl::core::FileId;
use ironplc_project::project::{MemoryBackedProject, Project};
use serde::Serialize;

use super::common::{parse_options, serialize_diagnostics, validate_sources, SourceInput};

/// Response returned by the `project_manifest` tool.
#[derive(Debug, Serialize)]
pub struct ProjectManifestResponse {
    pub ok: bool,
    pub files: Vec<String>,
    pub programs: Vec<String>,
    pub functions: Vec<String>,
    pub function_blocks: Vec<String>,
    pub enumerations: Vec<String>,
    pub structures: Vec<String>,
    pub arrays: Vec<String>,
    pub subranges: Vec<String>,
    pub aliases: Vec<String>,
    pub strings: Vec<String>,
    pub references: Vec<String>,
    pub diagnostics: Vec<serde_json::Value>,
}

impl ProjectManifestResponse {
    fn empty(ok: bool, files: Vec<String>, diagnostics: Vec<serde_json::Value>) -> Self {
        Self {
            ok,
            files,
            programs: vec![],
            functions: vec![],
            function_blocks: vec![],
            enumerations: vec![],
            structures: vec![],
            arrays: vec![],
            subranges: vec![],
            aliases: vec![],
            strings: vec![],
            references: vec![],
            diagnostics,
        }
    }
}

/// Builds the `project_manifest` response from raw inputs.
pub fn build_response(
    sources: &[SourceInput],
    options_value: &serde_json::Value,
) -> ProjectManifestResponse {
    let source_errors = validate_sources(sources);
    if !source_errors.is_empty() {
        return ProjectManifestResponse::empty(
            false,
            vec![],
            serialize_diagnostics(&source_errors),
        );
    }

    let options = match parse_options(options_value) {
        Ok(opts) => opts,
        Err(errs) => {
            return ProjectManifestResponse::empty(false, vec![], serialize_diagnostics(&errs));
        }
    };

    let files: Vec<String> = sources.iter().map(|s| s.name.clone()).collect();

    let mut project = MemoryBackedProject::new(options);
    for src in sources {
        project.add_source(FileId::from_string(&src.name), src.content.clone());
    }

    let diagnostics_json = match project.semantic() {
        Ok(()) => vec![],
        Err(diags) => serialize_diagnostics(&diags),
    };

    let has_errors = diagnostics_json
        .iter()
        .any(|d| d["severity"].as_str() == Some("error"));

    // REQ-TOL-201: always attempt to populate a partial manifest. `semantic_context()`
    // is set by `MemoryBackedProject::semantic()` even when analysis fails, as long
    // as a context could be built from what the parser recognized.
    let context = match project.semantic_context() {
        Some(ctx) => ctx,
        None => {
            let mut sorted_files = files;
            sorted_files.sort();
            return ProjectManifestResponse::empty(!has_errors, sorted_files, diagnostics_json);
        }
    };

    let mut response = populate(context, files, diagnostics_json);
    response.ok = !has_errors;
    response
}

fn populate(
    context: &SemanticContext,
    files: Vec<String>,
    diagnostics: Vec<serde_json::Value>,
) -> ProjectManifestResponse {
    let mut files = files;
    files.sort();

    let mut programs: Vec<String> = context
        .symbols()
        .get_programs()
        .into_iter()
        .map(|(name, _)| name.to_string())
        .collect();
    programs.sort();

    let mut function_blocks: Vec<String> = context
        .symbols()
        .get_function_blocks()
        .into_iter()
        .map(|(name, _)| name.to_string())
        .collect();
    function_blocks.sort();

    let mut functions: Vec<String> = context
        .functions()
        .iter_user_defined()
        .map(|(name, _)| name.clone())
        .collect();
    functions.sort();

    let mut enumerations = Vec::new();
    let mut structures = Vec::new();
    let mut arrays = Vec::new();
    let mut subranges = Vec::new();
    let mut aliases = Vec::new();
    let mut strings = Vec::new();
    let mut references = Vec::new();

    for (name, attrs) in context.types().iter_user_defined() {
        let bucket = match &attrs.representation {
            IntermediateType::Enumeration { .. } => &mut enumerations,
            IntermediateType::Structure { .. } => &mut structures,
            IntermediateType::Array { .. } => &mut arrays,
            IntermediateType::Subrange { .. } => &mut subranges,
            IntermediateType::String { .. } => &mut strings,
            IntermediateType::Reference { .. } => &mut references,
            _ => &mut aliases,
        };
        bucket.push(name.to_string());
    }

    enumerations.sort();
    structures.sort();
    arrays.sort();
    subranges.sort();
    aliases.sort();
    strings.sort();
    references.sort();

    ProjectManifestResponse {
        ok: true,
        files,
        programs,
        functions,
        function_blocks,
        enumerations,
        structures,
        arrays,
        subranges,
        aliases,
        strings,
        references,
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ed2_options() -> serde_json::Value {
        serde_json::json!({"dialect": "iec61131-3-ed2"})
    }

    #[test]
    fn build_response_when_valid_program_then_ok_true() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "PROGRAM p\nEND_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options());
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert_eq!(resp.programs, vec!["p".to_string()]);
        assert_eq!(resp.files, vec!["main.st".to_string()]);
    }

    #[test]
    fn build_response_when_function_block_then_in_function_blocks() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content:
                "FUNCTION_BLOCK fb\nEND_FUNCTION_BLOCK\nPROGRAM p\nVAR i : fb; END_VAR\nEND_PROGRAM"
                    .into(),
        }];
        let resp = build_response(&sources, &ed2_options());
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(resp.function_blocks.contains(&"fb".to_string()));
    }

    #[test]
    fn build_response_when_function_then_in_functions() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "FUNCTION f : INT\nVAR_INPUT a : INT; END_VAR\nf := a;\nEND_FUNCTION\nPROGRAM p\nVAR r : INT; END_VAR\nr := f(a := 1);\nEND_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options());
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(resp.functions.contains(&"f".to_string()));
    }

    #[test]
    fn build_response_when_enum_type_then_in_enumerations() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "TYPE MyEnum : (A, B, C); END_TYPE\nPROGRAM p\nEND_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options());
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(resp.enumerations.contains(&"MyEnum".to_string()));
    }

    #[test]
    fn build_response_when_struct_type_then_in_structures() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "TYPE MyStruct : STRUCT a : INT; b : REAL; END_STRUCT; END_TYPE\nPROGRAM p\nEND_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options());
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert!(resp.structures.contains(&"MyStruct".to_string()));
    }

    #[test]
    fn build_response_when_multiple_files_then_all_listed() {
        let sources = vec![
            SourceInput {
                name: "b.st".into(),
                content: "PROGRAM b\nEND_PROGRAM".into(),
            },
            SourceInput {
                name: "a.st".into(),
                content: "PROGRAM a\nEND_PROGRAM".into(),
            },
        ];
        let resp = build_response(&sources, &ed2_options());
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert_eq!(resp.files, vec!["a.st".to_string(), "b.st".to_string()]);
        assert_eq!(resp.programs, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn build_response_when_semantic_error_then_ok_false() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "PROGRAM p\nVAR x : INT; END_VAR\nx := y;\nEND_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options());
        assert!(!resp.ok);
        assert!(!resp.diagnostics.is_empty());
    }

    #[test]
    fn build_response_when_semantic_error_then_partial_manifest_preserved() {
        // REQ-TOL-201: semantic failure must still return whatever the
        // analyzer recognized.
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "PROGRAM p\nVAR x : INT; END_VAR\nx := y;\nEND_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options());
        assert!(!resp.ok);
        assert_eq!(resp.files, vec!["main.st".to_string()]);
        // The program `p` was recognized even though semantic analysis failed.
        assert!(resp.programs.contains(&"p".to_string()));
    }

    #[test]
    fn build_response_when_invalid_sources_then_error_diagnostic() {
        let sources = vec![SourceInput {
            name: String::new(),
            content: "PROGRAM p END_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options());
        assert!(!resp.ok);
        assert!(resp.diagnostics.iter().any(|d| d["code"] == "P8001"));
    }

    #[test]
    fn build_response_when_invalid_options_then_error_diagnostic() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "PROGRAM p END_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &serde_json::json!({}));
        assert!(!resp.ok);
        assert!(resp.diagnostics.iter().any(|d| d["code"] == "P8001"));
    }

    #[test]
    fn build_response_when_valid_program_then_lists_sorted() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "PROGRAM charlie\nEND_PROGRAM\nPROGRAM alpha\nEND_PROGRAM\nPROGRAM bravo\nEND_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options());
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert_eq!(
            resp.programs,
            vec![
                "alpha".to_string(),
                "bravo".to_string(),
                "charlie".to_string(),
            ]
        );
    }
}
