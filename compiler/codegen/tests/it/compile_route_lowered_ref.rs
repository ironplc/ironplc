//! Tests of references made and compared in the lowered program, with every
//! statement required to lower: `REF(x)`, `r REF= x` and `NULL`.

use rstest::rstest;

use crate::compile_route_lowered::{assert_same_code, run_lowered};

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
