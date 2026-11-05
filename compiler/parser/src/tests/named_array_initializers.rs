//! A declaration of a named array type with an array initializer,
//! `x : A3 := [1, 2, 3]` (IEC 61131-3 `array_spec_init`, whose
//! `array_specification` is either a named array type or `ARRAY[...] OF ...`).
//!
//! Only an array takes a bracketed initializer, so the parser records the
//! declaration as an array of the named type without waiting for the type
//! resolver (ADR-0050). These tests pin that shape in each position that
//! reaches `array_spec_init`.
use super::common::*;
use dsl::common::{ArrayInitialElementKind, ArrayInitialValueAssignment};

/// The first variable of the first program organization unit in `library`.
fn first_pou_var(library: &Library) -> &VarDecl {
    match &library.elements[0] {
        LibraryElementKind::ProgramDeclaration(p) => &p.variables[0],
        LibraryElementKind::FunctionBlockDeclaration(fb) => &fb.variables[0],
        LibraryElementKind::FunctionDeclaration(f) => &f.variables[0],
        other => panic!("not a program organization unit: {other:?}"),
    }
}

/// Asserts that `init` is an array initializer of the named type `A3` and
/// returns its values.
fn named_a3_values(init: &InitialValueAssignmentKind) -> &Vec<ArrayInitialElementKind> {
    let array: &ArrayInitialValueAssignment = cast!(init, InitialValueAssignmentKind::Array);
    assert_eq!(SpecificationKind::Named(TypeName::from("A3")), array.spec);
    &array.initial_values
}

#[rstest]
#[case::program_var("PROGRAM main VAR x : A3 := [1, 2, 3]; END_VAR END_PROGRAM")]
#[case::program_var_input("PROGRAM main VAR_INPUT x : A3 := [1, 2, 3]; END_VAR END_PROGRAM")]
#[case::program_var_output("PROGRAM main VAR_OUTPUT x : A3 := [1, 2, 3]; END_VAR END_PROGRAM")]
#[case::program_var_temp("PROGRAM main VAR_TEMP x : A3 := [1, 2, 3]; END_VAR END_PROGRAM")]
#[case::function_block_var_input(
    "FUNCTION_BLOCK fb VAR_INPUT x : A3 := [1, 2, 3]; END_VAR END_FUNCTION_BLOCK"
)]
#[case::function_var("FUNCTION f : DINT VAR x : A3 := [1, 2, 3]; END_VAR f := 0; END_FUNCTION")]
#[case::located_var("PROGRAM main VAR x AT %MW10 : A3 := [1, 2, 3]; END_VAR END_PROGRAM")]
fn parse_when_pou_var_of_named_array_with_values_then_named_array(#[case] source: &'static str) {
    let library = parse_text(source);

    let values = named_a3_values(&first_pou_var(&library).initializer);

    assert_eq!(3, values.len());
}

#[test]
fn parse_when_global_var_of_named_array_with_values_then_named_array() {
    let library = parse_text(
        "CONFIGURATION config
  VAR_GLOBAL g : A3 := [4, 5, 6]; END_VAR
  RESOURCE res ON PLC
    PROGRAM p : main;
  END_RESOURCE
END_CONFIGURATION",
    );
    let config = cast!(
        &library.elements[0],
        LibraryElementKind::ConfigurationDeclaration
    );

    let values = named_a3_values(&config.global_var[0].initializer);

    assert_eq!(3, values.len());
}

#[test]
fn parse_when_var_config_of_named_array_with_values_then_named_array() {
    let library = parse_text(
        "CONFIGURATION config
  RESOURCE res ON PLC
    PROGRAM p : main;
  END_RESOURCE
  VAR_CONFIG
    res.p.x : A3 := [4, 5, 6];
  END_VAR
END_CONFIGURATION",
    );
    let config = cast!(
        &library.elements[0],
        LibraryElementKind::ConfigurationDeclaration
    );

    let values = named_a3_values(&config.located_var_inits[0].initializer);

    assert_eq!(3, values.len());
}

#[test]
fn parse_when_structure_field_of_named_array_with_values_then_named_array() {
    let library = parse_text("TYPE S : STRUCT f : A3 := [1, 2, 3]; END_STRUCT; END_TYPE");
    let data_type = cast!(
        &library.elements[0],
        LibraryElementKind::DataTypeDeclaration
    );
    let structure = cast!(data_type, DataTypeDeclarationKind::Structure);

    let values = named_a3_values(&structure.elements[0].init);

    assert_eq!(3, values.len());
}

#[test]
fn parse_when_type_alias_of_named_array_with_values_then_array_declaration() {
    let library = parse_text("TYPE B3 : A3 := [1, 2, 3]; END_TYPE");
    let data_type = cast!(
        &library.elements[0],
        LibraryElementKind::DataTypeDeclaration
    );

    let array = cast!(data_type, DataTypeDeclarationKind::Array);

    assert_eq!(TypeName::from("B3"), array.type_name);
    assert_eq!(SpecificationKind::Named(TypeName::from("A3")), array.spec);
    assert_eq!(3, array.init.len());
}

#[test]
fn parse_when_named_array_with_repeated_value_then_repeated_element() {
    let library = parse_text("PROGRAM main VAR x : A3 := [2(0), 1]; END_VAR END_PROGRAM");

    let values = named_a3_values(&first_pou_var(&library).initializer);

    assert_eq!(2, values.len());
    let repeated = cast!(&values[0], ArrayInitialElementKind::Repeated);
    assert_eq!(2, repeated.size.value);
    assert!(matches!(values[1], ArrayInitialElementKind::Constant(_)));
}

#[test]
fn parse_when_named_array_with_enumerated_values_then_enum_elements() {
    let library = parse_text("PROGRAM main VAR x : A3 := [Red, Green]; END_VAR END_PROGRAM");

    let values = named_a3_values(&first_pou_var(&library).initializer);

    assert_eq!(2, values.len());
    let first = cast!(&values[0], ArrayInitialElementKind::EnumValue);
    assert_eq!(Id::from("Red"), first.value);
}

#[test]
fn parse_when_elementary_type_with_array_initializer_then_err() {
    // `array_type_name` is an identifier, and an elementary type name is a
    // keyword, so the bracket has no rule to match.
    let result = parse_program(
        "PROGRAM main VAR x : DINT := [1, 2]; END_VAR END_PROGRAM",
        &FileId::default(),
        &CompilerOptions::default(),
    );

    assert_eq!("P0002", result.unwrap_err().code);
}
