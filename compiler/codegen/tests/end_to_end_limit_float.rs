//! End-to-end integration tests for the LIMIT function with float types.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_limit_real_in_range_then_unchanged() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 5.0;
  y := LIMIT(0.0, x, 10.0);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f32();
    assert!((y - 5.0).abs() < 1e-5, "expected 5.0, got {y}");
}

#[test]
fn end_to_end_when_limit_real_below_min_then_clamped() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := -5.0;
  y := LIMIT(0.0, x, 10.0);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f32();
    assert!((y - 0.0).abs() < 1e-5, "expected 0.0, got {y}");
}

#[test]
fn end_to_end_when_limit_real_above_max_then_clamped() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : REAL;
  END_VAR
  x := 99.0;
  y := LIMIT(0.0, x, 10.0);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f32();
    assert!((y - 10.0).abs() < 1e-5, "expected 10.0, got {y}");
}

#[test]
fn end_to_end_when_limit_lreal_below_min_then_clamped() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LREAL;
  END_VAR
  x := -5.0;
  y := LIMIT(0.0, x, 10.0);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let y = bufs.vars[1].as_f64();
    assert!((y - 0.0).abs() < 1e-12, "expected 0.0, got {y}");
}
