//! Array declarations (of REFERENCE TO and of STRING).

use super::common::*;

#[test]
fn parse_when_array_of_ref_to_then_ok() {
    let lib = parse_text_edition3(
        "PROGRAM main
VAR
    data : ARRAY[0..3] OF REF_TO BYTE;
END_VAR
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    assert_eq!(prog.variables.len(), 1);
    let arr = cast!(
        &prog.variables[0].initializer,
        InitialValueAssignmentKind::Array
    );
    let subranges = cast!(&arr.spec, SpecificationKind::Inline);
    assert!(subranges.ref_to.is_some());
    assert_eq!(subranges.type_name.to_type_name().to_string(), "BYTE");
    assert_eq!(subranges.ranges.len(), 1);
}

#[test]
fn parse_when_array_of_ref_to_type_decl_then_ok() {
    let lib = parse_text_edition3("TYPE MyArr : ARRAY[1..5] OF REF_TO INT; END_TYPE");
    let dt = cast!(&lib.elements[0], LibraryElementKind::DataTypeDeclaration);
    let arr = cast!(dt, DataTypeDeclarationKind::Array);
    let subranges = cast!(&arr.spec, SpecificationKind::Inline);
    assert!(subranges.ref_to.is_some());
    assert_eq!(subranges.type_name.to_type_name().to_string(), "INT");
}

#[test]
fn parse_when_array_without_ref_to_then_ref_to_is_false() {
    let lib = parse_text_edition3(
        "PROGRAM main
VAR
    data : ARRAY[0..3] OF BYTE;
END_VAR
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let arr = cast!(
        &prog.variables[0].initializer,
        InitialValueAssignmentKind::Array
    );
    let subranges = cast!(&arr.spec, SpecificationKind::Inline);
    assert!(subranges.ref_to.is_none());
}

#[test]
fn parse_when_array_of_string_with_size_then_ok() {
    let lib = parse_text(
        "PROGRAM main
VAR
    names : ARRAY[1..3] OF STRING[10];
END_VAR
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let arr = cast!(
        &prog.variables[0].initializer,
        InitialValueAssignmentKind::Array
    );
    let subranges = cast!(&arr.spec, SpecificationKind::Inline);
    assert_eq!(subranges.type_name.to_type_name().to_string(), "STRING");
    assert_eq!(subranges.ranges.len(), 1);
    let spec = cast!(&subranges.type_name, ArrayElementType::String);
    assert_eq!(
        spec.length.as_ref().unwrap().as_integer().unwrap().value,
        10
    );
}

#[test]
fn parse_when_array_of_wstring_with_size_then_ok() {
    let lib = parse_text(
        "PROGRAM main
VAR
    names : ARRAY[1..3] OF WSTRING[20];
END_VAR
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let arr = cast!(
        &prog.variables[0].initializer,
        InitialValueAssignmentKind::Array
    );
    let subranges = cast!(&arr.spec, SpecificationKind::Inline);
    assert_eq!(subranges.type_name.to_type_name().to_string(), "WSTRING");
    let spec = cast!(&subranges.type_name, ArrayElementType::WString);
    assert_eq!(
        spec.length.as_ref().unwrap().as_integer().unwrap().value,
        20
    );
}

#[test]
fn parse_when_array_of_string_without_size_then_ok() {
    let lib = parse_text(
        "PROGRAM main
VAR
    names : ARRAY[1..3] OF STRING;
END_VAR
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let arr = cast!(
        &prog.variables[0].initializer,
        InitialValueAssignmentKind::Array
    );
    let subranges = cast!(&arr.spec, SpecificationKind::Inline);
    let spec = cast!(&subranges.type_name, ArrayElementType::String);
    assert!(spec.length.is_none());
}

#[test]
fn parse_when_multidim_array_of_string_with_size_then_ok() {
    let lib = parse_text(
        "PROGRAM main
VAR
    weekdays : ARRAY[1..3, 1..7] OF STRING[10];
END_VAR
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let arr = cast!(
        &prog.variables[0].initializer,
        InitialValueAssignmentKind::Array
    );
    let subranges = cast!(&arr.spec, SpecificationKind::Inline);
    assert_eq!(subranges.ranges.len(), 2);
    let spec = cast!(&subranges.type_name, ArrayElementType::String);
    assert_eq!(
        spec.length.as_ref().unwrap().as_integer().unwrap().value,
        10
    );
}
