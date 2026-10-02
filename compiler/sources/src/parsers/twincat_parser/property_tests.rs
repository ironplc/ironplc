//! `<Property>` elements in a `.TcPOU`: each is rebuilt as
//! `PROPERTY ... END_PROPERTY` with `GET`/`SET` accessors from its `<Get>`
//! and `<Set>` children.

use super::tests::{only_function_block, test_file_id};
use super::*;
use ironplc_dsl::core::Id;
use ironplc_parser::options::Dialect;

/// TwinCAT writes an empty `VAR END_VAR` into an accessor's declaration,
/// so these files need the dialect's `allow_empty_var_blocks` as well as
/// `allow_fb_inheritance`.
fn twincat() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::TwinCat)
}

/// Wraps member elements in the function block POU TwinCAT writes.
fn fb_with_members(members: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="FB_Motor" Id="{{00000000-0000-0000-0000-000000000000}}" SpecialFunc="None">
    <Declaration><![CDATA[FUNCTION_BLOCK FB_Motor
VAR
    speed : REAL;
END_VAR]]></Declaration>
    <Implementation>
      <ST><![CDATA[speed := 0.0;]]></ST>
    </Implementation>
{members}
  </POU>
</TcPlcObject>"#
    )
}

/// The `<Property>` shape TwinCAT writes: the property's `<Declaration>`
/// holds only the header, and each accessor's `<Declaration>` only its VAR
/// blocks.
const SPEED_PROPERTY: &str = r#"    <Property Name="Speed" Id="{00000000-0000-0000-0000-000000000001}">
      <Declaration><![CDATA[PROPERTY Speed : REAL]]></Declaration>
      <Get Name="Get" Id="{00000000-0000-0000-0000-000000000002}">
        <Declaration><![CDATA[VAR
END_VAR]]></Declaration>
        <Implementation>
          <ST><![CDATA[Speed := speed;]]></ST>
        </Implementation>
      </Get>
      <Set Name="Set" Id="{00000000-0000-0000-0000-000000000003}">
        <Declaration><![CDATA[VAR
END_VAR]]></Declaration>
        <Implementation>
          <ST><![CDATA[speed := Speed;]]></ST>
        </Implementation>
      </Set>
    </Property>"#;

const START_METHOD: &str = r#"    <Method Name="Start" Id="{00000000-0000-0000-0000-000000000004}">
      <Declaration><![CDATA[METHOD Start]]></Declaration>
      <Implementation>
        <ST><![CDATA[speed := 1.0;]]></ST>
      </Implementation>
    </Method>"#;

#[test]
fn parse_when_pou_with_property_element_then_property_has_both_accessors() {
    let xml = fb_with_members(SPEED_PROPERTY);
    let result = parse(&xml, &test_file_id(), &twincat());
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());

    let function_block = only_function_block(result.unwrap());
    assert_eq!(function_block.properties.len(), 1);
    let property = &function_block.properties[0];
    assert_eq!(property.name, Id::from("Speed"));
    assert_eq!(property.get.as_ref().unwrap().body.len(), 1);
    assert_eq!(property.set.as_ref().unwrap().body.len(), 1);
}

#[test]
fn parse_when_methods_and_property_interleave_then_all_are_kept() {
    let xml = fb_with_members(&format!(
        "{START_METHOD}\n{SPEED_PROPERTY}\n{}",
        START_METHOD
            .replace("\"Start\"", "\"Stop\"")
            .replace("METHOD Start", "METHOD Stop")
    ));
    let result = parse(&xml, &test_file_id(), &twincat());
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());

    let function_block = only_function_block(result.unwrap());
    let methods: Vec<_> = function_block
        .methods
        .iter()
        .map(|m| m.name.to_string())
        .collect();
    assert_eq!(methods, ["Start", "Stop"]);
    assert_eq!(function_block.properties.len(), 1);
}

/// TwinCAT writes an accessor with an empty body without an
/// `<Implementation>`, as it does for a method.
#[test]
fn parse_when_get_has_no_implementation_and_no_set_then_get_has_empty_body() {
    let xml = fb_with_members(
        r#"    <Property Name="Speed" Id="{00000000-0000-0000-0000-000000000001}">
      <Declaration><![CDATA[PROPERTY Speed : REAL]]></Declaration>
      <Get Name="Get" Id="{00000000-0000-0000-0000-000000000002}">
        <Declaration><![CDATA[VAR
END_VAR]]></Declaration>
      </Get>
    </Property>"#,
    );
    let result = parse(&xml, &test_file_id(), &twincat());
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());

    let property = &only_function_block(result.unwrap()).properties[0];
    assert!(property.get.as_ref().unwrap().body.is_empty());
    assert!(property.set.is_none());
}

#[test]
fn parse_when_set_body_has_syntax_error_then_position_points_into_set_cdata() {
    let xml = fb_with_members(&SPEED_PROPERTY.replace("speed := Speed;", "INVALID SYNTAX !!!"));
    let result = parse(&xml, &test_file_id(), &twincat());

    let diagnostic = result.unwrap_err();
    let set_body_start = xml.find("INVALID").unwrap();
    assert!(
        diagnostic.primary.location.start >= set_body_start,
        "Error position {} should be >= SET body start {set_body_start}",
        diagnostic.primary.location.start,
    );
}

#[test]
fn parse_when_property_missing_declaration_then_returns_p0009() {
    let xml = fb_with_members(&SPEED_PROPERTY.replace(
        "      <Declaration><![CDATA[PROPERTY Speed : REAL]]></Declaration>\n",
        "",
    ));
    let diagnostic = parse(&xml, &test_file_id(), &twincat()).unwrap_err();
    assert_eq!(diagnostic.code, "P0009");
    assert!(diagnostic.primary.message.contains("Property 'Speed'"));
}

#[test]
fn parse_when_get_missing_declaration_then_returns_p0009() {
    let without_get_declaration = SPEED_PROPERTY.replacen(
        "        <Declaration><![CDATA[VAR\nEND_VAR]]></Declaration>\n",
        "",
        1,
    );
    let xml = fb_with_members(&without_get_declaration);
    let diagnostic = parse(&xml, &test_file_id(), &twincat()).unwrap_err();
    assert_eq!(diagnostic.code, "P0009");
    assert!(diagnostic.primary.message.contains("Get 'Get'"));
}
