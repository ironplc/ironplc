//! End-to-end integration tests for boolean operators.

#[macro_use]
mod common;

use common::parse_and_run;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

// Initial-value and literal-assignment tests: single var, single expected value.
e2e_i32!(
    end_to_end_when_bool_initial_value_true_then_variable_initialized,
    "PROGRAM main VAR x : BOOL := TRUE; END_VAR END_PROGRAM",
    &[(0, 1)],
);

e2e_i32!(
    end_to_end_when_bool_initial_value_false_then_variable_initialized,
    "PROGRAM main VAR x : BOOL := FALSE; END_VAR END_PROGRAM",
    &[(0, 0)],
);

e2e_i32!(
    end_to_end_when_true_literal_then_one,
    "PROGRAM main VAR y : DINT; END_VAR y := TRUE; END_PROGRAM",
    &[(0, 1)],
);

e2e_i32!(
    end_to_end_when_false_literal_then_zero,
    "PROGRAM main VAR y : DINT; END_VAR y := FALSE; END_PROGRAM",
    &[(0, 0)],
);

// Binary and unary boolean-operator tests share the same envelope:
//   VAR x, y : DINT;  x := <lhs>;  y := <expr>;
// so `expected_x` / `expected_y` are enough to describe each case.
#[rstest]
#[case::and_both_true(5, "x > 0 AND x < 10", 1)]
#[case::and_one_false(15, "x > 0 AND x < 10", 0)]
#[case::or_first_true(5, "x > 10 OR x < 10", 1)]
#[case::or_both_false(5, "x > 10 OR x < 0", 0)]
#[case::xor_one_true(5, "x > 10 XOR x < 10", 1)]
#[case::xor_both_true(5, "x > 0 XOR x < 10", 0)]
#[case::not_zero(0, "NOT x", 1)]
#[case::not_nonzero(5, "NOT x", 0)]
fn end_to_end_bool_op(#[case] x: i32, #[case] expr: &str, #[case] expected_y: i32) {
    let source = format!(
        "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := {x}; y := {expr}; END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), x);
    assert_eq!(bufs.vars[1].as_i32(), expected_y);
}
