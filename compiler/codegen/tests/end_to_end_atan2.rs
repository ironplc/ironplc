//! End-to-end integration tests for ATAN2 function.

mod common;

use common::parse_and_run;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

// REAL ATAN2 cases: vars[2] is the result, near-equality tolerance 1e-5.
#[rstest]
#[case::one_one_pi_quarter(1.0_f32, 1.0, std::f32::consts::FRAC_PI_4)]
#[case::zero_one_zero(0.0, 1.0, 0.0)]
#[case::one_zero_pi_half(1.0, 0.0, std::f32::consts::FRAC_PI_2)]
fn end_to_end_atan2_real(#[case] y: f32, #[case] x: f32, #[case] expected: f32) {
    let source = format!(
        "PROGRAM main VAR y : REAL; x : REAL; result : REAL; END_VAR y := {y}; x := {x}; result := ATAN2(y, x); END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    let r = bufs.vars[2].as_f32();
    assert!((r - expected).abs() < 1e-5, "expected {expected}, got {r}");
}

// LREAL ATAN2: -1, -1 lands in the third quadrant at -3π/4.
#[test]
fn end_to_end_when_atan2_lreal_neg_one_neg_one_then_neg_three_pi_quarter() {
    let source = "PROGRAM main VAR y : LREAL; x : LREAL; result : LREAL; END_VAR y := -1.0; x := -1.0; result := ATAN2(y, x); END_PROGRAM";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    let r = bufs.vars[2].as_f64();
    let expected = -3.0 * std::f64::consts::FRAC_PI_4;
    assert!((r - expected).abs() < 1e-12, "expected {expected}, got {r}");
}
