//! TwinCAT XML parser implementation
//!
//! This module parses TwinCAT 3 (Beckhoff) XML files (.TcPOU, .TcGVL, .TcDUT)
//! into IronPLC's DSL by extracting Structured Text from CDATA sections.
//!
//! TwinCAT splits what would normally be a single ST file into separate XML
//! sections. For POUs, the `<Declaration>` contains the header and VAR blocks
//! (e.g. `PROGRAM MAIN VAR ... END_VAR`) while `<Implementation><ST>` contains
//! the body statements. The closing keyword (e.g. `END_PROGRAM`) is implicit
//! in the XML structure and must be reconstructed for the ST parser.
//!
//! Since the ST parser produces byte positions relative to the concatenated
//! text, this module adjusts all positions to point to the correct locations
//! in the original XML file using the CDATA byte offsets from roxmltree.

use ironplc_dsl::{
    common::Library,
    core::{FileId, SourceSpan},
    diagnostic::{Diagnostic, Label},
    fold::Fold,
};
use ironplc_problems::Problem;
use log::debug;

use super::st_parser;

/// Byte offset information for CDATA sections in the original XML document.
struct CdataOffsets {
    /// Byte offset where Declaration CDATA text starts in the XML document.
    declaration_start: usize,
    /// Length of the declaration text.
    declaration_len: usize,
    /// Byte offset where Implementation/ST CDATA text starts in the XML document.
    /// None if there is no implementation section.
    implementation_start: Option<usize>,
}

/// Parse TwinCAT XML files into an IronPLC Library
///
/// # Errors
///
/// Returns a `Diagnostic` if:
/// - The XML is malformed (P0006)
/// - The XML doesn't have valid TwinCAT structure (P0009)
/// - A non-ST implementation language is used (P9003)
pub fn parse(content: &str, file_id: &FileId) -> Result<Library, Diagnostic> {
    debug!("Parsing TwinCAT XML file: {}", file_id);

    let doc = roxmltree::Document::parse(content).map_err(|e| {
        Diagnostic::problem(
            Problem::XmlMalformed,
            Label::file(file_id.clone(), format!("XML parse error: {e}")),
        )
    })?;

    let root = doc.root_element();
    if root.tag_name().name() != "TcPlcObject" {
        return Err(Diagnostic::problem(
            Problem::TwinCatMalformed,
            Label::file(
                file_id.clone(),
                format!(
                    "Expected root element 'TcPlcObject', found '{}'",
                    root.tag_name().name()
                ),
            ),
        ));
    }

    // Find the POU, GVL, or DUT child element
    let object = root
        .children()
        .find(|n| n.is_element() && matches!(n.tag_name().name(), "POU" | "GVL" | "DUT"))
        .ok_or_else(|| {
            Diagnostic::problem(
                Problem::TwinCatMalformed,
                Label::file(
                    file_id.clone(),
                    "TcPlcObject must contain a POU, GVL, or DUT element".to_string(),
                ),
            )
        })?;

    let object_type = object.tag_name().name();
    debug!("Found TwinCAT {} object", object_type);

    // Extract Declaration CDATA with byte offset
    let declaration = find_child_element(&object, "Declaration").ok_or_else(|| {
        Diagnostic::problem(
            Problem::TwinCatMalformed,
            Label::file(
                file_id.clone(),
                format!("{object_type} element is missing required 'Declaration' element"),
            ),
        )
    })?;

    let (declaration_text, declaration_byte_offset) = cdata_text_with_offset(&declaration);

    match object_type {
        "POU" => parse_pou(declaration_text, declaration_byte_offset, &object, file_id),
        "DUT" => parse_dut(declaration_text, declaration_byte_offset, file_id),
        "GVL" => parse_gvl(declaration_text, file_id),
        _ => unreachable!(),
    }
}

/// Parse a POU by combining declaration + implementation + closing keyword.
///
/// TwinCAT POU declarations contain the header (`PROGRAM MAIN`) and VAR blocks,
/// but omit the closing keyword. We detect the POU type from the declaration
/// text and append the appropriate `END_xxx` keyword.
fn parse_pou(
    declaration_text: String,
    declaration_byte_offset: usize,
    object: &roxmltree::Node,
    file_id: &FileId,
) -> Result<Library, Diagnostic> {
    let (impl_text, impl_byte_offset) = extract_pou_implementation(object, file_id)?;
    let closing = closing_keyword(&declaration_text);

    let offsets = CdataOffsets {
        declaration_start: declaration_byte_offset,
        declaration_len: declaration_text.len(),
        implementation_start: impl_byte_offset,
    };

    let combined = format!("{declaration_text}\n{impl_text}\n{closing}");
    debug!("POU combined ST ({} bytes)", combined.len());

    let result = st_parser::parse(&combined, file_id);

    match result {
        Ok(library) => {
            let mut adjuster = PositionAdjuster { offsets: &offsets };
            adjuster.fold_library(library)
        }
        Err(diag) => Err(adjust_diagnostic(&offsets, diag)),
    }
}

/// Parse a DUT — the declaration contains a complete `TYPE...END_TYPE` block.
fn parse_dut(
    declaration_text: String,
    declaration_byte_offset: usize,
    file_id: &FileId,
) -> Result<Library, Diagnostic> {
    debug!("DUT declaration ST ({} bytes)", declaration_text.len());

    let offsets = CdataOffsets {
        declaration_start: declaration_byte_offset,
        declaration_len: declaration_text.len(),
        implementation_start: None,
    };

    let result = st_parser::parse(&declaration_text, file_id);

    match result {
        Ok(library) => {
            let mut adjuster = PositionAdjuster { offsets: &offsets };
            adjuster.fold_library(library)
        }
        Err(diag) => Err(adjust_diagnostic(&offsets, diag)),
    }
}

/// Parse a GVL — the declaration contains `VAR_GLOBAL...END_VAR`.
///
/// The IEC 61131-3 ST parser does not accept standalone `VAR_GLOBAL` blocks
/// at the top level (they must be inside a CONFIGURATION with RESOURCE).
/// For Phase 1, we validate the XML structure but return an empty Library.
/// Full GVL analysis requires multi-file project support to associate global
/// variables with a configuration context.
fn parse_gvl(declaration_text: String, _file_id: &FileId) -> Result<Library, Diagnostic> {
    debug!(
        "GVL declaration ({} bytes) — structural validation only",
        declaration_text.len()
    );
    Ok(Library { elements: vec![] })
}

/// Adjust a diagnostic's source positions from concatenated-text-relative
/// to original-XML-relative using the CDATA byte offsets.
fn adjust_diagnostic(offsets: &CdataOffsets, mut diag: Diagnostic) -> Diagnostic {
    diag.primary.location.start = adjust_byte_offset(offsets, diag.primary.location.start);
    diag.primary.location.end = adjust_byte_offset(offsets, diag.primary.location.end);
    for label in &mut diag.secondary {
        label.location.start = adjust_byte_offset(offsets, label.location.start);
        label.location.end = adjust_byte_offset(offsets, label.location.end);
    }
    diag
}

/// Map a byte offset in the concatenated ST text to a byte offset in the
/// original XML document.
///
/// Positions within the declaration part (0..declaration_len) are shifted by
/// the declaration CDATA offset. Positions in the implementation part
/// (declaration_len+1..) are shifted by the implementation CDATA offset.
fn adjust_byte_offset(offsets: &CdataOffsets, pos: usize) -> usize {
    if pos <= offsets.declaration_len {
        // Position is in the declaration part
        pos + offsets.declaration_start
    } else if let Some(impl_start) = offsets.implementation_start {
        // Position is in the implementation part (after declaration + newline)
        let impl_relative = pos - offsets.declaration_len - 1;
        impl_relative + impl_start
    } else {
        // No implementation section — position is in the synthetic closing keyword.
        // Point to the end of the declaration instead.
        offsets.declaration_start + offsets.declaration_len
    }
}

/// Fold transform that adjusts all SourceSpan positions in a Library.
struct PositionAdjuster<'a> {
    offsets: &'a CdataOffsets,
}

impl Fold<Diagnostic> for PositionAdjuster<'_> {
    fn fold_source_span(&mut self, node: SourceSpan) -> Result<SourceSpan, Diagnostic> {
        Ok(SourceSpan {
            start: adjust_byte_offset(self.offsets, node.start),
            end: adjust_byte_offset(self.offsets, node.end),
            file_id: node.file_id,
        })
    }
}

/// Detect the POU type from the declaration text and return the closing keyword.
fn closing_keyword(declaration: &str) -> &'static str {
    let trimmed = declaration.trim_start();
    // Check FUNCTION_BLOCK before FUNCTION since FUNCTION is a prefix
    if trimmed.len() >= 14 && trimmed[..14].eq_ignore_ascii_case("FUNCTION_BLOCK") {
        "END_FUNCTION_BLOCK"
    } else if trimmed.len() >= 8 && trimmed[..8].eq_ignore_ascii_case("FUNCTION") {
        "END_FUNCTION"
    } else if trimmed.len() >= 7 && trimmed[..7].eq_ignore_ascii_case("PROGRAM") {
        "END_PROGRAM"
    } else {
        // Fallback — the ST parser will report a more specific error
        ""
    }
}

/// Extract the ST implementation text and its byte offset from a POU element.
fn extract_pou_implementation(
    pou: &roxmltree::Node,
    file_id: &FileId,
) -> Result<(String, Option<usize>), Diagnostic> {
    let implementation = match find_child_element(pou, "Implementation") {
        Some(elem) => elem,
        None => return Ok((String::new(), None)),
    };

    if let Some(st) = find_child_element(&implementation, "ST") {
        let (text, offset) = cdata_text_with_offset(&st);
        return Ok((text, Some(offset)));
    }

    // Check for unsupported implementation languages
    for child in implementation.children().filter(|n| n.is_element()) {
        let lang = child.tag_name().name();
        if matches!(lang, "FBD" | "LD" | "IL" | "SFC") {
            return Err(Diagnostic::problem(
                Problem::XmlBodyTypeNotSupported,
                Label::file(
                    file_id.clone(),
                    format!(
                        "POU uses {lang} which is not supported. Use ST (Structured Text) instead."
                    ),
                ),
            ));
        }
    }

    Ok((String::new(), None))
}

fn find_child_element<'a>(
    parent: &'a roxmltree::Node,
    name: &str,
) -> Option<roxmltree::Node<'a, 'a>> {
    parent
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == name)
}

/// Extract text content from a node and its byte offset in the original document.
///
/// Uses roxmltree's positions feature to get the byte offset of the text node
/// (CDATA or regular text) in the original XML. If no text child is found,
/// returns offset 0.
///
/// For CDATA sections, roxmltree includes the `<![CDATA[` and `]]>` markers
/// in the node range, so we skip past the 9-byte prefix to get the actual
/// text content offset.
fn cdata_text_with_offset(node: &roxmltree::Node) -> (String, usize) {
    if let Some(text_node) = node.children().find(|n| n.is_text()) {
        let text = text_node.text().unwrap_or("").to_string();
        let range = text_node.range();
        // roxmltree includes CDATA markers in the range. When the range
        // is larger than the text, the node is a CDATA section and we
        // need to skip past the <![CDATA[ prefix (9 bytes).
        let byte_offset = if range.len() > text.len() {
            range.start + "<![CDATA[".len()
        } else {
            range.start
        };
        (text, byte_offset)
    } else {
        (String::new(), 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::core::FileId;

    fn test_file_id() -> FileId {
        FileId::from_string("test.TcPOU")
    }

    #[test]
    fn parse_when_pou_with_declaration_and_st_then_succeeds() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="MAIN" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
    <Declaration><![CDATA[PROGRAM MAIN
VAR
    myVar : INT;
END_VAR]]></Declaration>
    <Implementation>
      <ST><![CDATA[myVar := myVar + 1;]]></ST>
    </Implementation>
  </POU>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id());
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        let library = result.unwrap();
        assert_eq!(library.elements.len(), 1);
    }

    #[test]
    fn parse_when_pou_then_positions_point_into_xml() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="MAIN" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
    <Declaration><![CDATA[PROGRAM MAIN
VAR
    myVar : INT;
END_VAR]]></Declaration>
    <Implementation>
      <ST><![CDATA[myVar := myVar + 1;]]></ST>
    </Implementation>
  </POU>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id()).unwrap();

        // Collect all spans to verify they point into the CDATA sections
        use ironplc_dsl::fold::Fold;
        let mut spans = Vec::new();
        struct SpanCollector<'a> {
            spans: &'a mut Vec<SourceSpan>,
        }
        impl Fold<()> for SpanCollector<'_> {
            fn fold_source_span(&mut self, node: SourceSpan) -> Result<SourceSpan, ()> {
                self.spans.push(node.clone());
                Ok(node)
            }
        }
        let mut collector = SpanCollector { spans: &mut spans };
        let _ = collector.fold_library(result);

        // All spans should point to positions within the XML document that
        // fall inside CDATA sections
        let cdata_start = xml.find("<![CDATA[").unwrap() + "<![CDATA[".len();
        for span in &spans {
            assert!(
                span.start >= cdata_start,
                "Span start {} should be >= CDATA start {} (pointing into XML CDATA)",
                span.start,
                cdata_start
            );
        }
    }

    #[test]
    fn parse_when_pou_syntax_error_then_position_points_into_xml() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="MAIN" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
    <Declaration><![CDATA[PROGRAM MAIN
VAR
    myVar : INT;
END_VAR]]></Declaration>
    <Implementation>
      <ST><![CDATA[INVALID SYNTAX HERE !!!]]></ST>
    </Implementation>
  </POU>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id());
        assert!(result.is_err());

        let diag = result.unwrap_err();
        // The error position should point into the Implementation CDATA
        let impl_cdata_start = xml.find("INVALID").unwrap();
        assert!(
            diag.primary.location.start >= impl_cdata_start,
            "Error position {} should be >= impl CDATA start {} (pointing into XML)",
            diag.primary.location.start,
            impl_cdata_start
        );
    }

    #[test]
    fn parse_when_function_block_pou_then_succeeds() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="FB_Counter" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
    <Declaration><![CDATA[FUNCTION_BLOCK FB_Counter
VAR
    count : INT := 0;
END_VAR]]></Declaration>
    <Implementation>
      <ST><![CDATA[count := count + 1;]]></ST>
    </Implementation>
  </POU>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id());
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        let library = result.unwrap();
        assert_eq!(library.elements.len(), 1);
    }

    #[test]
    fn parse_when_gvl_then_succeeds() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <GVL Name="GVL_Main" Id="{00000000-0000-0000-0000-000000000000}">
    <Declaration><![CDATA[VAR_GLOBAL
    gCounter : INT := 0;
    gRunning : BOOL := FALSE;
END_VAR]]></Declaration>
  </GVL>
</TcPlcObject>"#;

        let file_id = FileId::from_string("test.TcGVL");
        let result = parse(xml, &file_id);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn parse_when_dut_then_succeeds() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <DUT Name="ST_MyStruct" Id="{00000000-0000-0000-0000-000000000000}">
    <Declaration><![CDATA[TYPE ST_MyStruct :
STRUCT
    value : INT;
    name : STRING;
END_STRUCT;
END_TYPE]]></Declaration>
  </DUT>
</TcPlcObject>"#;

        let file_id = FileId::from_string("test.TcDUT");
        let result = parse(xml, &file_id);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn parse_when_malformed_xml_then_returns_p0006() {
        let xml = "NOT VALID XML <>";

        let result = parse(xml, &test_file_id());
        assert!(result.is_err());

        let diagnostic = result.unwrap_err();
        assert_eq!(diagnostic.code, "P0006");
    }

    #[test]
    fn parse_when_wrong_root_element_then_returns_p0009() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<WrongRoot>
  <POU Name="MAIN">
    <Declaration><![CDATA[PROGRAM MAIN END_PROGRAM]]></Declaration>
  </POU>
</WrongRoot>"#;

        let result = parse(xml, &test_file_id());
        assert!(result.is_err());

        let diagnostic = result.unwrap_err();
        assert_eq!(diagnostic.code, "P0009");
        assert!(diagnostic.primary.message.contains("TcPlcObject"));
    }

    #[test]
    fn parse_when_missing_object_element_then_returns_p0009() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id());
        assert!(result.is_err());

        let diagnostic = result.unwrap_err();
        assert_eq!(diagnostic.code, "P0009");
        assert!(diagnostic.primary.message.contains("POU, GVL, or DUT"));
    }

    #[test]
    fn parse_when_missing_declaration_then_returns_p0009() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="MAIN" Id="{00000000-0000-0000-0000-000000000000}">
    <Implementation>
      <ST><![CDATA[myVar := 1;]]></ST>
    </Implementation>
  </POU>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id());
        assert!(result.is_err());

        let diagnostic = result.unwrap_err();
        assert_eq!(diagnostic.code, "P0009");
        assert!(diagnostic.primary.message.contains("Declaration"));
    }

    #[test]
    fn parse_when_fbd_implementation_then_returns_p9003() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="MAIN" Id="{00000000-0000-0000-0000-000000000000}">
    <Declaration><![CDATA[PROGRAM MAIN
VAR
    myVar : INT;
END_VAR]]></Declaration>
    <Implementation>
      <FBD/>
    </Implementation>
  </POU>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id());
        assert!(result.is_err());

        let diagnostic = result.unwrap_err();
        assert_eq!(diagnostic.code, Problem::XmlBodyTypeNotSupported.code());
        assert!(diagnostic.primary.message.contains("FBD"));
    }

    #[test]
    fn closing_keyword_when_program_then_returns_end_program() {
        assert_eq!(closing_keyword("PROGRAM MAIN\nVAR\nEND_VAR"), "END_PROGRAM");
    }

    #[test]
    fn closing_keyword_when_function_block_then_returns_end_function_block() {
        assert_eq!(
            closing_keyword("FUNCTION_BLOCK FB_Test\nVAR\nEND_VAR"),
            "END_FUNCTION_BLOCK"
        );
    }

    #[test]
    fn closing_keyword_when_function_then_returns_end_function() {
        assert_eq!(
            closing_keyword("FUNCTION MyFunc : INT\nVAR\nEND_VAR"),
            "END_FUNCTION"
        );
    }
}
