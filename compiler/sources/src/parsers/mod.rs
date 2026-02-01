//! Parsers for different file types

pub mod st_parser;
pub mod xml_parser;

use ironplc_dsl::{common::Library, core::FileId, diagnostic::Diagnostic};
use ironplc_problems::Problem;

use crate::file_type::FileType;

/// Parse source content based on file type
pub fn parse_source(
    file_type: FileType,
    content: &str,
    file_id: &FileId,
) -> Result<Library, Diagnostic> {
    match file_type {
        FileType::StructuredText => st_parser::parse(content, file_id),
        FileType::Xml => xml_parser::parse(content, file_id),
        FileType::Unknown => Err(Diagnostic::problem(
            Problem::UnsupportedFileType,
            ironplc_dsl::diagnostic::Label::file(
                file_id.clone(),
                format!("Unsupported file type: {file_type:?}"),
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::core::FileId;

    #[test]
    fn parse_source_structured_text() {
        let content = "PROGRAM Main\nEND_PROGRAM";
        let file_id = FileId::from_string("test.st");
        let result = parse_source(FileType::StructuredText, content, &file_id);
        assert!(result.is_ok());
    }

    #[test]
    fn parse_source_xml() {
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0201">
  <fileHeader companyName="Test" productName="Test" productVersion="1.0" creationDateTime="2024-01-01T00:00:00"/>
  <contentHeader name="TestProject">
    <coordinateInfo>
      <fbd><scaling x="1" y="1"/></fbd>
      <ld><scaling x="1" y="1"/></ld>
      <sfc><scaling x="1" y="1"/></sfc>
    </coordinateInfo>
  </contentHeader>
  <types>
    <dataTypes/>
    <pous/>
  </types>
</project>"#;
        let file_id = FileId::from_string("test.xml");
        let result = parse_source(FileType::Xml, content, &file_id);
        assert!(result.is_ok());
        let library = result.unwrap();
        assert_eq!(library.elements.len(), 0);
    }

    #[test]
    fn parse_source_unknown_file_type() {
        let content = "some content";
        let file_id = FileId::from_string("test.unknown");
        let result = parse_source(FileType::Unknown, content, &file_id);
        assert!(result.is_err());
    }
}
