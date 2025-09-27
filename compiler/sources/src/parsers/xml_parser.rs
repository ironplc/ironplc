//! XML parser implementation

use ironplc_dsl::{common::Library, core::FileId, diagnostic::Diagnostic};
use log::debug;

/// Parse XML (.xml) files
///
/// Currently returns an empty Library as requested, but can be extended
/// to parse actual XML content in the future.
pub fn parse(_content: &str, file_id: &FileId) -> Result<Library, Diagnostic> {
    debug!("XML file detected, returning empty library for {}", file_id);
    // For now, return an empty Library as requested
    // TODO: Implement actual XML parsing when needed
    Ok(Library::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::core::FileId;

    #[test]
    fn parse_xml_returns_empty_library() {
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <name>Test Project</name>
    <description>This is a test XML file</description>
</project>"#;
        let file_id = FileId::from_string("test.xml");
        let result = parse(content, &file_id);

        assert!(result.is_ok());
        let library = result.unwrap();
        assert_eq!(library.elements.len(), 0); // Should be empty
    }

    #[test]
    fn parse_invalid_xml_still_returns_empty_library() {
        let content = "INVALID XML CONTENT";
        let file_id = FileId::from_string("test.xml");
        let result = parse(content, &file_id);

        // Should still return empty library, not parse the XML
        assert!(result.is_ok());
        let library = result.unwrap();
        assert_eq!(library.elements.len(), 0);
    }
}
