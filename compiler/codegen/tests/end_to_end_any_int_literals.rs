//! End-to-end integration tests for bare literal type inference (ANY_INT / ANY_REAL).
//!
//! Bare integer literals (e.g. `5`) resolve as ANY_INT and are compatible with
//! any integer parameter type. Bare real literals resolve as ANY_REAL and are
//! compatible with any real parameter type.

#[macro_use]
mod common;

use common::parse_and_run;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

// Integer bare-literal cases: each declares a FUNCTION that takes one typed
// parameter and returns it via a small expression, then calls it from a
// PROGRAM with a bare literal argument. vars[0] holds the PROGRAM's result.
#[rstest]
#[case::bare_int_to_int_param(
    "ADD_ONE",
    "INT",
    "ADD_ONE := x + INT#1;",
    "5",
    6
)]
#[case::bare_int_to_sint_param(
    "DOUBLE",
    "SINT",
    "DOUBLE := x + x;",
    "7",
    14
)]
#[case::bare_int_to_dint_param(
    "TRIPLE",
    "DINT",
    "TRIPLE := x + x + x;",
    "100",
    300
)]
fn end_to_end_bare_int_literal(
    #[case] fn_name: &str,
    #[case] ty: &str,
    #[case] body: &str,
    #[case] arg: &str,
    #[case] expected: i32,
) {
    let source = format!(
        "FUNCTION {fn_name} : {ty} VAR_INPUT x : {ty}; END_VAR {body} END_FUNCTION PROGRAM main VAR result : {ty}; END_VAR result := {fn_name}({arg}); END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), expected);
}

// Bare INT literal in an expression with an INT variable. 10 + 5 = 15.
e2e_i32!(
    end_to_end_when_bare_literal_in_expression_with_int_var_then_correct,
    "PROGRAM main VAR x : INT; result : INT; END_VAR x := INT#10; result := x + 5; END_PROGRAM",
    &[(1, 15)],
);

// LREAL / REAL cases — float assertions need near-equality, so keep as raw
// tests. (`e2e_f32_near!` exists but these also vary in the function body.)
#[test]
fn end_to_end_when_bare_real_literal_to_lreal_param_then_correct() {
    let source = "FUNCTION ADD_PI : LREAL VAR_INPUT x : LREAL; END_VAR ADD_PI := x; END_FUNCTION PROGRAM main VAR result : LREAL; END_VAR result := ADD_PI(3.14); END_PROGRAM";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert!((bufs.vars[0].as_f64() - 3.14).abs() < 0.001);
}

#[test]
fn end_to_end_when_bare_int_literal_to_real_param_then_correct() {
    let source = "FUNCTION HALVE : REAL VAR_INPUT x : REAL; END_VAR HALVE := x / REAL#2.0; END_FUNCTION PROGRAM main VAR result : REAL; END_VAR result := HALVE(10); END_PROGRAM";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert!((bufs.vars[0].as_f32() - 5.0).abs() < 1e-5);
}

#[test]
fn end_to_end_when_bare_int_literal_to_lreal_param_then_correct() {
    let source = "FUNCTION IDENTITY_LREAL : LREAL VAR_INPUT x : LREAL; END_VAR IDENTITY_LREAL := x; END_FUNCTION PROGRAM main VAR result : LREAL; END_VAR result := IDENTITY_LREAL(42); END_PROGRAM";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert!((bufs.vars[0].as_f64() - 42.0).abs() < 1e-10);
}
