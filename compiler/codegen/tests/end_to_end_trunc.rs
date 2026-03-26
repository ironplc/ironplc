//! End-to-end integration tests for the TRUNC function.

mod common;
use ironplc_parser::options::ParseOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_trunc_real_positive_then_truncates_toward_zero() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : DINT;
  END_VAR
  x := 3.7;
  y := TRUNC(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    let y = bufs.vars[1].as_i32();
    assert_eq!(y, 3, "expected 3, got {y}");
}

#[test]
fn end_to_end_when_trunc_real_negative_then_truncates_toward_zero() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : DINT;
  END_VAR
  x := -3.7;
  y := TRUNC(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    let y = bufs.vars[1].as_i32();
    assert_eq!(y, -3, "expected -3, got {y}");
}

#[test]
fn end_to_end_when_trunc_real_zero_then_zero() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : DINT;
  END_VAR
  x := 0.0;
  y := TRUNC(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    let y = bufs.vars[1].as_i32();
    assert_eq!(y, 0, "expected 0, got {y}");
}

#[test]
fn end_to_end_when_trunc_lreal_positive_then_truncates_toward_zero() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LINT;
  END_VAR
  x := 99.9;
  y := TRUNC(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    let y = bufs.vars[1].as_i64();
    assert_eq!(y, 99, "expected 99, got {y}");
}

#[test]
fn end_to_end_when_trunc_lreal_negative_then_truncates_toward_zero() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LINT;
  END_VAR
  x := -99.9;
  y := TRUNC(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    let y = bufs.vars[1].as_i64();
    assert_eq!(y, -99, "expected -99, got {y}");
}
