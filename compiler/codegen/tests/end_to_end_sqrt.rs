//! End-to-end integration tests for the SQRT function.

mod common;
use ironplc_container::VarIndex;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_sqrt_real_perfect_square_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 9.0;
  y := SQRT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f32();
    assert!((y - 3.0).abs() < 1e-5, "expected 3.0, got {y}");
}

#[test]
fn end_to_end_when_sqrt_real_zero_then_zero() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 0.0;
  y := SQRT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f32();
    assert!((y - 0.0).abs() < 1e-5, "expected 0.0, got {y}");
}

#[test]
fn end_to_end_when_sqrt_real_negative_then_nan() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := -1.0;
  y := SQRT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f32();
    assert!(y.is_nan(), "expected NaN, got {y}");
}

#[test]
fn end_to_end_when_sqrt_lreal_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 2.0;
  y := SQRT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f64();
    assert!(
        (y - std::f64::consts::SQRT_2).abs() < 1e-12,
        "expected sqrt(2), got {y}"
    );
}

#[test]
fn end_to_end_when_sqrt_lreal_negative_then_nan() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := -1.0;
  y := SQRT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f64();
    assert!(y.is_nan(), "expected NaN, got {y}");
}
