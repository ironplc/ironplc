//! OOP extension: the method and property prototypes an INTERFACE declares.
//! See `MethodPrototype` and `PropertyPrototype`.

use super::common::*;
use dsl::common::InterfaceDeclaration;

fn parse_interface(source: &str) -> InterfaceDeclaration {
    let library = parse_program(source, &FileId::default(), &opts_with_fb_inheritance()).unwrap();
    let element = library
        .elements
        .iter()
        .find(|e| matches!(e, LibraryElementKind::InterfaceDeclaration(_)))
        .unwrap();
    cast!(element, LibraryElementKind::InterfaceDeclaration).clone()
}

fn parse_err(source: &str) -> bool {
    parse_program(source, &FileId::default(), &opts_with_fb_inheritance()).is_err()
}

#[test]
fn parse_when_interface_without_members_then_no_members() {
    let itf = parse_interface(
        "
INTERFACE I_Drivable
END_INTERFACE",
    );
    assert!(itf.methods.is_empty());
    assert!(itf.properties.is_empty());
}

#[test]
fn parse_when_method_prototype_with_return_type_and_inputs_then_signature_kept() {
    let itf = parse_interface(
        "
INTERFACE I_Brake
METHOD CloseBrake : BOOL
VAR_INPUT
    force : REAL;
END_VAR
VAR_OUTPUT
    done : BOOL;
END_VAR
VAR_IN_OUT
    count : INT;
END_VAR
END_METHOD
END_INTERFACE",
    );
    assert_eq!(itf.methods.len(), 1);
    let method = &itf.methods[0];
    assert_eq!(method.name, Id::from("CloseBrake"));
    assert!(method.return_type.is_some());
    let kinds: Vec<_> = method
        .variables
        .iter()
        .map(|v| v.var_type.clone())
        .collect();
    assert_eq!(
        kinds,
        vec![
            VariableType::Input,
            VariableType::Output,
            VariableType::InOut
        ]
    );
}

#[test]
fn parse_when_method_prototype_without_return_type_or_variables_then_ok() {
    let itf = parse_interface(
        "
INTERFACE I_Brake
METHOD Reset
END_METHOD
END_INTERFACE",
    );
    let method = &itf.methods[0];
    assert_eq!(method.name, Id::from("Reset"));
    assert!(method.return_type.is_none());
    assert!(method.variables.is_empty());
}

#[test]
fn parse_when_method_prototype_has_qualifiers_then_qualifiers_kept() {
    let itf = parse_interface(
        "
INTERFACE I_Telescope
METHOD PUBLIC ABSTRACT Park : BOOL
END_METHOD
END_INTERFACE",
    );
    let method = &itf.methods[0];
    assert_eq!(method.name, Id::from("Park"));
    assert_eq!(method.qualifiers.iter().count(), 2);
    assert!(method.qualifiers.is_abstract());
}

#[test]
fn parse_when_method_prototype_named_like_qualifier_then_name_kept() {
    let itf = parse_interface(
        "
INTERFACE I_Brake
METHOD Final : BOOL
END_METHOD
END_INTERFACE",
    );
    let method = &itf.methods[0];
    assert_eq!(method.name, Id::from("Final"));
    assert!(method.qualifiers.is_empty());
}

#[test]
fn parse_when_property_prototypes_then_accessors_recorded() {
    let itf = parse_interface(
        "
INTERFACE I_Axis
PROPERTY Position : LREAL
GET
END_GET
END_PROPERTY
PROPERTY Target : LREAL
SET END_SET
END_PROPERTY
PROPERTY Speed : LREAL
GET END_GET
SET END_SET
END_PROPERTY
END_INTERFACE",
    );
    let accessors: Vec<_> = itf
        .properties
        .iter()
        .map(|p| (p.name.to_string(), p.get.is_some(), p.set.is_some()))
        .collect();
    assert_eq!(
        accessors,
        vec![
            ("Position".to_string(), true, false),
            ("Target".to_string(), false, true),
            ("Speed".to_string(), true, true),
        ]
    );
}

#[test]
fn parse_when_members_interleave_and_extends_then_all_kept() {
    let itf = parse_interface(
        "
INTERFACE I_Telescope EXTENDS I_Axis, I_Brake
PROPERTY Ready : BOOL
GET END_GET
END_PROPERTY
METHOD Park : BOOL
END_METHOD
PROPERTY Busy : BOOL
GET END_GET
END_PROPERTY
METHOD Stop
END_METHOD
END_INTERFACE",
    );
    assert_eq!(itf.extends.len(), 2);
    let methods: Vec<_> = itf.methods.iter().map(|m| m.name.to_string()).collect();
    let properties: Vec<_> = itf.properties.iter().map(|p| p.name.to_string()).collect();
    assert_eq!(methods, vec!["Park", "Stop"]);
    assert_eq!(properties, vec!["Ready", "Busy"]);
}

#[test]
fn parse_when_method_prototype_has_body_then_err() {
    assert!(parse_err(
        "
INTERFACE I_Brake
METHOD CloseBrake : BOOL
CloseBrake := TRUE;
END_METHOD
END_INTERFACE"
    ));
}

#[rstest]
#[case::var("VAR\n    x : INT;\nEND_VAR")]
#[case::var_temp("VAR_TEMP\n    x : INT;\nEND_VAR")]
fn parse_when_method_prototype_has_local_block_then_err(#[case] block: &str) {
    let source = format!(
        "
INTERFACE I_Brake
METHOD CloseBrake : BOOL
{block}
END_METHOD
END_INTERFACE"
    );
    assert!(parse_err(&source));
}

#[test]
fn parse_when_property_prototype_accessor_has_body_then_err() {
    assert!(parse_err(
        "
INTERFACE I_Axis
PROPERTY Position : LREAL
GET
    Position := 1.0;
END_GET
END_PROPERTY
END_INTERFACE"
    ));
}

#[test]
fn parse_when_interface_members_and_default_dialect_then_err() {
    let source = "
INTERFACE I_Brake
METHOD CloseBrake : BOOL
END_METHOD
END_INTERFACE";
    let result = parse_program(source, &FileId::default(), &CompilerOptions::default());
    assert!(result.is_err());
}
