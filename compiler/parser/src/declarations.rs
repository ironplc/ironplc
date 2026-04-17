//! Extracts top-level declaration summaries from a parsed `Library`.
//!
//! This keeps the `LibraryElementKind`-variant matching inside the parser
//! crate boundary so that other crates (e.g. the MCP server) do not need
//! to depend on DSL details.

use dsl::common::{DataTypeDeclarationKind, Library, LibraryElementKind};
use dsl::core::{FileId, Located, SourceSpan};

/// Summary of a single top-level declaration found by the parser.
#[derive(Debug, Clone)]
pub struct DeclarationSummary {
    /// One of `"program"`, `"function"`, `"function_block"`, `"type"`,
    /// or `"configuration"`.
    pub kind: &'static str,
    /// The name of the declaration, or `None` if the parser could not
    /// recover one.
    pub name: Option<String>,
    /// The file that contains this declaration.
    pub file_id: FileId,
    /// 0-indexed byte offset of the start of the declaration's name span.
    pub start: usize,
    /// 0-indexed byte offset one past the end of the name span.
    pub end: usize,
}

/// Extracts declaration summaries from a parsed library.
///
/// Each top-level element (program, function, function block, type,
/// configuration) produces one entry. Global variable declarations
/// are skipped — they have no single "name" to report.
pub fn extract_declarations(library: &Library) -> Vec<DeclarationSummary> {
    let mut result = Vec::new();
    for element in &library.elements {
        if let Some(summary) = element_to_summary(element) {
            result.push(summary);
        }
    }
    result
}

fn element_to_summary(element: &LibraryElementKind) -> Option<DeclarationSummary> {
    match element {
        LibraryElementKind::ProgramDeclaration(decl) => {
            let span = decl.name.span();
            Some(DeclarationSummary {
                kind: "program",
                name: Some(decl.name.to_string()),
                file_id: span.file_id.clone(),
                start: span.start,
                end: span.end,
            })
        }
        LibraryElementKind::FunctionDeclaration(decl) => {
            let span = decl.name.span();
            Some(DeclarationSummary {
                kind: "function",
                name: Some(decl.name.to_string()),
                file_id: span.file_id.clone(),
                start: span.start,
                end: span.end,
            })
        }
        LibraryElementKind::FunctionBlockDeclaration(decl) => {
            let span = decl.span();
            Some(DeclarationSummary {
                kind: "function_block",
                name: Some(decl.name.to_string()),
                file_id: span.file_id.clone(),
                start: span.start,
                end: span.end,
            })
        }
        LibraryElementKind::DataTypeDeclaration(decl) => {
            let name = data_type_name(decl);
            let span = data_type_span(decl);
            Some(DeclarationSummary {
                kind: "type",
                name,
                file_id: span.file_id.clone(),
                start: span.start,
                end: span.end,
            })
        }
        LibraryElementKind::ConfigurationDeclaration(decl) => {
            let span = decl.name.span();
            Some(DeclarationSummary {
                kind: "configuration",
                name: Some(decl.name.to_string()),
                file_id: span.file_id.clone(),
                start: span.start,
                end: span.end,
            })
        }
        LibraryElementKind::GlobalVarDeclarations(_) => None,
    }
}

fn data_type_name(decl: &DataTypeDeclarationKind) -> Option<String> {
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

fn data_type_span(decl: &DataTypeDeclarationKind) -> SourceSpan {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::CompilerOptions;
    use crate::parse_program;
    use dsl::core::FileId;

    fn parse(source: &str) -> Library {
        let file_id = FileId::from_string("test.st");
        let options = CompilerOptions::default();
        parse_program(source, &file_id, &options).unwrap()
    }

    #[test]
    fn extract_declarations_when_program_then_has_program() {
        let lib = parse("PROGRAM p\nEND_PROGRAM");
        let decls = extract_declarations(&lib);
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].kind, "program");
        assert_eq!(decls[0].name.as_deref(), Some("p"));
    }

    #[test]
    fn extract_declarations_when_function_block_then_has_fb() {
        let lib = parse("FUNCTION_BLOCK fb\nEND_FUNCTION_BLOCK");
        let decls = extract_declarations(&lib);
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].kind, "function_block");
        assert_eq!(decls[0].name.as_deref(), Some("fb"));
    }

    #[test]
    fn extract_declarations_when_enum_type_then_has_type() {
        let lib = parse("TYPE\nMyEnum : (A, B);\nEND_TYPE");
        let decls = extract_declarations(&lib);
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].kind, "type");
    }

    #[test]
    fn extract_declarations_when_multiple_then_all_returned() {
        let lib = parse("FUNCTION_BLOCK fb\nEND_FUNCTION_BLOCK\nPROGRAM p\nEND_PROGRAM");
        let decls = extract_declarations(&lib);
        assert_eq!(decls.len(), 2);
    }
}
