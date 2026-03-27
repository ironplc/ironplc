//! End-to-end integration tests for the MIN function with float types.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_min_real_then_returns_smaller() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 7.5;
  y := MIN(x, 3.0);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f32();
    assert!((y - 3.0).abs() < 1e-5, "expected 3.0, got {y}");
}

#[test]
fn end_to_end_when_min_real_first_smaller_then_returns_first() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 2.0;
  y := MIN(x, 8.0);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f32();
    assert!((y - 2.0).abs() < 1e-5, "expected 2.0, got {y}");
}

#[test]
fn end_to_end_when_min_lreal_then_returns_smaller() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := 7.5;
  y := MIN(x, 3.0);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f64();
    assert!((y - 3.0).abs() < 1e-12, "expected 3.0, got {y}");
}
