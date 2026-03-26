//! End-to-end tests for integer narrowing type conversions.

mod common;
use ironplc_parser::options::ParseOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_dint_to_int_then_narrows() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : INT;
  END_VAR
  x := 1000;
  y := DINT_TO_INT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 1000);
}

#[test]
fn end_to_end_when_lint_to_dint_then_narrows() {
    let source = "
PROGRAM main
  VAR
    x : LINT;
    y : DINT;
  END_VAR
  x := 42;
  y := LINT_TO_DINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 42);
}

#[test]
fn end_to_end_when_dint_to_sint_overflow_then_wraps() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : SINT;
  END_VAR
  x := 300;
  y := DINT_TO_SINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());
    // 300 mod 256 = 44 (wrapping to i8 range)
    assert_eq!(bufs.vars[1].as_i32() as i8, 44);
}

#[test]
fn end_to_end_when_lint_to_sint_then_narrows() {
    let source = "
PROGRAM main
  VAR
    x : LINT;
    y : SINT;
  END_VAR
  x := 50;
  y := LINT_TO_SINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 50);
}

#[test]
fn end_to_end_when_ulint_to_udint_then_narrows() {
    let source = "
PROGRAM main
  VAR
    x : ULINT;
    y : UDINT;
  END_VAR
  x := 1000;
  y := ULINT_TO_UDINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());
    assert_eq!(bufs.vars[1].as_i32() as u32, 1000);
}
