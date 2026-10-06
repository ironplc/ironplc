//! `<Method>` and `<Property>` elements of an `<Itf>` (a `.TcIO` file): each
//! is rebuilt as a method or property prototype, without a body.

use super::tests::test_file_id;
use super::*;
use ironplc_dsl::common::{InterfaceDeclaration, LibraryElementKind};
use ironplc_dsl::core::{Id, Located};
use ironplc_parser::options::Dialect;

/// TwinCAT writes an empty `VAR_INPUT END_VAR` into most interface method
/// declarations, so these files need the dialect's `allow_empty_var_blocks`
/// as well as `allow_fb_inheritance`.
fn twincat() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::TwinCat)
}

/// Wraps member elements in the `<Itf>` TwinCAT writes.
fn itf_with_members(members: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <Itf Name="I_Brake" Id="{{00000000-0000-0000-0000-000000000000}}">
    <Declaration><![CDATA[INTERFACE I_Brake
]]></Declaration>
{members}
  </Itf>
</TcPlcObject>"#
    )
}

/// The shapes TwinCAT writes: an accessor's `<Declaration>` is empty, and no
/// member has an `<Implementation>`.
const BRAKE_OPEN_PROPERTY: &str = r#"    <Property Name="BrakeOpen" Id="{00000000-0000-0000-0000-000000000001}">
      <Declaration><![CDATA[PROPERTY BrakeOpen : BOOL]]></Declaration>
      <Get Name="Get" Id="{00000000-0000-0000-0000-000000000002}">
        <Declaration><![CDATA[]]></Declaration>
      </Get>
    </Property>"#;

const CLOSE_BRAKE_METHOD: &str = r#"    <Method Name="CloseBrake" Id="{00000000-0000-0000-0000-000000000003}">
      <Declaration><![CDATA[METHOD CloseBrake : BOOL
VAR_INPUT
    force : REAL;
END_VAR
]]></Declaration>
    </Method>"#;

fn only_interface(library: Library) -> InterfaceDeclaration {
    match library.elements.into_iter().next() {
        Some(LibraryElementKind::InterfaceDeclaration(itf)) => itf,
        other => panic!("expected an interface, got {other:?}"),
    }
}

#[test]
fn parse_when_itf_with_method_element_then_method_prototype_kept() {
    let xml = itf_with_members(CLOSE_BRAKE_METHOD);
    let result = parse(&xml, &test_file_id(), &twincat());
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());

    let itf = only_interface(result.unwrap());
    assert_eq!(itf.methods.len(), 1);
    let method = &itf.methods[0];
    assert_eq!(method.name, Id::from("CloseBrake"));
    assert!(method.return_type.is_some());
    assert_eq!(method.variables.len(), 1);
}

#[test]
fn parse_when_itf_with_get_only_property_then_property_prototype_kept() {
    let xml = itf_with_members(BRAKE_OPEN_PROPERTY);
    let result = parse(&xml, &test_file_id(), &twincat());
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());

    let itf = only_interface(result.unwrap());
    let property = &itf.properties[0];
    assert_eq!(property.name, Id::from("BrakeOpen"));
    assert!(property.get.is_some());
    assert!(property.set.is_none());
}

#[test]
fn parse_when_itf_members_interleave_then_all_kept_in_document_order() {
    let open_method = CLOSE_BRAKE_METHOD
        .replace("CloseBrake", "OpenBrake")
        .replace("-000000000003", "-000000000004");
    let xml = itf_with_members(&format!(
        "{BRAKE_OPEN_PROPERTY}\n{CLOSE_BRAKE_METHOD}\n{open_method}"
    ));
    let itf = only_interface(parse(&xml, &test_file_id(), &twincat()).unwrap());

    let methods: Vec<_> = itf.methods.iter().map(|m| m.name.to_string()).collect();
    assert_eq!(methods, vec!["CloseBrake", "OpenBrake"]);
    assert_eq!(itf.properties.len(), 1);
}

#[test]
fn parse_when_itf_method_then_span_points_into_method_cdata() {
    let xml = itf_with_members(CLOSE_BRAKE_METHOD);
    let itf = only_interface(parse(&xml, &test_file_id(), &twincat()).unwrap());

    let name_start = itf.methods[0].name.span().start;
    assert_eq!(
        &xml[name_start..name_start + "CloseBrake".len()],
        "CloseBrake"
    );
}

#[test]
fn parse_when_itf_method_has_implementation_then_returns_p0009() {
    let with_body = CLOSE_BRAKE_METHOD.replace(
        "    </Method>",
        "      <Implementation>\n        <ST><![CDATA[CloseBrake := TRUE;]]></ST>\n      </Implementation>\n    </Method>",
    );
    let diagnostic = parse(&itf_with_members(&with_body), &test_file_id(), &twincat()).unwrap_err();
    assert_eq!(diagnostic.code, "P0009");
    assert!(diagnostic.primary.message.contains("Method 'CloseBrake'"));
}

#[test]
fn parse_when_itf_get_has_implementation_then_returns_p0009() {
    let with_body = BRAKE_OPEN_PROPERTY.replace(
        "      </Get>",
        "        <Implementation>\n          <ST><![CDATA[BrakeOpen := TRUE;]]></ST>\n        </Implementation>\n      </Get>",
    );
    let diagnostic = parse(&itf_with_members(&with_body), &test_file_id(), &twincat()).unwrap_err();
    assert_eq!(diagnostic.code, "P0009");
    assert!(diagnostic.primary.message.contains("Get 'Get'"));
}

#[test]
fn parse_when_itf_method_has_empty_implementation_then_ok() {
    let with_empty_body = CLOSE_BRAKE_METHOD.replace(
        "    </Method>",
        "      <Implementation>\n        <ST><![CDATA[\n]]></ST>\n      </Implementation>\n    </Method>",
    );
    let result = parse(
        &itf_with_members(&with_empty_body),
        &test_file_id(),
        &twincat(),
    );
    assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
}

#[test]
fn parse_when_itf_get_declares_variables_then_error_points_into_get_cdata() {
    let with_vars = BRAKE_OPEN_PROPERTY.replace(
        "<Declaration><![CDATA[]]></Declaration>",
        "<Declaration><![CDATA[VAR\n    tmp : BOOL;\nEND_VAR]]></Declaration>",
    );
    let xml = itf_with_members(&with_vars);
    let diagnostic = parse(&xml, &test_file_id(), &twincat()).unwrap_err();

    assert_eq!(diagnostic.code, "P0002");
    let get_declaration_start = xml.find("VAR\n    tmp").unwrap();
    assert!(
        diagnostic.primary.location.start >= get_declaration_start,
        "Error position {} should be >= GET declaration start {get_declaration_start}",
        diagnostic.primary.location.start,
    );
}
