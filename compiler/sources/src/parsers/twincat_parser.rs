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
//! function block body, where the grammar expects them. Properties follow the
//! same path: a `<Property>` holds its own `<Declaration>` and a `<Get>` and/or
//! `<Set>` element, each shaped like a method, and is reconstructed as
//! `PROPERTY ... GET ... END_GET SET ... END_SET END_PROPERTY`.
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
    // specs/design/beckhoff-twincat-dialect.md §1.3).
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

    // Methods and properties follow the function block body and precede
    // END_FUNCTION_BLOCK, which is where `function_block_declaration` expects
    // them.
    if closing == "END_FUNCTION_BLOCK" {
        append_members(&mut builder, object, file_id)?;
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

/// The keywords that open a POU declaration, each with its closing keyword.
/// `FUNCTION_BLOCK` comes before `FUNCTION` since `FUNCTION` is a prefix.
const POU_KEYWORDS: [(&str, &str); 4] = [
    ("FUNCTION_BLOCK", "END_FUNCTION_BLOCK"),
    ("FUNCTION", "END_FUNCTION"),
    ("PROGRAM", "END_PROGRAM"),
    ("INTERFACE", "END_INTERFACE"),
];

/// Detect the POU type from the declaration text and return the closing keyword.
fn closing_keyword(declaration: &str) -> &'static str {
    let header = skip_leading_trivia(declaration);
    POU_KEYWORDS
        .iter()
        .find(|(open, _)| {
            header
                .get(..open.len())
                .is_some_and(|start| start.eq_ignore_ascii_case(open))
        })
        // Fallback — the ST parser will report a more specific error
        .map_or("", |(_, close)| close)
}

/// Skip the whitespace, comments and pragmas before the POU keyword, such as
/// a header comment or `{attribute 'hide'}`.
///
/// The comment forms are those of the lexer's `Comment` token, none of which
/// nest. An unterminated comment or pragma stops the skipping, so no keyword
/// is found and the ST parser reports the problem.
fn skip_leading_trivia(text: &str) -> &str {
    const DELIMITED: [(&str, &str); 3] = [("(*", "*)"), ("/*", "*/"), ("{", "}")];

    let mut rest = text.trim_start();
    loop {
        let after = if let Some(comment) = rest.strip_prefix("//") {
            Some(comment.find('\n').map_or("", |end| &comment[end..]))
        } else {
            DELIMITED.iter().find_map(|(open, close)| {
                let body = rest.strip_prefix(open)?;
                body.find(close).map(|end| &body[end + close.len()..])
            })
        };
        match after {
            Some(after) => rest = after.trim_start(),
            None => return rest,
        }
    }
}

/// Append every `<Method>` and `<Property>` child of a POU to the combined
/// text, in document order, as inline `METHOD ... END_METHOD` and
/// `PROPERTY ... END_PROPERTY` declarations.
///
/// TwinCAT stores each method and property as a sibling element rather than
/// inline in the POU's own `<Declaration>`. A method element has the same
/// shape as the POU itself: a `<Declaration>` (which already begins with the
/// `METHOD` keyword and holds the signature and VAR blocks) and an optional
/// `<Implementation><ST>` with the body. Only the closing `END_METHOD` is
/// implicit in the XML structure and has to be reconstructed.
///
/// Only function block members are appended. `method_declaration` and
/// `property_declaration` are reachable only from
/// `function_block_declaration`, so a member of a `PROGRAM`, a `FUNCTION`, or
/// an interface has nowhere to go in the grammar and is still dropped.
fn append_members(
    builder: &mut CombinedText,
    pou: &roxmltree::Node,
    file_id: &FileId,
) -> Result<(), Diagnostic> {
    for member in pou.children().filter(|n| n.is_element()) {
        match member.tag_name().name() {
            "Method" => append_declared_block(builder, &member, "Method", "END_METHOD", file_id)?,
            "Property" => append_property(builder, &member, file_id)?,
            _ => {}
        }
    }

    Ok(())
}

/// Append one `<Property>` element.
///
/// Its `<Declaration>` holds only the `PROPERTY name : type` header. Each
/// accessor is a `<Get>` or `<Set>` child shaped like a method, whose
/// `<Declaration>` holds only its VAR blocks, so the opening `GET`/`SET`
/// keyword is implicit as well as the closing one.
fn append_property(
    builder: &mut CombinedText,
    property: &roxmltree::Node,
    file_id: &FileId,
) -> Result<(), Diagnostic> {
    let declaration = required_declaration(property, "Property", file_id)?;
    let (declaration_text, declaration_byte_offset) = cdata_text_with_offset(&declaration);
    builder.push_synthetic("\n");
    builder.push_cdata(&declaration_text, declaration_byte_offset);

    for (tag, opening, closing) in [("Get", "GET", "END_GET"), ("Set", "SET", "END_SET")] {
        if let Some(accessor) = find_child_element(property, tag) {
            builder.push_synthetic("\n");
            builder.push_synthetic(opening);
            append_declared_block(builder, &accessor, tag, closing, file_id)?;
        }
    }

    builder.push_synthetic("\nEND_PROPERTY");
    Ok(())
}

/// Append one element that carries a `<Declaration>` and an optional
/// `<Implementation><ST>`, followed by the `closing` keyword that the XML
/// structure leaves implicit.
///
/// `kind` names the element in the diagnostic when its `<Declaration>` is
/// missing.
fn append_declared_block(
    builder: &mut CombinedText,
    element: &roxmltree::Node,
    kind: &str,
    closing: &str,
    file_id: &FileId,
) -> Result<(), Diagnostic> {
    let declaration = required_declaration(element, kind, file_id)?;
    let (declaration_text, declaration_byte_offset) = cdata_text_with_offset(&declaration);
    let (impl_text, impl_byte_offset) = extract_implementation(element, file_id)?;

    builder.push_synthetic("\n");
    builder.push_cdata(&declaration_text, declaration_byte_offset);
    builder.push_synthetic("\n");
    match impl_byte_offset {
        Some(offset) => builder.push_cdata(&impl_text, offset),
        None => builder.push_synthetic(&impl_text),
    }

    builder.push_synthetic("\n");
    builder.push_synthetic(closing);

    Ok(())
}

/// The `<Declaration>` child of `element`, which TwinCAT always writes.
/// `kind` names the element in the diagnostic when it is missing.
fn required_declaration<'a>(
    element: &'a roxmltree::Node,
    kind: &str,
    file_id: &FileId,
) -> Result<roxmltree::Node<'a, 'a>, Diagnostic> {
    find_child_element(element, "Declaration").ok_or_else(|| {
        Diagnostic::problem(
            Problem::TwinCatMalformed,
            Label::file(
                file_id.clone(),
                format!(
                    "{kind} '{}' is missing required 'Declaration' element",
                    element.attribute("Name").unwrap_or("<unnamed>")
                ),
            ),
        )
    })
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
mod tests;

#[cfg(test)]
mod property_tests;
