//! End-to-end integration tests for LN, LOG, EXP functions.

mod common;

use common::parse_and_run;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

// REAL (f32) math cases: x : REAL; y : REAL; y := <fn>(x);
#[rstest]
#[case::ln_e("LN", 2.718282, 1.0)]
#[case::log_100("LOG", 100.0, 2.0)]
#[case::exp_zero("EXP", 0.0, 1.0)]
fn end_to_end_math_real(#[case] func: &str, #[case] input: f32, #[case] expected: f32) {
    let source = format!(
        "PROGRAM main VAR x : REAL; y : REAL; END_VAR x := {input}; y := {func}(x); END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    let y = bufs.vars[1].as_f32();
    // LN / LOG require coarser tolerance because their inputs are themselves
    // rounded; EXP is exact at 0.
    let tolerance = if func == "LN" { 1e-4 } else { 1e-5 };
    assert!(
        (y - expected).abs() < tolerance,
        "expected ~{expected}, got {y}"
    );
}

// LREAL (f64) math cases: tighter tolerance.
#[rstest]
#[case::ln_one("LN", 1.0, 0.0)]
#[case::log_1000("LOG", 1000.0, 3.0)]
#[case::exp_one("EXP", 1.0, std::f64::consts::E)]
fn end_to_end_math_lreal(#[case] func: &str, #[case] input: f64, #[case] expected: f64) {
    let source = format!(
        "PROGRAM main VAR x : LREAL; y : LREAL; END_VAR x := {input}; y := {func}(x); END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    let y = bufs.vars[1].as_f64();
    assert!((y - expected).abs() < 1e-12, "expected {expected}, got {y}");
}
