//! End-to-end integration tests for the MAX function with float types.

mod common;
use ironplc_parser::options::ParseOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_max_real_then_returns_larger() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 3.0;
  y := MAX(x, 7.5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    let y = bufs.vars[1].as_f32();
    assert!((y - 7.5).abs() < 1e-5, "expected 7.5, got {y}");
}

#[test]
fn end_to_end_when_max_real_first_larger_then_returns_first() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 8.0;
  y := MAX(x, 2.0);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    let y = bufs.vars[1].as_f32();
    assert!((y - 8.0).abs() < 1e-5, "expected 8.0, got {y}");
}

#[test]
fn end_to_end_when_max_lreal_then_returns_larger() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 3.0;
  y := MAX(x, 7.5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    let y = bufs.vars[1].as_f64();
    assert!((y - 7.5).abs() < 1e-12, "expected 7.5, got {y}");
}
