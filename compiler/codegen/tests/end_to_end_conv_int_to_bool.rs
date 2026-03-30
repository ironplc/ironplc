//! End-to-end tests for integer to BOOL type conversions.

mod common;
use ironplc_container::VarIndex;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

// --- Signed integer to BOOL ---

#[test]
fn end_to_end_when_sint_to_bool_nonzero_then_returns_true() {
    let source = "
PROGRAM main
  VAR
    x : SINT;
    y : BOOL;
  END_VAR
  x := 5;
  y := SINT_TO_BOOL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_sint_to_bool_zero_then_returns_false() {
    let source = "
PROGRAM main
  VAR
    x : SINT;
    y : BOOL;
  END_VAR
  x := 0;
  y := SINT_TO_BOOL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_int_to_bool_nonzero_then_returns_true() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : BOOL;
  END_VAR
  x := 42;
  y := INT_TO_BOOL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_int_to_bool_zero_then_returns_false() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : BOOL;
  END_VAR
  x := 0;
  y := INT_TO_BOOL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_dint_to_bool_nonzero_then_returns_true() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : BOOL;
  END_VAR
  x := 1000;
  y := DINT_TO_BOOL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_dint_to_bool_zero_then_returns_false() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
    y : BOOL;
  END_VAR
  x := 0;
  y := DINT_TO_BOOL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_lint_to_bool_nonzero_then_returns_true() {
    let source = "
PROGRAM main
  VAR
    x : LINT;
    y : BOOL;
  END_VAR
  x := 100000;
  y := LINT_TO_BOOL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_lint_to_bool_zero_then_returns_false() {
    let source = "
PROGRAM main
  VAR
    x : LINT;
    y : BOOL;
  END_VAR
  x := 0;
  y := LINT_TO_BOOL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

// --- Unsigned integer to BOOL ---

#[test]
fn end_to_end_when_usint_to_bool_nonzero_then_returns_true() {
    let source = "
PROGRAM main
  VAR
    x : USINT;
    y : BOOL;
  END_VAR
  x := 1;
  y := USINT_TO_BOOL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_uint_to_bool_nonzero_then_returns_true() {
    let source = "
PROGRAM main
  VAR
    x : UINT;
    y : BOOL;
  END_VAR
  x := 255;
  y := UINT_TO_BOOL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_udint_to_bool_nonzero_then_returns_true() {
    let source = "
PROGRAM main
  VAR
    x : UDINT;
    y : BOOL;
  END_VAR
  x := 1000;
  y := UDINT_TO_BOOL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_ulint_to_bool_nonzero_then_returns_true() {
    let source = "
PROGRAM main
  VAR
    x : ULINT;
    y : BOOL;
  END_VAR
  x := 999;
  y := ULINT_TO_BOOL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_ulint_to_bool_zero_then_returns_false() {
    let source = "
PROGRAM main
  VAR
    x : ULINT;
    y : BOOL;
  END_VAR
  x := 0;
  y := ULINT_TO_BOOL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

// --- Edge case: value 2 should be TRUE (not FALSE from bit truncation) ---

#[test]
fn end_to_end_when_int_to_bool_value_2_then_returns_true() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : BOOL;
  END_VAR
  x := 2;
  y := INT_TO_BOOL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_sint_to_bool_negative_then_returns_true() {
    let source = "
PROGRAM main
  VAR
    x : SINT;
    y : BOOL;
  END_VAR
  x := -1;
  y := SINT_TO_BOOL(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 1);
}
