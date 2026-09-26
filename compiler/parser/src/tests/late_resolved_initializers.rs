//! A declaration against a user-defined type name is recorded with its
//! initializer as written; the parser does not decide what the type is
//! (ADR-0050). Only an unambiguous shape settles the kind in the parser.
use super::common::*;

fn first_var_initializer(source: &'static str) -> InitialValueAssignmentKind {
    let library = parse_text(source);
    let fb = cast!(
        &library.elements[0],
        LibraryElementKind::FunctionBlockDeclaration
    );
    fb.variables[0].initializer.clone()
}

#[test]
fn parse_when_user_type_with_member_list_then_late_resolved_members() {
    let init = first_var_initializer(
        "
FUNCTION_BLOCK fb
VAR
    inst : SomeType := (a := 1, b := 2);
END_VAR
END_FUNCTION_BLOCK",
    );
    let late = cast!(&init, InitialValueAssignmentKind::LateResolvedType);
    assert_eq!(TypeName::from("SomeType"), late.type_name);
    assert!(matches!(
        &late.initial_value,
        Some(LateResolvedInitialValue::Members(elements)) if elements.len() == 2
    ));
}

#[test]
fn parse_when_user_type_with_bare_identifier_then_late_resolved_value() {
    let init = first_var_initializer(
        "
FUNCTION_BLOCK fb
VAR
    state : SomeType := Idle;
END_VAR
END_FUNCTION_BLOCK",
    );
    let late = cast!(&init, InitialValueAssignmentKind::LateResolvedType);
    assert_eq!(TypeName::from("SomeType"), late.type_name);
    assert_eq!(
        Some(LateResolvedInitialValue::Value(Id::from("Idle"))),
        late.initial_value
    );
}

#[test]
fn parse_when_user_type_without_initializer_then_late_resolved_bare() {
    let init = first_var_initializer(
        "
FUNCTION_BLOCK fb
VAR
    state : SomeType;
END_VAR
END_FUNCTION_BLOCK",
    );
    let late = cast!(&init, InitialValueAssignmentKind::LateResolvedType);
    assert_eq!(
        LateResolvedInitializer::bare(TypeName::from("SomeType")),
        *late
    );
}

#[test]
fn parse_when_qualified_enumerated_value_then_enumerated_type() {
    // `SomeType#Idle` can only be an enumeration value, so there is nothing
    // left for the resolver to decide.
    let init = first_var_initializer(
        "
FUNCTION_BLOCK fb
VAR
    state : SomeType := SomeType#Idle;
END_VAR
END_FUNCTION_BLOCK",
    );
    let enumerated = cast!(&init, InitialValueAssignmentKind::EnumeratedType);
    assert_eq!(TypeName::from("SomeType"), enumerated.type_name);
    assert!(matches!(
        &enumerated.initial_value,
        Some(value) if value.value == Id::from("Idle") && value.type_name.is_some()
    ));
}
