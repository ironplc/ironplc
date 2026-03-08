//! End-to-end tests for BOOL to integer type conversions.

mod common;

use common::parse_and_run;

#[test]
fn end_to_end_when_bool_to_sint_true_then_returns_1() {
    let source = "
PROGRAM main
  VAR
    x : BOOL;
    y : SINT;
  END_VAR
  x := TRUE;
  y := BOOL_TO_SINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32() as i8, 1);
}

#[test]
fn end_to_end_when_bool_to_sint_false_then_returns_0() {
    let source = "
PROGRAM main
  VAR
    x : BOOL;
    y : SINT;
  END_VAR
  x := FALSE;
  y := BOOL_TO_SINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32() as i8, 0);
}

#[test]
fn end_to_end_when_bool_to_int_true_then_returns_1() {
    let source = "
PROGRAM main
  VAR
    x : BOOL;
    y : INT;
  END_VAR
  x := TRUE;
  y := BOOL_TO_INT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_bool_to_int_false_then_returns_0() {
    let source = "
PROGRAM main
  VAR
    x : BOOL;
    y : INT;
  END_VAR
  x := FALSE;
  y := BOOL_TO_INT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_bool_to_dint_true_then_returns_1() {
    let source = "
PROGRAM main
  VAR
    x : BOOL;
    y : DINT;
  END_VAR
  x := TRUE;
  y := BOOL_TO_DINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_bool_to_lint_true_then_returns_1() {
    let source = "
PROGRAM main
  VAR
    x : BOOL;
    y : LINT;
  END_VAR
  x := TRUE;
  y := BOOL_TO_LINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i64(), 1);
}

#[test]
fn end_to_end_when_bool_to_usint_true_then_returns_1() {
    let source = "
PROGRAM main
  VAR
    x : BOOL;
    y : USINT;
  END_VAR
  x := TRUE;
  y := BOOL_TO_USINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32() as u8, 1);
}

#[test]
fn end_to_end_when_bool_to_uint_true_then_returns_1() {
    let source = "
PROGRAM main
  VAR
    x : BOOL;
    y : UINT;
  END_VAR
  x := TRUE;
  y := BOOL_TO_UINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32() as u16, 1);
}

#[test]
fn end_to_end_when_bool_to_udint_true_then_returns_1() {
    let source = "
PROGRAM main
  VAR
    x : BOOL;
    y : UDINT;
  END_VAR
  x := TRUE;
  y := BOOL_TO_UDINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i32() as u32, 1);
}

#[test]
fn end_to_end_when_bool_to_ulint_true_then_returns_1() {
    let source = "
PROGRAM main
  VAR
    x : BOOL;
    y : ULINT;
  END_VAR
  x := TRUE;
  y := BOOL_TO_ULINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i64() as u64, 1);
}

#[test]
fn end_to_end_when_bool_to_ulint_false_then_returns_0() {
    let source = "
PROGRAM main
  VAR
    x : BOOL;
    y : ULINT;
  END_VAR
  x := FALSE;
  y := BOOL_TO_ULINT(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i64() as u64, 0);
}
