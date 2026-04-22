//! End-to-end integration tests for comparison operators.

#[macro_use]
mod common;

use common::parse_and_run;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

// DINT comparison ops share the envelope `VAR x, y : DINT; x := <lhs>; y := <expr>;`
// where the result is stored in `y` and the operand in `x`.
#[rstest]
#[case::eq_true(5, "x = 5", 1)]
#[case::ne_true(5, "x <> 3", 1)]
#[case::lt_true(3, "x < 5", 1)]
#[case::le_equal(5, "x <= 5", 1)]
#[case::gt_true(7, "x > 5", 1)]
#[case::ge_false(3, "x >= 5", 0)]
fn end_to_end_cmp_dint(#[case] x: i32, #[case] expr: &str, #[case] expected_y: i32) {
    let source =
        format!("PROGRAM main VAR x : DINT; y : DINT; END_VAR x := {x}; y := {expr}; END_PROGRAM");
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), x);
    assert_eq!(bufs.vars[1].as_i32(), expected_y);
}

// BYTE comparison ops: `VAR c : BYTE; result : BOOL; c := BYTE#<lhs>; result := c <op> BYTE#<rhs>;`
#[rstest]
#[case::ge_true(72, ">=", 65, 1)]
#[case::le_true(72, "<=", 90, 1)]
#[case::gt_false(50, ">", 65, 0)]
#[case::lt_false(200, "<", 100, 0)]
fn end_to_end_cmp_byte(#[case] lhs: u8, #[case] op: &str, #[case] rhs: u8, #[case] expected: i32) {
    let source = format!(
        "PROGRAM main VAR c : BYTE; result : BOOL; END_VAR c := BYTE#{lhs}; result := c {op} BYTE#{rhs}; END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), expected);
}

// REAL comparison-to-BOOL, two variants with slightly different shapes.
e2e_i32!(
    end_to_end_when_real_lt_assigned_to_bool_then_correct,
    "PROGRAM main VAR x : REAL; neg : BOOL; pos : BOOL; END_VAR x := -2.5; neg := x < 0.0; x := 3.5; pos := x < 0.0; END_PROGRAM",
    &[(1, 1), (2, 0)],
);

e2e_i32!(
    end_to_end_when_real_gt_assigned_to_bool_then_correct,
    "PROGRAM main VAR x : REAL; result : BOOL; END_VAR x := 1.5; result := x > 0.0; END_PROGRAM",
    &[(1, 1)],
);

// Range-check exercises a function with a compound comparison (AND of two BYTE comparisons).
e2e_i32!(
    end_to_end_when_byte_range_check_then_correct,
    "FUNCTION IS_UPPERCASE : BOOL VAR_INPUT c : BYTE; END_VAR IS_UPPERCASE := c >= BYTE#65 AND c <= BYTE#90; END_FUNCTION PROGRAM main VAR yes : BOOL; no : BOOL; END_VAR yes := IS_UPPERCASE(c := BYTE#72); no := IS_UPPERCASE(c := BYTE#97); END_PROGRAM",
    &[(0, 1), (1, 0)],
);
