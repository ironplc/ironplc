//! A declaration of a named array type with an array initializer,
//! `x : Triple := [1, 2, 3]` (IEC 61131-3 `array_spec_init`, whose
//! `array_specification` is either a named array type or `ARRAY[...] OF ...`).
//!
//! Only an array takes a bracketed initializer, so the parser records the
//! declaration as an array of the named type without waiting for the type
//! resolver (ADR-0050). These tests pin that shape in each position that
//! reaches `array_spec_init`.
use super::common::*;
use dsl::common::{ArrayInitialElementKind, ArrayInitialValueAssignment};

/// The array type every test declares its variables with.
const TRIPLE: &str = "TYPE Triple : ARRAY[1..3] OF DINT; END_TYPE\n";

/// Parses `TRIPLE` followed by `source`.
fn parse_with_triple(source: &str) -> Library {
    let source = format!("{TRIPLE}{source}");
    parse_program(&source, &FileId::default(), &CompilerOptions::default())
        .unwrap_or_else(|e| panic!("Source did not parse: {e:?}\n{source}"))
}

/// The first variable of the first program organization unit in `library`.
fn first_pou_var(library: &Library) -> &VarDecl {
    library
        .elements
        .iter()
        .find_map(|element| match element {
            LibraryElementKind::ProgramDeclaration(p) => Some(&p.variables[0]),
            LibraryElementKind::FunctionBlockDeclaration(fb) => Some(&fb.variables[0]),
            LibraryElementKind::FunctionDeclaration(f) => Some(&f.variables[0]),
            _ => None,
        })
        .expect("no program organization unit")
}

/// The configuration in `library`.
fn configuration(library: &Library) -> &ConfigurationDeclaration {
    library
        .elements
        .iter()
        .find_map(|element| match element {
            LibraryElementKind::ConfigurationDeclaration(c) => Some(c),
            _ => None,
        })
        .expect("no configuration")
}

/// The last data type declared in `library`: the one a test declares after
/// `TRIPLE`.
fn last_data_type(library: &Library) -> &DataTypeDeclarationKind {
    cast!(
        library.elements.last().unwrap(),
        LibraryElementKind::DataTypeDeclaration
    )
}

/// Asserts that `init` is an array initializer of the named type `type_name`
/// and returns its values.
fn named_array_values<'a>(
    init: &'a InitialValueAssignmentKind,
    type_name: &str,
) -> &'a Vec<ArrayInitialElementKind> {
    let array: &ArrayInitialValueAssignment = cast!(init, InitialValueAssignmentKind::Array);
    assert_eq!(
        SpecificationKind::Named(TypeName::from(type_name)),
        array.spec
    );
    &array.initial_values
}

#[rstest]
#[case::program_var("PROGRAM main VAR x : Triple := [1, 2, 3]; END_VAR END_PROGRAM")]
#[case::program_var_input("PROGRAM main VAR_INPUT x : Triple := [1, 2, 3]; END_VAR END_PROGRAM")]
#[case::program_var_output("PROGRAM main VAR_OUTPUT x : Triple := [1, 2, 3]; END_VAR END_PROGRAM")]
#[case::program_var_temp("PROGRAM main VAR_TEMP x : Triple := [1, 2, 3]; END_VAR END_PROGRAM")]
#[case::function_block_var_input(
    "FUNCTION_BLOCK fb VAR_INPUT x : Triple := [1, 2, 3]; END_VAR END_FUNCTION_BLOCK"
)]
#[case::function_var("FUNCTION f : DINT VAR x : Triple := [1, 2, 3]; END_VAR f := 0; END_FUNCTION")]
#[case::located_var("PROGRAM main VAR x AT %MD10 : Triple := [1, 2, 3]; END_VAR END_PROGRAM")]
fn parse_when_pou_var_of_named_array_with_values_then_named_array(#[case] source: &str) {
    let library = parse_with_triple(source);

    let values = named_array_values(&first_pou_var(&library).initializer, "Triple");

    assert_eq!(3, values.len());
}

#[test]
fn parse_when_global_var_of_named_array_with_values_then_named_array() {
    let library = parse_with_triple(
        "PROGRAM main END_PROGRAM
CONFIGURATION config
  VAR_GLOBAL g : Triple := [4, 5, 6]; END_VAR
  RESOURCE res ON PLC
    PROGRAM p : main;
  END_RESOURCE
END_CONFIGURATION",
    );

    let values = named_array_values(&configuration(&library).global_var[0].initializer, "Triple");

    assert_eq!(3, values.len());
}

#[test]
fn parse_when_var_config_of_named_array_with_values_then_named_array() {
    let library = parse_with_triple(
        "PROGRAM main VAR x : Triple; END_VAR END_PROGRAM
CONFIGURATION config
  RESOURCE res ON PLC
    PROGRAM p : main;
  END_RESOURCE
  VAR_CONFIG
    res.p.x : Triple := [4, 5, 6];
  END_VAR
END_CONFIGURATION",
    );

    let values = named_array_values(
        &configuration(&library).located_var_inits[0].initializer,
        "Triple",
    );

    assert_eq!(3, values.len());
}

#[test]
fn parse_when_structure_field_of_named_array_with_values_then_named_array() {
    let library =
        parse_with_triple("TYPE S : STRUCT f : Triple := [1, 2, 3]; END_STRUCT; END_TYPE");
    let structure = cast!(last_data_type(&library), DataTypeDeclarationKind::Structure);

    let values = named_array_values(&structure.elements[0].init, "Triple");

    assert_eq!(3, values.len());
}

#[test]
fn parse_when_type_alias_of_named_array_with_values_then_array_declaration() {
    let library = parse_with_triple("TYPE TripleDefault : Triple := [1, 2, 3]; END_TYPE");

    let array = cast!(last_data_type(&library), DataTypeDeclarationKind::Array);

    assert_eq!(TypeName::from("TripleDefault"), array.type_name);
    assert_eq!(
        SpecificationKind::Named(TypeName::from("Triple")),
        array.spec
    );
    assert_eq!(3, array.init.len());
}

#[test]
fn parse_when_named_array_with_repeated_value_then_repeated_element() {
    let library =
        parse_with_triple("PROGRAM main VAR x : Triple := [2(0), 1]; END_VAR END_PROGRAM");

    let values = named_array_values(&first_pou_var(&library).initializer, "Triple");

    assert_eq!(2, values.len());
    let repeated = cast!(&values[0], ArrayInitialElementKind::Repeated);
    assert_eq!(2, repeated.size.value);
    assert!(matches!(values[1], ArrayInitialElementKind::Constant(_)));
}

#[test]
fn parse_when_named_array_with_enumerated_values_then_enum_elements() {
    let library = parse_with_triple(
        "TYPE Color : (Red, Green); Colors : ARRAY[1..2] OF Color; END_TYPE
PROGRAM main VAR x : Colors := [Red, Green]; END_VAR END_PROGRAM",
    );

    let values = named_array_values(&first_pou_var(&library).initializer, "Colors");

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
