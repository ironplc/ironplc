//! `REFERENCE TO` / dereference / NULL round-tripping.

use super::common::*;
use rstest::rstest;

#[test]
fn write_to_string_when_reference_to_then_round_trips() {
    let options = CompilerOptions {
        allow_reference_to: true,
        ..CompilerOptions::default()
    };
    assert_resource_renders_to("reference_to.st", "reference_to_rendered.st", &options);
}

#[test]
fn write_to_string_ref() {
    assert_resource_renders_to("ref.st", "ref_rendered.st", &edition3());
}

/// Edition-3 render round-trips and preserves the given fragment.
///
/// Each case parses a small program under the edition-3 dialect, renders it
/// back to text, re-parses the rendering (same AST required), and checks the
/// rendering contains the expected fragment. The re-parse is what catches a
/// stray space the fragment check would miss.
#[rstest]
#[case::ref_to_var_decl(
    "PROGRAM main
VAR
    x : REF_TO INT;
END_VAR
END_PROGRAM",
    "REF_TO INT"
)]
#[case::ref_to_array_var_decl(
    "PROGRAM main
VAR
    PT : REF_TO ARRAY[0..10] OF BYTE;
END_VAR
END_PROGRAM",
    "REF_TO ARRAY"
)]
#[case::ref_to_var_decl_with_null_init(
    "PROGRAM main
VAR
    x : REF_TO INT := NULL;
END_VAR
END_PROGRAM",
    "REF_TO INT := NULL"
)]
#[case::ref_to_var_decl_with_ref_init(
    "PROGRAM main
VAR
    counter : INT;
    x : REF_TO INT := REF(counter);
END_VAR
END_PROGRAM",
    "REF_TO INT := REF("
)]
#[case::deref_assign(
    "PROGRAM main
VAR
    myRef : REF_TO INT;
END_VAR
    myRef^ := 42;
END_PROGRAM",
    "myRef^ :="
)]
#[case::deref_expression(
    "PROGRAM main
VAR
    myRef : REF_TO INT;
    value : INT;
END_VAR
    value := myRef^;
END_PROGRAM",
    "myRef^"
)]
#[case::ref_expression(
    "PROGRAM main
VAR
    counter : INT;
    x : REF_TO INT;
END_VAR
    x := REF(counter);
END_PROGRAM",
    "REF("
)]
#[case::null_expression(
    "PROGRAM main
VAR
    x : REF_TO INT;
END_VAR
    x := NULL;
END_PROGRAM",
    "NULL"
)]
// Array subscripts, in expression and assignment-target position. A
// subscript renders tight against its variable -- `symbolic_variable`
// chains its elements with no whitespace rule between them.
#[case::subscript_expression(
    "PROGRAM main
VAR
    refs : ARRAY[0..2] OF INT;
    value : INT;
END_VAR
    value := refs[0];
END_PROGRAM",
    "refs[ 0 ]"
)]
#[case::subscript_assignment_target(
    "PROGRAM main
VAR
    refs : ARRAY[0..2] OF INT;
END_VAR
    refs[1] := 5;
END_PROGRAM",
    "refs[ 1 ] :="
)]
#[case::deref_of_subscript(
    "PROGRAM main
VAR
    refs : ARRAY[0..2] OF REF_TO INT;
    value : INT;
END_VAR
    value := refs[0]^;
END_PROGRAM",
    "refs[ 0 ]^"
)]
#[case::subscript_of_deref(
    "FUNCTION my_func : BYTE
VAR_INPUT
    PT : REF_TO ARRAY[0..10] OF BYTE;
END_VAR
    my_func := PT^[0];
END_FUNCTION",
    "PT^[ 0 ]"
)]
#[case::subscript_of_field(
    "TYPE
S : STRUCT
    items : ARRAY[0..2] OF INT;
END_STRUCT;
END_TYPE
PROGRAM main
VAR
    s : S;
    value : INT;
END_VAR
    value := s.items[0];
END_PROGRAM",
    "s.items[ 0 ]"
)]
#[case::multi_dimensional_subscript(
    "PROGRAM main
VAR
    grid : ARRAY[0..2, 0..2] OF INT;
    value : INT;
END_VAR
    value := grid[1, 2];
END_PROGRAM",
    "grid[ 1 , 2 ]"
)]
fn write_to_string_when_reference_source_then_preserves(
    #[case] source: &str,
    #[case] needle: &str,
) {
    let rendered = assert_round_trips(source, &edition3());
    assert!(
        rendered.contains(needle),
        "Expected {needle} in output, got: {rendered}"
    );
}

#[test]
fn write_to_string_when_ref_to_type_decl_then_preserves() {
    let rendered = assert_round_trips("TYPE IntRef : REF_TO INT; END_TYPE", &edition3());
    let expected = "TYPE\n   IntRef : REF_TO INT ;\nEND_TYPE\n";
    assert_eq!(rendered, expected);
}
