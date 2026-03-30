//! End-to-end tests for integer widening type conversions.

mod common;
use ironplc_container::VarIndex;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

#[test]
fn end_to_end_when_sint_to_int_then_widens() {
    let source = "
PROGRAM main
  VAR
    x : SINT;
    y : INT;
  END_VAR
  x := -100;
  y := SINT_TO_INT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), -100);
}

#[test]
fn end_to_end_when_int_to_dint_then_widens() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : DINT;
  END_VAR
  x := -30000;
  y := INT_TO_DINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), -30000);
}

#[test]
fn end_to_end_when_dint_to_lint_then_widens() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : LINT;
  END_VAR
  x := -1000000;
  y := DINT_TO_LINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i64(), -1000000);
}

#[test]
fn end_to_end_when_usint_to_uint_then_widens() {
    let source = "
PROGRAM main
  VAR
    x : USINT;
    y : UINT;
  END_VAR
  x := 200;
  y := USINT_TO_UINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32() as u16, 200);
}

#[test]
fn end_to_end_when_uint_to_ulint_then_widens() {
    let source = "
PROGRAM main
  VAR
    x : UINT;
    y : ULINT;
  END_VAR
  x := 50000;
  y := UINT_TO_ULINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i64() as u64, 50000);
}

#[test]
fn end_to_end_when_int_to_uint_then_reinterprets() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : UINT;
  END_VAR
  x := 1000;
  y := INT_TO_UINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32() as u16, 1000);
}
