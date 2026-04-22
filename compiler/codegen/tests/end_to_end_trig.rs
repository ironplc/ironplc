//! End-to-end integration tests for SIN, COS, TAN, ASIN, ACOS, ATAN functions.

mod common;

use common::parse_and_run;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

// REAL (f32) trig cases: x : REAL; y : REAL; y := <fn>(x);
#[rstest]
#[case::sin_zero("SIN", 0.0_f32, 0.0)]
#[case::cos_zero("COS", 0.0, 1.0)]
#[case::tan_zero("TAN", 0.0, 0.0)]
#[case::asin_zero("ASIN", 0.0, 0.0)]
#[case::acos_one("ACOS", 1.0, 0.0)]
#[case::atan_zero("ATAN", 0.0, 0.0)]
fn end_to_end_trig_real(#[case] func: &str, #[case] input: f32, #[case] expected: f32) {
    let source = format!(
        "PROGRAM main VAR x : REAL; y : REAL; END_VAR x := {input}; y := {func}(x); END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    let y = bufs.vars[1].as_f32();
    assert!((y - expected).abs() < 1e-5, "expected {expected}, got {y}");
}

// LREAL (f64) trig cases: x : LREAL; y : LREAL; y := <fn>(x); with tighter tolerance.
#[rstest]
#[case::sin_pi_half("SIN", std::f64::consts::FRAC_PI_2, 1.0)]
#[case::cos_pi("COS", std::f64::consts::PI, -1.0)]
#[case::tan_pi_quarter("TAN", std::f64::consts::FRAC_PI_4, 1.0)]
#[case::asin_one("ASIN", 1.0, std::f64::consts::FRAC_PI_2)]
#[case::acos_zero("ACOS", 0.0, std::f64::consts::FRAC_PI_2)]
#[case::atan_one("ATAN", 1.0, std::f64::consts::FRAC_PI_4)]
fn end_to_end_trig_lreal(#[case] func: &str, #[case] input: f64, #[case] expected: f64) {
    let source = format!(
        "PROGRAM main VAR x : LREAL; y : LREAL; END_VAR x := {input}; y := {func}(x); END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    let y = bufs.vars[1].as_f64();
    assert!((y - expected).abs() < 1e-12, "expected {expected}, got {y}");
}
