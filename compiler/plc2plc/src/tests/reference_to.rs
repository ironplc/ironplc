//! `REFERENCE TO` / dereference / NULL round-tripping.

use super::common::*;
use rstest::rstest;

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

/// Edition-3 render preserves the given reference-related fragment.
///
/// Each case parses a small program under the edition-3 dialect, renders it
/// back to text, and checks the rendering contains the expected fragment.
/// Collapses the single-`contains` reference round-trip tests into one table;
/// each row still runs as an individually-named test.
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
fn write_to_string_when_reference_source_then_preserves(
    #[case] source: &str,
    #[case] needle: &str,
) {
    let rendered = parse_and_render_edition3(source);
    assert!(
        rendered.contains(needle),
        "Expected {needle} in output, got: {rendered}"
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
fn write_to_string_when_ref_to_type_decl_then_preserves() {
    let rendered = parse_and_render_edition3("TYPE IntRef : REF_TO INT; END_TYPE");
    let expected = "TYPE\n   IntRef : REF_TO INT ;\nEND_TYPE\n";
    assert_eq!(rendered, expected);
}

/// Renders and then *re-parses* a dereference in expression position.
///
/// The renderer used to emit `myRef ^` with a separating space, which the
/// parser's `unary_expression` rule rejects, so the rendered text did not
/// re-parse. Re-parsing here means a future stray space fails this test
/// rather than passing silently.
///
/// Limited to a plain dereference: a subscripted one (`refs[0]^`) renders
/// as `refs [ 0 ]^`, which the parser also rejects -- a separate,
/// pre-existing spacing bug in the subscript rendering (#1407), not this
/// one.
#[test]
fn write_to_string_when_deref_expression_then_round_trips() {
    assert_round_trips(
        "PROGRAM main
VAR
    myRef : REF_TO INT;
    value : INT;
END_VAR
    value := myRef^;
END_PROGRAM",
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
}
