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
//! A function block's methods are split out the same way, each into its own
//! `<Method>` element with the same `<Declaration>`/`<Implementation>` pair.
//! They are reconstructed as `METHOD ... END_METHOD` and appended after the
//! function block body, where the grammar expects them.
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
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use log::debug;

use super::st_parser;

/// A run of bytes copied verbatim out of one CDATA section of the XML
/// document into the combined ST text.
struct CdataSegment {
    /// Byte offset where this run starts in the combined ST text.
    combined_start: usize,
    /// Length of the run, in bytes.
    len: usize,
    /// Byte offset where the same bytes start in the XML document.
    xml_start: usize,
}

/// Byte offset information for CDATA sections in the original XML document.
///
/// The combined ST text handed to the parser is a sequence of segments — one
/// per CDATA section copied out of the XML — joined by synthetic text that
/// exists only in the reconstruction (the separating newlines, `END_METHOD`,
/// and the POU's closing keyword). Segments appear in combined-text order and
/// do not overlap.
struct CdataOffsets {
    segments: Vec<CdataSegment>,
}

/// Assembles the combined ST text while recording, for every run of bytes
/// copied out of the XML, where it came from.
///
/// Keeping the text and the offsets in one place is what makes the mapping
/// back to XML positions reliable: text can only be added through
/// `push_cdata` (copied, and therefore mapped) or `push_synthetic`
/// (reconstructed, and therefore unmapped).
struct CombinedText {
    text: String,
    segments: Vec<CdataSegment>,
}

impl CombinedText {
    fn new() -> Self {
        CombinedText {
            text: String::new(),
            segments: Vec::new(),
        }
    }

    /// Append text copied from a CDATA section starting at `xml_start` in the
    /// XML document.
    fn push_cdata(&mut self, text: &str, xml_start: usize) {
        self.segments.push(CdataSegment {
            combined_start: self.text.len(),
            len: text.len(),
            xml_start,
        });
        self.text.push_str(text);
    }

    /// Append reconstructed text that has no counterpart in the XML document.
    fn push_synthetic(&mut self, text: &str) {
        self.text.push_str(text);
    }

    fn finish(self) -> (String, CdataOffsets) {
        (
            self.text,
            CdataOffsets {
                segments: self.segments,
            },
        )
    }
}

/// Parse TwinCAT XML files into an IronPLC Library
///
/// # Errors
///
/// Returns a `Diagnostic` if:
/// - The XML is malformed (P0006)
/// - The XML doesn't have valid TwinCAT structure (P0009)
/// - A non-ST implementation language is used (P9003)
pub fn parse(
    content: &str,
    file_id: &FileId,
    compiler_options: &CompilerOptions,
) -> Result<Library, Diagnostic> {
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

    // Find the POU, GVL, DUT, or Itf (interface) child element. Itf holds
    // a Beckhoff TwinCAT INTERFACE declaration — a separate object type
    // from POU, stored in its own .TcIO file (see
    // specs/plans/2026-07-18-twincat-extends-implements-interface.md).
    let object = root
        .children()
        .find(|n| n.is_element() && matches!(n.tag_name().name(), "POU" | "GVL" | "DUT" | "Itf"))
        .ok_or_else(|| {
            Diagnostic::problem(
                Problem::TwinCatMalformed,
                Label::file(
                    file_id.clone(),
                    "TcPlcObject must contain a POU, GVL, DUT, or Itf element".to_string(),
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
        // An interface's Declaration is just the header line (e.g.
        // `INTERFACE I_Drivable EXTENDS I_Base`) with no Implementation —
        // structurally identical to how parse_pou already handles a POU
        // with an absent Implementation element.
        "POU" | "Itf" => parse_pou(
            declaration_text,
            declaration_byte_offset,
            &object,
            file_id,
            compiler_options,
        ),
        "DUT" => parse_dut(
            declaration_text,
            declaration_byte_offset,
            file_id,
            compiler_options,
        ),
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
    compiler_options: &CompilerOptions,
) -> Result<Library, Diagnostic> {
    let (impl_text, impl_byte_offset) = extract_implementation(object, file_id)?;
    let closing = closing_keyword(&declaration_text);

    let mut builder = CombinedText::new();
    builder.push_cdata(&declaration_text, declaration_byte_offset);
    builder.push_synthetic("\n");
    match impl_byte_offset {
        Some(offset) => builder.push_cdata(&impl_text, offset),
        None => builder.push_synthetic(&impl_text),
    }

    // Methods follow the function block body and precede END_FUNCTION_BLOCK,
    // which is where `function_block_declaration` expects them.
    if closing == "END_FUNCTION_BLOCK" {
        append_methods(&mut builder, object, file_id)?;
    }

    builder.push_synthetic("\n");
    builder.push_synthetic(closing);

    let (combined, offsets) = builder.finish();
    debug!("POU combined ST ({} bytes)", combined.len());

    let result = st_parser::parse(&combined, file_id, compiler_options);

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
    compiler_options: &CompilerOptions,
) -> Result<Library, Diagnostic> {
    debug!("DUT declaration ST ({} bytes)", declaration_text.len());

    let offsets = CdataOffsets {
        segments: vec![CdataSegment {
            combined_start: 0,
            len: declaration_text.len(),
            xml_start: declaration_byte_offset,
        }],
    };

    let result = st_parser::parse(&declaration_text, file_id, compiler_options);

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

/// Map a byte offset in the combined ST text to a byte offset in the
/// original XML document.
///
/// A position inside a segment is shifted by that segment's CDATA offset. A
/// position in the synthetic text between or after segments (the joining
/// newlines, `END_METHOD`, the POU's closing keyword) has no counterpart in
/// the XML, so it maps to the end of the nearest preceding segment — the
/// closest real location the reader can be pointed at.
fn adjust_byte_offset(offsets: &CdataOffsets, pos: usize) -> usize {
    let mut preceding_end = None;

    for segment in &offsets.segments {
        if pos < segment.combined_start {
            break;
        }
        if pos <= segment.combined_start + segment.len {
            return segment.xml_start + (pos - segment.combined_start);
        }
        preceding_end = Some(segment.xml_start + segment.len);
    }

    // Position is past the end of a segment and before the next one starts.
    preceding_end.unwrap_or_else(|| {
        offsets
            .segments
            .first()
            .map_or(0, |segment| segment.xml_start)
    })
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
    } else if trimmed.len() >= 9 && trimmed[..9].eq_ignore_ascii_case("INTERFACE") {
        "END_INTERFACE"
    } else {
        // Fallback — the ST parser will report a more specific error
        ""
    }
}

/// Append every `<Method>` child of a POU to the combined text as an inline
/// `METHOD ... END_METHOD` declaration.
///
/// TwinCAT stores each method as a sibling `<Method>` element rather than
/// inline in the POU's own `<Declaration>`. A method element has the same
/// shape as the POU itself: a `<Declaration>` (which already begins with the
/// `METHOD` keyword and holds the signature and VAR blocks) and an optional
/// `<Implementation><ST>` with the body. Only the closing `END_METHOD` is
/// implicit in the XML structure and has to be reconstructed.
///
/// Only function block methods are appended. `method_declaration` is
/// reachable only from `function_block_declaration`, so a method on a
/// `PROGRAM`, a `FUNCTION`, or an interface has nowhere to go in the grammar
/// and is still dropped — see issue #1418.
fn append_methods(
    builder: &mut CombinedText,
    pou: &roxmltree::Node,
    file_id: &FileId,
) -> Result<(), Diagnostic> {
    for method in pou
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "Method")
    {
        let declaration = match find_child_element(&method, "Declaration") {
            Some(elem) => elem,
            None => {
                return Err(Diagnostic::problem(
                    Problem::TwinCatMalformed,
                    Label::file(
                        file_id.clone(),
                        format!(
                            "Method '{}' is missing required 'Declaration' element",
                            method.attribute("Name").unwrap_or("<unnamed>")
                        ),
                    ),
                ));
            }
        };

        let (declaration_text, declaration_byte_offset) = cdata_text_with_offset(&declaration);
        let (impl_text, impl_byte_offset) = extract_implementation(&method, file_id)?;

        builder.push_synthetic("\n");
        builder.push_cdata(&declaration_text, declaration_byte_offset);
        builder.push_synthetic("\n");
        match impl_byte_offset {
            Some(offset) => builder.push_cdata(&impl_text, offset),
            None => builder.push_synthetic(&impl_text),
        }

        // A method body must hold at least one statement, unlike a function
        // block body which may be empty. TwinCAT writes a do-nothing method
        // as a `<Method>` with no `<Implementation>` at all, so stand in an
        // empty statement; the parser discards it and the method keeps the
        // empty body it declares.
        if impl_text.trim().is_empty() {
            builder.push_synthetic(";");
        }

        builder.push_synthetic("\nEND_METHOD");
    }

    Ok(())
}

/// Extract the ST implementation text and its byte offset from an element
/// that has an `<Implementation>` child — a POU or one of its methods.
fn extract_implementation(
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
    use ironplc_dsl::common::{FunctionBlockDeclaration, LibraryElementKind, TypeName};
    use ironplc_dsl::core::{FileId, Id};

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

        let result = parse(xml, &test_file_id(), &CompilerOptions::default());
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

        let result = parse(xml, &test_file_id(), &CompilerOptions::default()).unwrap();

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

        let result = parse(xml, &test_file_id(), &CompilerOptions::default());
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

        let result = parse(xml, &test_file_id(), &CompilerOptions::default());
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
        let result = parse(xml, &file_id, &CompilerOptions::default());
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
        let result = parse(xml, &file_id, &CompilerOptions::default());
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn parse_when_malformed_xml_then_returns_p0006() {
        let xml = "NOT VALID XML <>";

        let result = parse(xml, &test_file_id(), &CompilerOptions::default());
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

        let result = parse(xml, &test_file_id(), &CompilerOptions::default());
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

        let result = parse(xml, &test_file_id(), &CompilerOptions::default());
        assert!(result.is_err());

        let diagnostic = result.unwrap_err();
        assert_eq!(diagnostic.code, "P0009");
        assert!(diagnostic.primary.message.contains("POU, GVL, DUT, or Itf"));
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

        let result = parse(xml, &test_file_id(), &CompilerOptions::default());
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

        let result = parse(xml, &test_file_id(), &CompilerOptions::default());
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

    #[test]
    fn closing_keyword_when_interface_then_returns_end_interface() {
        assert_eq!(closing_keyword("INTERFACE I_Drivable"), "END_INTERFACE");
    }

    #[test]
    fn closing_keyword_when_unknown_keyword_then_returns_empty() {
        // Unknown POU type — fallback returns empty string
        assert_eq!(closing_keyword("UNKNOWN_TYPE Something"), "");
    }

    #[test]
    fn closing_keyword_when_leading_whitespace_then_still_detects() {
        assert_eq!(
            closing_keyword("  PROGRAM MAIN\nVAR\nEND_VAR"),
            "END_PROGRAM"
        );
    }

    #[test]
    fn parse_when_pou_with_no_implementation_then_succeeds() {
        // POU with declaration only, no Implementation element
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="MAIN" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
    <Declaration><![CDATA[PROGRAM MAIN
VAR
    myVar : INT;
END_VAR]]></Declaration>
  </POU>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id(), &CompilerOptions::default());
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        let library = result.unwrap();
        assert_eq!(library.elements.len(), 1);
    }

    fn opts_with_fb_inheritance() -> CompilerOptions {
        CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        }
    }

    #[test]
    fn parse_when_itf_bare_interface_then_succeeds() {
        // Modeled on a real .TcIO file's structure (Beckhoff TwinCAT
        // interface, no base) — see
        // specs/plans/2026-07-18-twincat-extends-implements-interface.md.
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <Itf Name="I_Drivable" Id="{00000000-0000-0000-0000-000000000000}">
    <Declaration><![CDATA[INTERFACE I_Drivable
]]></Declaration>
  </Itf>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id(), &opts_with_fb_inheritance());
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        let library = result.unwrap();
        assert_eq!(library.elements.len(), 1);
        assert!(matches!(
            library.elements[0],
            LibraryElementKind::InterfaceDeclaration(_)
        ));
    }

    #[test]
    fn parse_when_itf_extends_base_interface_then_succeeds() {
        // Modeled on a real .TcIO file's structure (e.g. `I_Focus.TcIO`
        // extending `I_BaseAxis`).
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <Itf Name="I_Focus" Id="{00000000-0000-0000-0000-000000000000}">
    <Declaration><![CDATA[INTERFACE I_Focus EXTENDS I_BaseAxis]]></Declaration>
  </Itf>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id(), &opts_with_fb_inheritance());
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        let library = result.unwrap();
        let interface = match &library.elements[0] {
            LibraryElementKind::InterfaceDeclaration(decl) => decl,
            other => panic!("expected InterfaceDeclaration, got {other:?}"),
        };
        assert_eq!(interface.extends, vec![TypeName::from("I_BaseAxis")]);
    }

    #[test]
    fn parse_when_itf_and_default_dialect_then_err() {
        // Without allow_fb_inheritance, INTERFACE is just an identifier.
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <Itf Name="I_Drivable" Id="{00000000-0000-0000-0000-000000000000}">
    <Declaration><![CDATA[INTERFACE I_Drivable
]]></Declaration>
  </Itf>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id(), &CompilerOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn parse_when_pou_with_empty_implementation_then_succeeds() {
        // Implementation exists but has no ST or other language child
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="MAIN" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
    <Declaration><![CDATA[PROGRAM MAIN
VAR
    myVar : INT;
END_VAR]]></Declaration>
    <Implementation>
    </Implementation>
  </POU>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id(), &CompilerOptions::default());
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn parse_when_ld_implementation_then_returns_unsupported() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="MAIN" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
    <Declaration><![CDATA[PROGRAM MAIN
VAR
    myVar : INT;
END_VAR]]></Declaration>
    <Implementation>
      <LD/>
    </Implementation>
  </POU>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id(), &CompilerOptions::default());
        assert!(result.is_err());
        let diag = result.unwrap_err();
        assert_eq!(diag.code, Problem::XmlBodyTypeNotSupported.code());
        assert!(diag.primary.message.contains("LD"));
    }

    #[test]
    fn parse_when_il_implementation_then_returns_unsupported() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="MAIN" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
    <Declaration><![CDATA[PROGRAM MAIN
VAR
    myVar : INT;
END_VAR]]></Declaration>
    <Implementation>
      <IL/>
    </Implementation>
  </POU>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id(), &CompilerOptions::default());
        assert!(result.is_err());
        let diag = result.unwrap_err();
        assert_eq!(diag.code, Problem::XmlBodyTypeNotSupported.code());
        assert!(diag.primary.message.contains("IL"));
    }

    #[test]
    fn parse_when_sfc_implementation_then_returns_unsupported() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="MAIN" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
    <Declaration><![CDATA[PROGRAM MAIN
VAR
    myVar : INT;
END_VAR]]></Declaration>
    <Implementation>
      <SFC/>
    </Implementation>
  </POU>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id(), &CompilerOptions::default());
        assert!(result.is_err());
        let diag = result.unwrap_err();
        assert_eq!(diag.code, Problem::XmlBodyTypeNotSupported.code());
        assert!(diag.primary.message.contains("SFC"));
    }

    #[test]
    fn parse_when_dut_with_invalid_syntax_then_returns_error_with_adjusted_position() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <DUT Name="ST_Bad" Id="{00000000-0000-0000-0000-000000000000}">
    <Declaration><![CDATA[TYPE ST_Bad :
INVALID SYNTAX HERE
END_TYPE]]></Declaration>
  </DUT>
</TcPlcObject>"#;

        let file_id = FileId::from_string("test.TcDUT");
        let result = parse(xml, &file_id, &CompilerOptions::default());
        assert!(result.is_err());

        let diag = result.unwrap_err();
        // Position should point into the CDATA section in the XML
        let cdata_start = xml.find("<![CDATA[").unwrap() + "<![CDATA[".len();
        assert!(
            diag.primary.location.start >= cdata_start,
            "Error position {} should be >= CDATA start {}",
            diag.primary.location.start,
            cdata_start
        );
    }

    #[test]
    fn parse_when_pou_syntax_error_in_declaration_then_position_in_declaration() {
        // Syntax error in the declaration section (not implementation)
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="MAIN" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
    <Declaration><![CDATA[PROGRAM MAIN
VAR
    BAD SYNTAX : INT;
END_VAR]]></Declaration>
    <Implementation>
      <ST><![CDATA[x := 1;]]></ST>
    </Implementation>
  </POU>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id(), &CompilerOptions::default());
        assert!(result.is_err());

        let diag = result.unwrap_err();
        // Should point into the first CDATA (declaration)
        let decl_cdata_start = xml.find("<![CDATA[").unwrap() + "<![CDATA[".len();
        assert!(
            diag.primary.location.start >= decl_cdata_start,
            "Error position {} should be >= declaration CDATA start {}",
            diag.primary.location.start,
            decl_cdata_start
        );
    }

    /// A declaration of 50 bytes at XML offset 100, then a newline, then an
    /// implementation at XML offset 200 — the layout `parse_pou` builds for a
    /// POU that has an implementation.
    fn declaration_and_implementation_offsets() -> CdataOffsets {
        CdataOffsets {
            segments: vec![
                CdataSegment {
                    combined_start: 0,
                    len: 50,
                    xml_start: 100,
                },
                CdataSegment {
                    combined_start: 51,
                    len: 20,
                    xml_start: 200,
                },
            ],
        }
    }

    #[test]
    fn adjust_byte_offset_when_pos_in_implementation_then_adjusts_correctly() {
        let offsets = declaration_and_implementation_offsets();

        // Position 0 is in declaration: 0 + 100 = 100
        assert_eq!(adjust_byte_offset(&offsets, 0), 100);

        // Position 50 (= declaration length) is in declaration: 50 + 100 = 150
        assert_eq!(adjust_byte_offset(&offsets, 50), 150);

        // Position 52 is in implementation: (52 - 51) + 200 = 201
        assert_eq!(adjust_byte_offset(&offsets, 52), 201);
    }

    #[test]
    fn adjust_byte_offset_when_pos_past_last_segment_then_points_to_its_end() {
        let offsets = declaration_and_implementation_offsets();

        // Position 90 is in the synthetic closing keyword, past every
        // segment: it maps to the end of the implementation.
        assert_eq!(adjust_byte_offset(&offsets, 90), 220);
    }

    #[test]
    fn adjust_byte_offset_when_no_implementation_and_pos_past_declaration_then_points_to_end() {
        let offsets = CdataOffsets {
            segments: vec![CdataSegment {
                combined_start: 0,
                len: 50,
                xml_start: 100,
            }],
        };

        // Position beyond the declaration with no implementation: returns the
        // end of the declaration.
        assert_eq!(adjust_byte_offset(&offsets, 60), 150);
    }

    #[test]
    fn adjust_byte_offset_when_no_segments_then_returns_zero() {
        let offsets = CdataOffsets { segments: vec![] };

        assert_eq!(adjust_byte_offset(&offsets, 0), 0);
        assert_eq!(adjust_byte_offset(&offsets, 42), 0);
    }

    #[test]
    fn parse_when_pou_with_function_declaration_then_succeeds() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="FC_Add" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
    <Declaration><![CDATA[FUNCTION FC_Add : INT
VAR_INPUT
    a : INT;
    b : INT;
END_VAR]]></Declaration>
    <Implementation>
      <ST><![CDATA[FC_Add := a + b;]]></ST>
    </Implementation>
  </POU>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id(), &CompilerOptions::default());
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        let library = result.unwrap();
        assert_eq!(library.elements.len(), 1);
    }

    /// The `<Method>` shape TwinCAT writes into a `.TcPOU`: the method's own
    /// `<Declaration>` already carries the `METHOD` keyword and the signature,
    /// and the body lives in a sibling `<Implementation><ST>`.
    const FB_WITH_METHOD: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="FB_Motor" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
    <Declaration><![CDATA[FUNCTION_BLOCK FB_Motor
VAR
    speed : REAL;
END_VAR]]></Declaration>
    <Implementation>
      <ST><![CDATA[speed := 0.0;]]></ST>
    </Implementation>
    <Method Name="SetSpeed" Id="{00000000-0000-0000-0000-000000000001}">
      <Declaration><![CDATA[METHOD SetSpeed
VAR_INPUT
    value : REAL;
END_VAR]]></Declaration>
      <Implementation>
        <ST><![CDATA[speed := value;]]></ST>
      </Implementation>
    </Method>
  </POU>
</TcPlcObject>"#;

    /// Extract the single function block from a library, or panic describing
    /// what was found instead.
    fn only_function_block(library: Library) -> FunctionBlockDeclaration {
        assert_eq!(library.elements.len(), 1);
        match library.elements.into_iter().next() {
            Some(LibraryElementKind::FunctionBlockDeclaration(decl)) => decl,
            other => panic!("expected FunctionBlockDeclaration, got {other:?}"),
        }
    }

    /// Collect every source span in a library.
    fn collect_spans(library: Library) -> Vec<SourceSpan> {
        struct SpanCollector<'a> {
            spans: &'a mut Vec<SourceSpan>,
        }
        impl Fold<()> for SpanCollector<'_> {
            fn fold_source_span(&mut self, node: SourceSpan) -> Result<SourceSpan, ()> {
                self.spans.push(node.clone());
                Ok(node)
            }
        }

        let mut spans = Vec::new();
        let mut collector = SpanCollector { spans: &mut spans };
        let _ = collector.fold_library(library);
        spans
    }

    #[test]
    fn parse_when_pou_with_method_element_then_method_is_declared() {
        let result = parse(FB_WITH_METHOD, &test_file_id(), &opts_with_fb_inheritance());
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());

        let function_block = only_function_block(result.unwrap());
        assert_eq!(function_block.methods.len(), 1);
        assert_eq!(function_block.methods[0].name, Id::from("SetSpeed"));
        // VAR_INPUT of the method, not of the enclosing function block.
        assert_eq!(function_block.methods[0].variables.len(), 1);
        assert_eq!(function_block.variables.len(), 1);
    }

    #[test]
    fn parse_when_pou_with_multiple_method_elements_then_all_kept_in_document_order() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="FB_Motor" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
    <Declaration><![CDATA[FUNCTION_BLOCK FB_Motor
VAR
    speed : REAL;
END_VAR]]></Declaration>
    <Implementation>
      <ST><![CDATA[speed := 0.0;]]></ST>
    </Implementation>
    <Method Name="Start" Id="{00000000-0000-0000-0000-000000000001}">
      <Declaration><![CDATA[METHOD Start]]></Declaration>
      <Implementation>
        <ST><![CDATA[speed := 1.0;]]></ST>
      </Implementation>
    </Method>
    <Method Name="Stop" Id="{00000000-0000-0000-0000-000000000002}">
      <Declaration><![CDATA[METHOD Stop]]></Declaration>
      <Implementation>
        <ST><![CDATA[speed := 0.0;]]></ST>
      </Implementation>
    </Method>
  </POU>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id(), &opts_with_fb_inheritance());
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());

        let function_block = only_function_block(result.unwrap());
        let names: Vec<&Id> = function_block.methods.iter().map(|m| &m.name).collect();
        assert_eq!(names, vec![&Id::from("Start"), &Id::from("Stop")]);
    }

    #[test]
    fn parse_when_method_declares_return_type_then_return_type_is_kept() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="FB_Motor" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
    <Declaration><![CDATA[FUNCTION_BLOCK FB_Motor
VAR
    speed : REAL;
END_VAR]]></Declaration>
    <Implementation>
      <ST><![CDATA[speed := 0.0;]]></ST>
    </Implementation>
    <Method Name="IsRunning" Id="{00000000-0000-0000-0000-000000000001}">
      <Declaration><![CDATA[METHOD IsRunning : BOOL]]></Declaration>
      <Implementation>
        <ST><![CDATA[IsRunning := speed > 0.0;]]></ST>
      </Implementation>
    </Method>
  </POU>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id(), &opts_with_fb_inheritance());
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());

        let function_block = only_function_block(result.unwrap());
        assert_eq!(function_block.methods.len(), 1);
        assert!(function_block.methods[0].return_type.is_some());
    }

    #[test]
    fn parse_when_method_has_no_implementation_then_method_has_empty_body() {
        // TwinCAT writes a method with an empty body as a `<Method>` with no
        // `<Implementation>` child at all.
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="FB_Motor" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
    <Declaration><![CDATA[FUNCTION_BLOCK FB_Motor
VAR
    speed : REAL;
END_VAR]]></Declaration>
    <Implementation>
      <ST><![CDATA[speed := 0.0;]]></ST>
    </Implementation>
    <Method Name="Reset" Id="{00000000-0000-0000-0000-000000000001}">
      <Declaration><![CDATA[METHOD Reset]]></Declaration>
    </Method>
  </POU>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id(), &opts_with_fb_inheritance());
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());

        let function_block = only_function_block(result.unwrap());
        assert_eq!(function_block.methods.len(), 1);
        assert!(function_block.methods[0].body.is_empty());
    }

    #[test]
    fn parse_when_pou_with_method_then_method_spans_point_into_method_cdata() {
        let result = parse(FB_WITH_METHOD, &test_file_id(), &opts_with_fb_inheritance());
        // Restrict to the method's own spans by measuring against the XML
        // region the `<Method>` element occupies.
        let method_element_start = FB_WITH_METHOD.find("<Method ").unwrap();
        let method_spans: Vec<SourceSpan> = collect_spans(result.unwrap())
            .into_iter()
            .filter(|span| span.start >= method_element_start)
            .collect();

        assert!(
            !method_spans.is_empty(),
            "expected spans inside the <Method> element"
        );

        // `value` is declared only inside the method, so its span must fall
        // inside the method's declaration CDATA rather than the POU's.
        let value_offset = FB_WITH_METHOD.find("value : REAL").unwrap();
        assert!(
            method_spans.iter().any(|span| span.start == value_offset),
            "expected a span at the method's `value` declaration ({value_offset})"
        );

        // The method body assignment must map into the method's own ST CDATA.
        let body_offset = FB_WITH_METHOD.find("speed := value;").unwrap();
        assert!(
            method_spans.iter().any(|span| span.start == body_offset),
            "expected a span at the method's body ({body_offset})"
        );
    }

    #[test]
    fn parse_when_method_body_has_syntax_error_then_position_points_into_method_cdata() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="FB_Motor" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
    <Declaration><![CDATA[FUNCTION_BLOCK FB_Motor
VAR
    speed : REAL;
END_VAR]]></Declaration>
    <Implementation>
      <ST><![CDATA[speed := 0.0;]]></ST>
    </Implementation>
    <Method Name="SetSpeed" Id="{00000000-0000-0000-0000-000000000001}">
      <Declaration><![CDATA[METHOD SetSpeed]]></Declaration>
      <Implementation>
        <ST><![CDATA[INVALID SYNTAX HERE !!!]]></ST>
      </Implementation>
    </Method>
  </POU>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id(), &opts_with_fb_inheritance());
        assert!(result.is_err());

        let diagnostic = result.unwrap_err();
        let method_body_start = xml.find("INVALID").unwrap();
        assert!(
            diagnostic.primary.location.start >= method_body_start,
            "Error position {} should be >= method body start {} (pointing into the method's CDATA)",
            diagnostic.primary.location.start,
            method_body_start
        );
    }

    #[test]
    fn parse_when_method_uses_fbd_implementation_then_returns_p9003() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="FB_Motor" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
    <Declaration><![CDATA[FUNCTION_BLOCK FB_Motor
VAR
    speed : REAL;
END_VAR]]></Declaration>
    <Implementation>
      <ST><![CDATA[speed := 0.0;]]></ST>
    </Implementation>
    <Method Name="SetSpeed" Id="{00000000-0000-0000-0000-000000000001}">
      <Declaration><![CDATA[METHOD SetSpeed]]></Declaration>
      <Implementation>
        <FBD/>
      </Implementation>
    </Method>
  </POU>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id(), &opts_with_fb_inheritance());
        assert!(result.is_err());

        let diagnostic = result.unwrap_err();
        assert_eq!(diagnostic.code, Problem::XmlBodyTypeNotSupported.code());
        assert!(diagnostic.primary.message.contains("FBD"));
    }

    #[test]
    fn parse_when_method_missing_declaration_then_returns_p0009() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="FB_Motor" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
    <Declaration><![CDATA[FUNCTION_BLOCK FB_Motor
VAR
    speed : REAL;
END_VAR]]></Declaration>
    <Implementation>
      <ST><![CDATA[speed := 0.0;]]></ST>
    </Implementation>
    <Method Name="SetSpeed" Id="{00000000-0000-0000-0000-000000000001}">
      <Implementation>
        <ST><![CDATA[speed := 1.0;]]></ST>
      </Implementation>
    </Method>
  </POU>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id(), &opts_with_fb_inheritance());
        assert!(result.is_err());

        let diagnostic = result.unwrap_err();
        assert_eq!(diagnostic.code, "P0009");
        assert!(diagnostic.primary.message.contains("SetSpeed"));
    }

    #[test]
    fn parse_when_pou_with_method_and_default_dialect_then_err() {
        // Without allow_fb_inheritance, METHOD is demoted to an identifier.
        // The methods are no longer silently dropped, so the file reports the
        // unsupported syntax instead of parsing as a method-less type.
        let result = parse(FB_WITH_METHOD, &test_file_id(), &CompilerOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn parse_when_program_pou_with_method_element_then_method_is_dropped() {
        // `method_declaration` is reachable only from
        // `function_block_declaration`, so a method on a PROGRAM has nowhere
        // to go in the grammar and is left alone rather than turned into a
        // parse error. See issue #1418.
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="MAIN" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
    <Declaration><![CDATA[PROGRAM MAIN
VAR
    speed : REAL;
END_VAR]]></Declaration>
    <Implementation>
      <ST><![CDATA[speed := 0.0;]]></ST>
    </Implementation>
    <Method Name="SetSpeed" Id="{00000000-0000-0000-0000-000000000001}">
      <Declaration><![CDATA[METHOD SetSpeed]]></Declaration>
      <Implementation>
        <ST><![CDATA[speed := 1.0;]]></ST>
      </Implementation>
    </Method>
  </POU>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id(), &opts_with_fb_inheritance());
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn parse_when_itf_with_method_element_then_method_is_dropped() {
        // `interface_declaration` parses the header only, so an interface's
        // `<Method>` children are still ignored. Appending them would turn
        // every real `.TcIO` file into a parse error.
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <Itf Name="I_Drivable" Id="{00000000-0000-0000-0000-000000000000}">
    <Declaration><![CDATA[INTERFACE I_Drivable
]]></Declaration>
    <Method Name="Start" Id="{00000000-0000-0000-0000-000000000001}">
      <Declaration><![CDATA[METHOD Start : BOOL]]></Declaration>
    </Method>
  </Itf>
</TcPlcObject>"#;

        let result = parse(xml, &test_file_id(), &opts_with_fb_inheritance());
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        let library = result.unwrap();
        assert!(matches!(
            library.elements[0],
            LibraryElementKind::InterfaceDeclaration(_)
        ));
    }
}
