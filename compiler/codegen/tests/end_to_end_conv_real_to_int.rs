//! End-to-end tests for real-to-integer type conversions.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_real_to_int_then_truncates() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : INT;
  END_VAR
  x := 3.14;
  y := REAL_TO_INT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 3);
}

#[test]
fn end_to_end_when_real_to_dint_negative_then_truncates() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : DINT;
  END_VAR
  x := -7.9;
  y := REAL_TO_DINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), -7);
}

#[test]
fn end_to_end_when_lreal_to_lint_then_truncates() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : LINT;
  END_VAR
  x := 99.9;
  y := LREAL_TO_LINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i64(), 99);
}

#[test]
fn end_to_end_when_real_to_sint_then_truncates_to_range() {
    let source = "
PROGRAM main
  VAR
    x : REAL;
    y : SINT;
  END_VAR
  x := 50.7;
  y := REAL_TO_SINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 50);
}

#[test]
fn end_to_end_when_lreal_to_udint_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LREAL;
    y : UDINT;
  END_VAR
  x := 1000.0;
  y := LREAL_TO_UDINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32() as u32, 1000);
}
