//! PLCopen XML parser implementation
//!
//! This module parses PLCopen TC6 XML files into IronPLC's DSL.

use ironplc_dsl::{
    common::Library,
    core::{FileId, SourceSpan},
    diagnostic::{Diagnostic, Label},
};
use ironplc_problems::Problem;
use log::debug;

use crate::xml::{position::parse_plcopen_xml, transform::transform_project};

/// Parse PLCopen XML (.xml) files into an IronPLC Library
///
/// This function:
/// 1. Parses the XML using roxmltree (for accurate error positions)
/// 2. Transforms the schema structures into IronPLC's DSL
///
/// # Errors
///
/// Returns a `Diagnostic` if:
/// - The XML is malformed (P0006)
/// - The XML doesn't conform to PLCopen schema (P0007)
/// - An unsupported body language is used (P9999 - not yet implemented)
pub fn parse(content: &str, file_id: &FileId) -> Result<Library, Diagnostic> {
    debug!("Parsing PLCopen XML file: {}", file_id);

    // Parse the XML into PLCopen schema structures using roxmltree
    let project = parse_plcopen_xml(content, file_id)?;

    debug!(
        "Parsed PLCopen project '{}' with {} POUs and {} data types",
        project.content_header.name,
        project.types.pous.pou.len(),
        project.types.data_types.data_type.len()
    );

    // Check for unsupported body languages
    for pou in &project.types.pous.pou {
        if let Some(ref body) = pou.body {
            if let Some((lang, range)) = body.unsupported_language() {
                let label = if let Some(range) = range {
                    Label::span(
                        SourceSpan {
                            start: range.start,
                            end: range.end,
                            file_id: file_id.clone(),
                        },
                        format!(
                            "POU '{}' uses {} which is not supported. Use ST (Structured Text) instead.",
                            pou.name, lang
                        ),
                    )
                } else {
                    Label::file(
                        file_id.clone(),
                        format!(
                            "POU '{}' uses {} which is not supported. Use ST (Structured Text) instead.",
                            pou.name, lang
                        ),
                    )
                };
                return Err(Diagnostic::problem(Problem::XmlBodyTypeNotSupported, label));
            }
        }
    }

    // Transform to IronPLC DSL
    transform_project(&project, file_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::core::FileId;

    fn test_file_id() -> FileId {
        FileId::from_string("test.xml")
    }

    #[test]
    fn parse_when_minimal_valid_project_then_returns_library() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
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

        let result = parse(xml, &test_file_id());
        assert!(result.is_ok());
    }

    #[test]
    fn parse_when_malformed_xml_then_returns_error() {
        let xml = "NOT VALID XML <>";

        let result = parse(xml, &test_file_id());
        assert!(result.is_err());

        let diagnostic = result.unwrap_err();
        assert_eq!(diagnostic.code, "P0006");
        assert!(diagnostic.primary.message.contains("XML parse error"));
    }

    #[test]
    fn parse_when_missing_required_element_then_returns_error() {
        // Missing types element - root element check will fail
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<notproject xmlns="http://www.plcopen.org/xml/tc6_0201">
</notproject>"#;

        let result = parse(xml, &test_file_id());
        assert!(result.is_err());

        let diagnostic = result.unwrap_err();
        assert_eq!(diagnostic.code, "P0007");
    }

    #[test]
    fn parse_when_function_block_with_st_body_then_succeeds() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
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
    <pous>
      <pou name="Counter" pouType="functionBlock">
        <interface>
          <inputVars>
            <variable name="Reset">
              <type><BOOL/></type>
            </variable>
          </inputVars>
        </interface>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">
IF Reset THEN
  ; (* do nothing *)
END_IF;
            </xhtml>
          </ST>
        </body>
      </pou>
    </pous>
  </types>
</project>"#;

        let result = parse(xml, &test_file_id());
        assert!(result.is_ok());
    }

    #[test]
    fn parse_when_fbd_body_then_returns_unsupported_error_with_position() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
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
    <pous>
      <pou name="FbdProgram" pouType="program">
        <body>
          <FBD>
            <!-- FBD content would be here -->
          </FBD>
        </body>
      </pou>
    </pous>
  </types>
</project>"#;

        let result = parse(xml, &test_file_id());
        assert!(result.is_err());

        let diagnostic = result.unwrap_err();
        assert_eq!(diagnostic.code, Problem::XmlBodyTypeNotSupported.code());
        assert!(diagnostic.primary.message.contains("FBD"));
        assert!(diagnostic.primary.message.contains("not supported"));

        // Verify that the error location points to the FBD element, not the file
        assert!(
            diagnostic.primary.location.start > 0,
            "Expected error to point to the FBD element in the XML"
        );
        assert!(
            diagnostic.primary.location.end > diagnostic.primary.location.start,
            "Expected valid range for error location"
        );

        // Verify the location points to the FBD element by checking the byte range contains "<FBD>"
        let error_text = &xml[diagnostic.primary.location.start..diagnostic.primary.location.end];
        assert!(
            error_text.contains("<FBD>") || error_text.starts_with("<FBD"),
            "Expected error location to contain the FBD element, but got: {}",
            error_text
        );
    }

    #[test]
    fn parse_when_ld_body_then_returns_unsupported_error_with_position() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
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
    <pous>
      <pou name="LdProgram" pouType="program">
        <body>
          <LD>
            <!-- LD content would be here -->
          </LD>
        </body>
      </pou>
    </pous>
  </types>
</project>"#;

        let result = parse(xml, &test_file_id());
        assert!(result.is_err());

        let diagnostic = result.unwrap_err();
        assert_eq!(diagnostic.code, Problem::XmlBodyTypeNotSupported.code());
        assert!(diagnostic.primary.message.contains("LD"));
        assert!(diagnostic.primary.message.contains("not supported"));

        // Verify that the error location points to the LD element, not the file
        assert!(
            diagnostic.primary.location.start > 0,
            "Expected error to point to the LD element in the XML"
        );
        assert!(
            diagnostic.primary.location.end > diagnostic.primary.location.start,
            "Expected valid range for error location"
        );

        // Verify the location points to the LD element by checking the byte range contains "<LD>"
        let error_text = &xml[diagnostic.primary.location.start..diagnostic.primary.location.end];
        assert!(
            error_text.contains("<LD>") || error_text.starts_with("<LD"),
            "Expected error location to contain the LD element, but got: {}",
            error_text
        );
    }
}
