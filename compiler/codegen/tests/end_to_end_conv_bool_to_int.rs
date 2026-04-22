//! End-to-end tests for BOOL to integer type conversions.

mod common;

use common::parse_and_run;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

// BOOL_TO_<TY>(x) with a 32-bit-wide target: asserts that y (vars[1]) equals
// 1 when x is TRUE and 0 when x is FALSE.
#[rstest]
#[case::sint_true("SINT", "TRUE", 1)]
#[case::sint_false("SINT", "FALSE", 0)]
#[case::int_true("INT", "TRUE", 1)]
#[case::int_false("INT", "FALSE", 0)]
#[case::dint_true("DINT", "TRUE", 1)]
#[case::usint_true("USINT", "TRUE", 1)]
#[case::uint_true("UINT", "TRUE", 1)]
#[case::udint_true("UDINT", "TRUE", 1)]
fn end_to_end_bool_to_int32(
    #[case] target_ty: &str,
    #[case] bool_lit: &str,
    #[case] expected: i32,
) {
    let source = format!(
        "PROGRAM main VAR x : BOOL; y : {target_ty}; END_VAR x := {bool_lit}; y := BOOL_TO_{target_ty}(x); END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), expected);
}

// BOOL_TO_<TY>(x) with a 64-bit-wide target (LINT / ULINT): asserts via as_i64.
#[rstest]
#[case::lint_true("LINT", "TRUE", 1)]
#[case::ulint_true("ULINT", "TRUE", 1)]
#[case::ulint_false("ULINT", "FALSE", 0)]
fn end_to_end_bool_to_int64(
    #[case] target_ty: &str,
    #[case] bool_lit: &str,
    #[case] expected: i64,
) {
    let source = format!(
        "PROGRAM main VAR x : BOOL; y : {target_ty}; END_VAR x := {bool_lit}; y := BOOL_TO_{target_ty}(x); END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i64(), expected);
}
