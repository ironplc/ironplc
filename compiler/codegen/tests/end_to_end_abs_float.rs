//! End-to-end integration tests for the ABS function with float types.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_abs_real_positive_then_unchanged() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 42.5;
  y := ABS(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f32();
    assert!((y - 42.5).abs() < 1e-5, "expected 42.5, got {y}");
}

#[test]
fn end_to_end_when_abs_real_negative_then_positive() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := -7.25;
  y := ABS(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f32();
    assert!((y - 7.25).abs() < 1e-5, "expected 7.25, got {y}");
}

#[test]
fn end_to_end_when_abs_lreal_negative_then_positive() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := -3.141592653589793;
  y := ABS(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let y = bufs.vars[1].as_f64();
    assert!(
        (y - 3.141592653589793).abs() < 1e-12,
        "expected pi, got {y}"
    );
}
