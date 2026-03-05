//! End-to-end tests for real-to-real type conversions.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_real_to_lreal_then_widens() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : LREAL;
  END_VAR
  x := 3.14;
  y := REAL_TO_LREAL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    let y = bufs.vars[1].as_f64();
    assert!((y - 3.14).abs() < 0.01, "expected ~3.14, got {y}");
}

#[test]
fn end_to_end_when_lreal_to_real_then_narrows() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : REAL;
  END_VAR
  x := 2.718281828;
  y := LREAL_TO_REAL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    let y = bufs.vars[1].as_f32();
    assert!((y - 2.718282).abs() < 1e-4, "expected ~2.718282, got {y}");
}
