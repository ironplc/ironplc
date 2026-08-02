//! `REFERENCE TO` / dereference / NULL round-tripping.

use super::common::*;

#[test]
fn write_to_string_when_reference_to_then_round_trips() {
    let source = read_shared_resource("reference_to.st");
    let options = CompilerOptions {
        allow_reference_to: true,
        ..CompilerOptions::default()
    };
    let library = parse_program(&source, &FileId::default(), &options).unwrap();
    let rendered = write_to_string(&library).unwrap();
    let expected = read_resource("reference_to_rendered.st");
    assert_eq!(rendered, expected);
}

#[test]
fn write_to_string_ref() {
    let rendered = parse_and_render_resource_edition3("ref.st");
    let expected = read_resource("ref_rendered.st");
    assert_eq!(rendered, expected);
}

#[test]
fn write_to_string_when_ref_to_var_decl_then_preserves_ref_to() {
    let rendered = parse_and_render_edition3(
        "PROGRAM main
VAR
    x : REF_TO INT;
END_VAR
END_PROGRAM",
    );
    assert!(
        rendered.contains("REF_TO INT"),
        "Expected REF_TO INT in output, got: {rendered}"
    );
}

#[test]
fn write_to_string_when_ref_to_array_var_decl_then_preserves_ref_to() {
    let rendered = parse_and_render_edition3(
        "PROGRAM main
VAR
    PT : REF_TO ARRAY[0..10] OF BYTE;
END_VAR
END_PROGRAM",
    );
    assert!(
        rendered.contains("REF_TO ARRAY"),
        "Expected REF_TO ARRAY in output, got: {rendered}"
    );
}

#[test]
fn write_to_string_when_ref_to_var_decl_with_null_init_then_preserves() {
    let rendered = parse_and_render_edition3(
        "PROGRAM main
VAR
    x : REF_TO INT := NULL;
END_VAR
END_PROGRAM",
    );
    assert!(
        rendered.contains("REF_TO INT := NULL"),
        "Expected REF_TO INT := NULL in output, got: {rendered}"
    );
}

#[test]
fn write_to_string_when_ref_to_var_decl_with_ref_init_then_preserves() {
    let rendered = parse_and_render_edition3(
        "PROGRAM main
VAR
    counter : INT;
    x : REF_TO INT := REF(counter);
END_VAR
END_PROGRAM",
    );
    assert!(
        rendered.contains("REF_TO INT := REF("),
        "Expected REF_TO INT := REF(...) in output, got: {rendered}"
    );
}

#[test]
fn write_to_string_when_deref_assign_then_preserves_caret() {
    let rendered = parse_and_render_edition3(
        "PROGRAM main
VAR
    myRef : REF_TO INT;
END_VAR
    myRef^ := 42;
END_PROGRAM",
    );
    assert!(
        rendered.contains("myRef^ :="),
        "Expected myRef^ := in output, got: {rendered}"
    );
}

#[test]
fn write_to_string_when_deref_expression_then_preserves_caret() {
    let rendered = parse_and_render_edition3(
        "PROGRAM main
VAR
    myRef : REF_TO INT;
    value : INT;
END_VAR
    value := myRef^;
END_PROGRAM",
    );
    assert!(
        rendered.contains("myRef ^"),
        "Expected myRef ^ in output, got: {rendered}"
    );
}

#[test]
fn write_to_string_when_deref_array_expression_then_preserves() {
    let rendered = parse_and_render_edition3(
        "FUNCTION my_func : BYTE
VAR_INPUT
    PT : REF_TO ARRAY[0..10] OF BYTE;
END_VAR
    my_func := PT^[0];
END_FUNCTION",
    );
    assert!(
        rendered.contains("PT^"),
        "Expected PT^ in output, got: {rendered}"
    );
    assert!(
        rendered.contains("[ 0 ]"),
        "Expected array subscript in output, got: {rendered}"
    );
}

#[test]
fn write_to_string_when_ref_expression_then_preserves() {
    let rendered = parse_and_render_edition3(
        "PROGRAM main
VAR
    counter : INT;
    x : REF_TO INT;
END_VAR
    x := REF(counter);
END_PROGRAM",
    );
    assert!(
        rendered.contains("REF("),
        "Expected REF( in output, got: {rendered}"
    );
}

#[test]
fn write_to_string_when_null_expression_then_preserves() {
    let rendered = parse_and_render_edition3(
        "PROGRAM main
VAR
    x : REF_TO INT;
END_VAR
    x := NULL;
END_PROGRAM",
    );
    assert!(
        rendered.contains("NULL"),
        "Expected NULL in output, got: {rendered}"
    );
}

#[test]
fn write_to_string_when_ref_to_type_decl_then_preserves() {
    let rendered = parse_and_render_edition3("TYPE IntRef : REF_TO INT; END_TYPE");
    let expected = "TYPE\n   IntRef : REF_TO INT ;\nEND_TYPE\n";
    assert_eq!(rendered, expected);
}
