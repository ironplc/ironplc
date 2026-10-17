//! Tests of references made, compared and followed in the lowered program,
//! with every statement required to lower: `REF(x)`, `r REF= x`, `NULL` and
//! the bits of an element a reference reaches.

use rstest::rstest;

use ironplc_codegen::Route;

use crate::common::bc;
use crate::compile_route_lowered::{assert_same_code, body_by_route, run_lowered};

/// A reference made by `REF(x)` or `REF=` reaches its variable, read and
/// written through it, and compares equal to another to the same variable
/// and unequal to `NULL`, which a reference compares equal to once set to
/// it.
#[test]
fn run_when_references_made_and_compared_then_values() {
    let snapshot = run_lowered(
        "PROGRAM main
           VAR x : DINT := 5; y : DINT; p : REF_TO DINT; q : REF_TO DINT;
               same : BOOL; set : BOOL; cleared : BOOL; END_VAR
           p := REF(x);
           q REF= x;
           p^ := p^ + 1;
           y := q^;
           same := p = q;
           set := p <> NULL;
           p := NULL;
           cleared := p = NULL;
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("x"), 6);
    assert_eq!(snapshot.read_as::<i64>("y"), 6);
    assert_eq!(snapshot.read_as::<i64>("same"), 1);
    assert_eq!(snapshot.read_as::<i64>("set"), 1);
    assert_eq!(snapshot.read_as::<i64>("cleared"), 1);
}

/// `REF(v)` of a function's `VAR_IN_OUT` refers to the caller's variable,
/// which a write through the reference changes.
#[test]
fn run_when_ref_of_in_out_then_refers_to_callers_variable() {
    let snapshot = run_lowered(
        "FUNCTION BUMP : BOOL VAR_IN_OUT v : DINT; END_VAR VAR p : REF_TO DINT; END_VAR
           p := REF(v);
           p^ := p^ + 10;
           BUMP := p <> NULL;
         END_FUNCTION
         PROGRAM main VAR x : DINT := 1; ok : BOOL; END_VAR
           ok := BUMP(x);
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i64>("x"), 11);
    assert_eq!(snapshot.read_as::<i64>("ok"), 1);
}

/// Making, binding, clearing and comparing a reference compile to the same
/// instructions and constants by both routes.
#[rstest]
#[case::ref_of_variable("p := REF(x);")]
#[case::ref_bind("p REF= x;")]
#[case::ref_of_array("pa := REF(a);")]
#[case::null("p := NULL;")]
#[case::compared_with_null("b := p = NULL;")]
#[case::compared_with_reference("b := p <> q;")]
fn compile_when_reference_lowered_then_same_code_as_ast(#[case] statement: &str) {
    let source = format!(
        "PROGRAM main
           VAR x : DINT; a : ARRAY[1..3] OF DINT; p : REF_TO DINT; q : REF_TO DINT;
               pa : REF_TO ARRAY[1..3] OF DINT; b : BOOL; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_same_code(&source);
}

/// Bits and parts of an element of an array a reference refers to are
/// written in the referenced array, leaving its other bits as they were,
/// and read from it.
#[test]
fn run_when_bits_of_element_through_reference_then_referenced_array() {
    let snapshot = run_lowered(
        "PROGRAM main
           VAR a : ARRAY[0..1] OF DWORD := [0, 16#12345678]; p : REF_TO ARRAY[0..1] OF DWORD;
               b : BYTE := 16#FF; i : DINT := 1; x : BOOL; y : BOOL; r : DWORD; part : BYTE; END_VAR
           p := REF(a);
           p^[1].0 := TRUE;
           p^[i].%B2 := b;
           x := p^[1].3;
           y := p^[i].2;
           part := p^[1].%B3;
           r := a[1];
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<u64>("r"), 0x12FF_5679);
    assert_eq!(snapshot.read_as::<u64>("x"), 1);
    assert_eq!(snapshot.read_as::<u64>("y"), 0);
    assert_eq!(snapshot.read_as::<u64>("part"), 0x12);
}

/// A partial read of an element of an array a reference refers to compiles
/// to the same instructions and constants by both routes.
#[rstest]
#[case::part_read("b := p^[1].%B2;")]
#[case::part_read_by_variable("b := p^[i].%B2;")]
fn compile_when_part_of_element_through_reference_read_then_same_code_as_ast(
    #[case] statement: &str,
) {
    let source = format!(
        "PROGRAM main
           VAR p : REF_TO ARRAY[0..1] OF DWORD; b : BYTE; i : DINT; END_VAR
           {statement}
         END_PROGRAM"
    );
    assert_same_code(&source);
}

/// REQ-LOW-codegen-121: a bit write to an element of an array a reference
/// refers to evaluates the element's subscript once, keeping the address for
/// the store, where the AST route evaluates it for the load and again for
/// the store.
#[test]
fn compile_when_bit_written_to_element_through_reference_then_subscript_evaluated_once() {
    let source = "PROGRAM main
           VAR i : DINT; p : REF_TO ARRAY[0..1] OF DWORD; END_VAR
           p^[i].3 := TRUE;
         END_PROGRAM";
    // i is var:0.
    let load_i = bc::load_var_i32(0);
    let loads = |code: Vec<u8>| {
        code.windows(load_i.len())
            .filter(|window| *window == load_i.as_slice())
            .count()
    };
    assert_eq!(loads(body_by_route(source, Route::RequireLowered)), 1);
    assert_eq!(loads(body_by_route(source, Route::Ast)), 2);
}
