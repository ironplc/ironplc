//! End-to-end integration tests for ATAN2 function.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_atan2_real_one_one_then_pi_quarter() {
    let source = "
PROGRAM main
  VAR
    y : REAL;
    x : REAL;
    result : REAL;
  END_VAR
  y := 1.0;
  x := 1.0;
  result := ATAN2(y, x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let r = bufs.vars[2].as_f32();
    assert!(
        (r - std::f32::consts::FRAC_PI_4).abs() < 1e-5,
        "expected PI/4, got {r}"
    );
}

#[test]
fn end_to_end_when_atan2_real_zero_one_then_zero() {
    let source = "
PROGRAM main
  VAR
    y : REAL;
    x : REAL;
    result : REAL;
  END_VAR
  y := 0.0;
  x := 1.0;
  result := ATAN2(y, x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let r = bufs.vars[2].as_f32();
    assert!((r - 0.0).abs() < 1e-5, "expected 0.0, got {r}");
}

#[test]
fn end_to_end_when_atan2_real_one_zero_then_pi_half() {
    let source = "
PROGRAM main
  VAR
    y : REAL;
    x : REAL;
    result : REAL;
  END_VAR
  y := 1.0;
  x := 0.0;
  result := ATAN2(y, x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let r = bufs.vars[2].as_f32();
    assert!(
        (r - std::f32::consts::FRAC_PI_2).abs() < 1e-5,
        "expected PI/2, got {r}"
    );
}

#[test]
fn end_to_end_when_atan2_lreal_neg_one_neg_one_then_neg_three_pi_quarter() {
    let source = "
PROGRAM main
  VAR
    y : LREAL;
    x : LREAL;
    result : LREAL;
  END_VAR
  y := -1.0;
  x := -1.0;
  result := ATAN2(y, x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let r = bufs.vars[2].as_f64();
    let expected = -3.0 * std::f64::consts::FRAC_PI_4;
    assert!((r - expected).abs() < 1e-12, "expected -3*PI/4, got {r}");
}
