//! The `parse` MCP tool.
//!
//! Syntax-checks source files and returns diagnostics plus a best-effort
//! `structure` array describing the top-level declarations found.

use std::collections::HashMap;

use ironplc_dsl::common::LibraryElementKind;
use ironplc_dsl::core::{FileId, Located, SourceSpan};
use ironplc_parser::parse_program;
use serde::Serialize;

use super::common::{
    map_diagnostic, parse_options, validate_sources, McpDiagnostic, SourceInput, StructureEntry,
};

/// Response returned by the `parse` tool.
#[derive(Debug, Serialize)]
pub struct ParseResponse {
    pub ok: bool,
    pub structure: Vec<StructureEntry>,
    pub diagnostics: Vec<McpDiagnostic>,
}

/// Builds the parse response from raw inputs.
pub fn build_response(sources: &[SourceInput], options_value: &serde_json::Value) -> ParseResponse {
    // Validate sources
    let source_errors = validate_sources(sources);
    if !source_errors.is_empty() {
        return ParseResponse {
            ok: false,
            structure: vec![],
            diagnostics: source_errors,
        };
    }

    // Parse options
    let options = match parse_options(options_value) {
        Ok(opts) => opts,
        Err(errs) => {
            return ParseResponse {
                ok: false,
                structure: vec![],
                diagnostics: errs,
            };
        }
    };

    let mut structure = Vec::new();
    let mut diagnostics = Vec::new();

    // Build source map for diagnostic line/col conversion
    let source_map: HashMap<FileId, &str> = sources
        .iter()
        .map(|s| (FileId::from_string(&s.name), s.content.as_str()))
        .collect();

    for src in sources {
        let file_id = FileId::from_string(&src.name);
        match parse_program(&src.content, &file_id, &options) {
            Ok(library) => {
                for element in &library.elements {
                    if let Some(entry) =
                        element_to_structure_entry(element, &src.name, &src.content)
                    {
                        structure.push(entry);
                    }
                }
            }
            Err(diag) => {
                diagnostics.push(map_diagnostic(&diag, &source_map));
            }
        }
    }

    let ok = !diagnostics.iter().any(|d| d.severity == "error");

    ParseResponse {
        ok,
        structure,
        diagnostics,
    }
}

/// Converts a `LibraryElementKind` into a `StructureEntry`.
fn element_to_structure_entry(
    element: &LibraryElementKind,
    file_name: &str,
    source: &str,
) -> Option<StructureEntry> {
    match element {
        LibraryElementKind::ProgramDeclaration(decl) => {
            let span = decl.name.span();
            let (start_line, end_line) = span_to_lines(source, &span);
            Some(StructureEntry {
                kind: "program".to_string(),
                name: Some(decl.name.to_string()),
                file: file_name.to_string(),
                start_line,
                end_line,
            })
        }
        LibraryElementKind::FunctionDeclaration(decl) => {
            let span = decl.name.span();
            let (start_line, end_line) = span_to_lines(source, &span);
            Some(StructureEntry {
                kind: "function".to_string(),
                name: Some(decl.name.to_string()),
                file: file_name.to_string(),
                start_line,
                end_line,
            })
        }
        LibraryElementKind::FunctionBlockDeclaration(decl) => {
            let span = decl.span();
            let (start_line, end_line) = span_to_lines(source, &span);
            Some(StructureEntry {
                kind: "function_block".to_string(),
                name: Some(decl.name.to_string()),
                file: file_name.to_string(),
                start_line,
                end_line,
            })
        }
        LibraryElementKind::DataTypeDeclaration(decl) => {
            let name = data_type_name(decl);
            let span = data_type_span(decl);
            let (start_line, end_line) = span_to_lines(source, &span);
            Some(StructureEntry {
                kind: "type".to_string(),
                name,
                file: file_name.to_string(),
                start_line,
                end_line,
            })
        }
        LibraryElementKind::ConfigurationDeclaration(decl) => {
            let span = decl.name.span();
            let (start_line, end_line) = span_to_lines(source, &span);
            Some(StructureEntry {
                kind: "configuration".to_string(),
                name: Some(decl.name.to_string()),
                file: file_name.to_string(),
                start_line,
                end_line,
            })
        }
        LibraryElementKind::GlobalVarDeclarations(_) => None,
    }
}

/// Extracts the name from a `DataTypeDeclarationKind`.
fn data_type_name(decl: &ironplc_dsl::common::DataTypeDeclarationKind) -> Option<String> {
    use ironplc_dsl::common::DataTypeDeclarationKind;
    match decl {
        DataTypeDeclarationKind::Enumeration(d) => Some(d.type_name.to_string()),
        DataTypeDeclarationKind::Subrange(d) => Some(d.type_name.to_string()),
        DataTypeDeclarationKind::Simple(d) => Some(d.type_name.to_string()),
        DataTypeDeclarationKind::Array(d) => Some(d.type_name.to_string()),
        DataTypeDeclarationKind::Structure(d) => Some(d.type_name.to_string()),
        DataTypeDeclarationKind::StructureInitialization(d) => Some(d.type_name.to_string()),
        DataTypeDeclarationKind::String(d) => Some(d.type_name.to_string()),
        DataTypeDeclarationKind::Reference(d) => Some(d.type_name.to_string()),
        DataTypeDeclarationKind::LateBound(d) => Some(d.data_type_name.to_string()),
    }
}

/// Extracts the span from a `DataTypeDeclarationKind` using its type name.
fn data_type_span(decl: &ironplc_dsl::common::DataTypeDeclarationKind) -> SourceSpan {
    use ironplc_dsl::common::DataTypeDeclarationKind;
    match decl {
        DataTypeDeclarationKind::Enumeration(d) => d.type_name.span(),
        DataTypeDeclarationKind::Subrange(d) => d.type_name.span(),
        DataTypeDeclarationKind::Simple(d) => d.type_name.span(),
        DataTypeDeclarationKind::Array(d) => d.type_name.span(),
        DataTypeDeclarationKind::Structure(d) => d.type_name.span(),
        DataTypeDeclarationKind::StructureInitialization(d) => d.type_name.span(),
        DataTypeDeclarationKind::String(d) => d.type_name.span(),
        DataTypeDeclarationKind::Reference(d) => d.type_name.span(),
        DataTypeDeclarationKind::LateBound(d) => d.span(),
    }
}

/// Converts a `SourceSpan` to 1-indexed (start_line, end_line) using the
/// source text.
fn span_to_lines(source: &str, span: &SourceSpan) -> (u32, u32) {
    let start_line = byte_offset_to_line(source, span.start);
    let end_line = byte_offset_to_line(source, span.end);
    (start_line, end_line)
}

/// Converts a 0-indexed byte offset into a 1-indexed line number.
fn byte_offset_to_line(source: &str, offset: usize) -> u32 {
    let mut line: u32 = 1;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
        }
    }
    line
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
}
