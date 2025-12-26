//! Structured Text parser implementation

use ironplc_dsl::{common::Library, core::FileId, diagnostic::Diagnostic};
use ironplc_parser::{options::ParseOptions, parse_program_enhanced};

/// Parse Structured Text (.st, .iec) files with enhanced error reporting
pub fn parse(content: &str, file_id: &FileId) -> Result<Library, Diagnostic> {
    // Use enhanced parsing that provides better error messages and multiple error collection
    match parse_program_enhanced(content, file_id, &ParseOptions::default()) {
        Ok(library) => Ok(library),
        Err(diagnostics) => {
            // Return the first diagnostic for compatibility, but enhanced parsing
            // has already collected multiple errors for better user experience
            Err(diagnostics.into_iter().next().unwrap_or_else(|| {
                Diagnostic::internal_error(file!(), line!())
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::core::FileId;

    #[test]
    fn parse_simple_program() {
        let content = r#"
PROGRAM Main
VAR
    x : INT := 42;
END_VAR
    x := x + 1;
END_PROGRAM
"#;
        let file_id = FileId::from_string("test.st");
        let result = parse(content, &file_id);

        assert!(result.is_ok());
        let library = result.unwrap();
        assert_eq!(library.elements.len(), 1);
    }

    #[test]
    fn parse_invalid_syntax() {
        let content = "INVALID SYNTAX";
        let file_id = FileId::from_string("test.st");
        let result = parse(content, &file_id);

        assert!(result.is_err());
    }
}
