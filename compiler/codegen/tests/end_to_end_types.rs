//! End-to-end tests for multi-width integer type support.
//!
//! Each test verifies the full pipeline: parse -> compile -> VM execution
//! for a specific IEC 61131-3 integer type. Tests cover basic assignment
//! and overflow/wrapping behavior for each type.

mod common;

use common::parse_and_run;

// --- SINT (8-bit signed, -128..127) ---

#[test]
fn end_to_end_when_sint_assignment_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : SINT;
  END_VAR
  x := 42;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 42);
}

#[test]
fn end_to_end_when_sint_overflow_then_wraps() {
    let source = "
PROGRAM main
  VAR
    x : SINT;
  END_VAR
  x := 127 + 1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 128 truncated to i8 wraps to -128
    assert_eq!(bufs.vars[0].as_i32(), -128);
}

// --- INT (16-bit signed, -32768..32767) ---

#[test]
fn end_to_end_when_int_assignment_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  x := 1000;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 1000);
}

#[test]
fn end_to_end_when_int_overflow_then_wraps() {
    let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  x := 32767 + 1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 32768 truncated to i16 wraps to -32768
    assert_eq!(bufs.vars[0].as_i32(), -32768);
}

// --- DINT (32-bit signed) ---

#[test]
fn end_to_end_when_dint_assignment_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 42;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 42);
}

#[test]
fn end_to_end_when_dint_large_value_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 100000;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 100000);
}

// --- LINT (64-bit signed) ---

#[test]
fn end_to_end_when_lint_assignment_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LINT;
  END_VAR
  x := 42;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64(), 42);
}

#[test]
fn end_to_end_when_lint_large_value_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LINT;
    y : LINT;
  END_VAR
  x := 3000000000;
  y := x + 1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64(), 3_000_000_000);
    assert_eq!(bufs.vars[1].as_i64(), 3_000_000_001);
}

// --- USINT (8-bit unsigned, 0..255) ---

#[test]
fn end_to_end_when_usint_assignment_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : USINT;
  END_VAR
  x := 200;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 200);
}

#[test]
fn end_to_end_when_usint_overflow_then_wraps() {
    let source = "
PROGRAM main
  VAR
    x : USINT;
  END_VAR
  x := 255 + 1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 256 truncated to u8 wraps to 0
    assert_eq!(bufs.vars[0].as_i32(), 0);
}

// --- UINT (16-bit unsigned, 0..65535) ---

#[test]
fn end_to_end_when_uint_assignment_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : UINT;
  END_VAR
  x := 50000;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 50000);
}

#[test]
fn end_to_end_when_uint_overflow_then_wraps() {
    let source = "
PROGRAM main
  VAR
    x : UINT;
  END_VAR
  x := 65535 + 1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 65536 truncated to u16 wraps to 0
    assert_eq!(bufs.vars[0].as_i32(), 0);
}

// --- UDINT (32-bit unsigned) ---

#[test]
fn end_to_end_when_udint_assignment_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : UDINT;
  END_VAR
  x := 42;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 42);
}

#[test]
fn end_to_end_when_udint_comparison_then_unsigned() {
    let source = "
PROGRAM main
  VAR
    result : UDINT;
    a : UDINT;
    b : UDINT;
  END_VAR
  a := 3000000000;
  b := 2000000000;
  IF a > b THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 3B > 2B is true when treated as unsigned (3B as i32 is negative)
    assert_eq!(bufs.vars[0].as_i32(), 1);
}

// --- ULINT (64-bit unsigned) ---

#[test]
fn end_to_end_when_ulint_assignment_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : ULINT;
  END_VAR
  x := 42;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64(), 42);
}

#[test]
fn end_to_end_when_ulint_large_value_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : ULINT;
    y : ULINT;
  END_VAR
  x := 5000000000;
  y := x + 1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64(), 5_000_000_000);
    assert_eq!(bufs.vars[1].as_i64(), 5_000_000_001);
}
