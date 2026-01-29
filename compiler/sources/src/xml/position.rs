//! Position tracking for PLCopen XML elements
//!
//! This module uses roxmltree to find the line/column positions of
//! ST body content within PLCopen XML files.

use std::collections::HashMap;

/// Position information for text content
#[derive(Debug, Clone, Copy)]
pub struct TextPosition {
    /// Line number (0-based)
    pub line: usize,
    /// Column number (0-based)
    pub col: usize,
}

/// A map from POU name to its ST body position
pub type StBodyPositions = HashMap<String, TextPosition>;

/// Find positions of ST body text content in a PLCopen XML document
///
/// Returns a map from POU name to the position of its ST body text.
/// This allows us to provide accurate line/column offsets when parsing
/// embedded ST code.
pub fn find_st_body_positions(xml_content: &str) -> Result<StBodyPositions, String> {
    let doc = roxmltree::Document::parse(xml_content)
        .map_err(|e| format!("Failed to parse XML: {}", e))?;

    let mut positions = HashMap::new();

    // Find all pou elements
    for pou_node in doc.descendants().filter(|n| n.has_tag_name("pou")) {
        let Some(pou_name) = pou_node.attribute("name") else {
            continue;
        };

        // Find the body/ST/xhtml path
        if let Some(pos) = find_st_body_position(&doc, pou_node) {
            positions.insert(pou_name.to_string(), pos);
        }
    }

    Ok(positions)
}

/// Find the position of ST body text within a POU node
fn find_st_body_position(
    doc: &roxmltree::Document,
    pou_node: roxmltree::Node,
) -> Option<TextPosition> {
    // Navigate: pou -> body -> ST -> xhtml -> text
    let body_node = pou_node.children().find(|n| n.has_tag_name("body"))?;
    let st_node = body_node.children().find(|n| n.has_tag_name("ST"))?;

    // Try xhtml first, then fall back to direct text content
    let text_node = if let Some(xhtml_node) = st_node.children().find(|n| n.has_tag_name("xhtml")) {
        // Find the text node inside xhtml
        xhtml_node.children().find(|n| n.is_text())?
    } else {
        // Direct text content in ST element
        st_node.children().find(|n| n.is_text())?
    };

    // Get the byte position and convert to line/column
    let byte_pos = text_node.range().start;
    let text_pos = doc.text_pos_at(byte_pos);

    Some(TextPosition {
        // roxmltree uses 1-based positions, convert to 0-based
        line: text_pos.row.saturating_sub(1) as usize,
        col: text_pos.col.saturating_sub(1) as usize,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_st_body_positions_when_single_pou_then_returns_position() {
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
        <interface/>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">IF Reset THEN
  Count := 0;
END_IF;</xhtml>
          </ST>
        </body>
      </pou>
    </pous>
  </types>
</project>"#;

        let positions = find_st_body_positions(xml).unwrap();

        assert!(positions.contains_key("Counter"));
        let pos = positions.get("Counter").unwrap();
        // The ST body text starts at line 18 (0-indexed: 17), after the xhtml opening tag
        assert!(pos.line > 0, "Expected line > 0, got {}", pos.line);
    }

    #[test]
    fn find_st_body_positions_when_multiple_pous_then_returns_all_positions() {
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
      <pou name="POU1" pouType="functionBlock">
        <interface/>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">x := 1;</xhtml>
          </ST>
        </body>
      </pou>
      <pou name="POU2" pouType="functionBlock">
        <interface/>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">y := 2;</xhtml>
          </ST>
        </body>
      </pou>
    </pous>
  </types>
</project>"#;

        let positions = find_st_body_positions(xml).unwrap();

        assert_eq!(positions.len(), 2);
        assert!(positions.contains_key("POU1"));
        assert!(positions.contains_key("POU2"));

        // POU2 should be on a later line than POU1
        let pos1 = positions.get("POU1").unwrap();
        let pos2 = positions.get("POU2").unwrap();
        assert!(pos2.line > pos1.line);
    }

    #[test]
    fn find_st_body_positions_when_no_st_body_then_returns_empty() {
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
      <pou name="EmptyPOU" pouType="functionBlock">
        <interface/>
      </pou>
    </pous>
  </types>
</project>"#;

        let positions = find_st_body_positions(xml).unwrap();

        assert!(positions.is_empty());
    }
}
