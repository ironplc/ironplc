//! End-to-end integration tests for function forms of operators.
//!
//! One smoke test per function form to verify the full pipeline works.
//! Detailed opcode testing is in compile_func_forms.rs.
//!
//! Note: NOT(x) is tested via the unary operator path since the parser
//! treats NOT as a unary operator applied to parenthesized expression (x).

#[macro_use]
mod common;

use common::parse_and_run;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

// Binary arithmetic functions with literal operands:
//   result := <FN>(<a>, <b>);
// Produces a single DINT result in `result`.
#[rstest]
#[case::sub("SUB", "10, 3", 7)]
#[case::mul("MUL", "6, 7", 42)]
#[case::div("DIV", "20, 4", 5)]
#[case::mod_("MOD", "10, 3", 1)]
fn end_to_end_arith_function(#[case] func: &str, #[case] args: &str, #[case] expected: i32) {
    let source =
        format!("PROGRAM main VAR result : DINT; END_VAR result := {func}({args}); END_PROGRAM");
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), expected);
}

// Two-case boolean/comparison functions:
//   true_result := <FN>(<args_true>);   false_result := <FN>(<args_false>);
// Every entry has the same VAR envelope and asserts vars[0]=1, vars[1]=0.
#[rstest]
#[case::gt("GT", "10, 5", "5, 10")]
#[case::eq("EQ", "5, 5", "5, 10")]
#[case::le("LE", "5, 10", "10, 5")]
#[case::lt("LT", "5, 10", "5, 5")]
#[case::ne("NE", "5, 10", "5, 5")]
#[case::and("AND", "TRUE, TRUE", "TRUE, FALSE")]
#[case::or("OR", "FALSE, TRUE", "FALSE, FALSE")]
#[case::xor("XOR", "TRUE, FALSE", "TRUE, TRUE")]
// NOT(x) parses as unary NOT applied to parenthesized expression (x).
#[case::not_parens("NOT", "FALSE", "TRUE")]
fn end_to_end_bool_function(#[case] func: &str, #[case] args_true: &str, #[case] args_false: &str) {
    let source = format!(
        "PROGRAM main VAR true_result : DINT; false_result : DINT; END_VAR true_result := {func}({args_true}); false_result := {func}({args_false}); END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 1);
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

// ADD uses a variable operand, giving it a distinct shape (2 user vars, both asserted).
e2e_i32!(
    end_to_end_when_add_function_then_returns_sum,
    "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := 10; y := ADD(x, 32); END_PROGRAM",
    &[(0, 10), (1, 42)],
);

// GE has three cases (true / equal / false) rather than the usual two.
e2e_i32!(
    end_to_end_when_ge_function_then_returns_bool,
    "PROGRAM main VAR true_result : DINT; equal_result : DINT; false_result : DINT; END_VAR true_result := GE(10, 5); equal_result := GE(5, 5); false_result := GE(5, 10); END_PROGRAM",
    &[(0, 1), (1, 1), (2, 0)],
);

// MOVE from a var into a var — separate shape from the binary operators.
e2e_i32!(
    end_to_end_when_move_function_then_returns_input_value,
    "PROGRAM main VAR x : DINT; result : DINT; END_VAR x := 42; result := MOVE(x); END_PROGRAM",
    &[(0, 42), (1, 42)],
);
