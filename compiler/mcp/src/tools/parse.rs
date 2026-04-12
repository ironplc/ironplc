//! The `parse` MCP tool.
//!
//! Syntax-checks source files and returns diagnostics plus a best-effort
//! `structure` array describing the top-level declarations found.

use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_parser::declarations::extract_declarations;
use ironplc_parser::parse_program;
use serde::Serialize;

use super::common::{
    parse_options, serialize_diagnostics, validate_sources, SourceInput, StructureEntry,
};

/// Response returned by the `parse` tool.
#[derive(Debug, Serialize)]
pub struct ParseResponse {
    pub ok: bool,
    pub structure: Vec<StructureEntry>,
    pub diagnostics: Vec<serde_json::Value>,
}

/// Builds the parse response from raw inputs.
pub fn build_response(sources: &[SourceInput], options_value: &serde_json::Value) -> ParseResponse {
    // Validate sources
    let source_errors = validate_sources(sources);
    if !source_errors.is_empty() {
        return ParseResponse {
            ok: false,
            structure: vec![],
            diagnostics: serialize_diagnostics(&source_errors),
        };
    }

    // Parse options
    let options = match parse_options(options_value) {
        Ok(opts) => opts,
        Err(errs) => {
            return ParseResponse {
                ok: false,
                structure: vec![],
                diagnostics: serialize_diagnostics(&errs),
            };
        }
    };

    let mut structure = Vec::new();
    let mut diagnostics: Vec<Diagnostic> = Vec::new();

    for src in sources {
        let file_id = FileId::from_string(&src.name);
        match parse_program(&src.content, &file_id, &options) {
            Ok(library) => {
                for decl in extract_declarations(&library) {
                    structure.push(StructureEntry {
                        kind: decl.kind.to_string(),
                        name: decl.name,
                        file: src.name.clone(),
                        start: decl.start,
                        end: decl.end,
                    });
                }
            }
            Err(diag) => {
                diagnostics.push(diag);
            }
        }
    }

    let ok = diagnostics.is_empty();

    ParseResponse {
        ok,
        structure,
        diagnostics: serialize_diagnostics(&diagnostics),
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
        assert!(resp.ok);
        assert!(resp.diagnostics.is_empty());
    }

    #[test]
    fn build_response_when_syntax_error_then_ok_false_with_diagnostics() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options());
        assert!(!resp.ok);
        assert!(!resp.diagnostics.is_empty());
    }

    #[test]
    fn build_response_when_valid_program_then_structure_has_program() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "PROGRAM p\nEND_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options());
        assert_eq!(resp.structure.len(), 1);
        assert_eq!(resp.structure[0].kind, "program");
        assert_eq!(resp.structure[0].name.as_deref(), Some("p"));
    }

    #[test]
    fn build_response_when_function_and_program_then_structure_has_both() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "FUNCTION f : INT\nVAR_INPUT x : INT; END_VAR\nf := x;\nEND_FUNCTION\nPROGRAM p\nEND_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options());
        assert_eq!(resp.structure.len(), 2);
        let kinds: Vec<&str> = resp.structure.iter().map(|s| s.kind.as_str()).collect();
        assert!(kinds.contains(&"function"));
        assert!(kinds.contains(&"program"));
    }

    #[test]
    fn build_response_when_function_block_then_structure_has_fb() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "FUNCTION_BLOCK fb\nEND_FUNCTION_BLOCK".into(),
        }];
        let resp = build_response(&sources, &ed2_options());
        assert_eq!(resp.structure.len(), 1);
        assert_eq!(resp.structure[0].kind, "function_block");
        assert_eq!(resp.structure[0].name.as_deref(), Some("fb"));
    }

    #[test]
    fn build_response_when_type_declaration_then_structure_has_type() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "TYPE\nMyEnum : (A, B, C);\nEND_TYPE".into(),
        }];
        let resp = build_response(&sources, &ed2_options());
        assert!(resp.ok, "expected ok, diagnostics: {:?}", resp.diagnostics);
        assert_eq!(resp.structure.len(), 1);
        assert_eq!(resp.structure[0].kind, "type");
    }

    #[test]
    fn build_response_when_invalid_sources_then_error_diagnostic() {
        let sources = vec![SourceInput {
            name: String::new(),
            content: "PROGRAM p END_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options());
        assert!(!resp.ok);
        assert!(!resp.diagnostics.is_empty());
    }

    #[test]
    fn build_response_when_invalid_options_then_error_diagnostic() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "PROGRAM p END_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &serde_json::json!({}));
        assert!(!resp.ok);
        assert!(!resp.diagnostics.is_empty());
    }

    #[test]
    fn build_response_when_multiple_sources_then_all_parsed() {
        let sources = vec![
            SourceInput {
                name: "a.st".into(),
                content: "PROGRAM a\nEND_PROGRAM".into(),
            },
            SourceInput {
                name: "b.st".into(),
                content: "PROGRAM b\nEND_PROGRAM".into(),
            },
        ];
        let resp = build_response(&sources, &ed2_options());
        assert!(resp.ok);
        assert_eq!(resp.structure.len(), 2);
    }

    #[test]
    fn build_response_when_valid_then_structure_has_byte_offsets() {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: "PROGRAM p\nEND_PROGRAM".into(),
        }];
        let resp = build_response(&sources, &ed2_options());
        let entry = &resp.structure[0];
        // "PROGRAM p" — 'p' starts at byte 8
        assert_eq!(entry.start, 8);
    }
}
