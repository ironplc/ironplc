//! Tests for the TwinCAT XML parser.
//!
//! Split out of `twincat_parser.rs` to keep that module within the project's
//! 1000-line limit.

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
    // specs/design/beckhoff-twincat-dialect.md §1.3.
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
    // parse error.
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
