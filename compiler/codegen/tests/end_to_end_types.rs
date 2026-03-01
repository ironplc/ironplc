//! End-to-end tests for multi-width integer type support.
//!
//! Each test verifies the full pipeline: parse -> compile -> VM execution
//! for a specific IEC 61131-3 integer type. Tests cover assignment,
//! overflow/wrapping, sign/zero extension, arithmetic, comparison,
//! and unsigned semantics for each type.

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
fn end_to_end_when_sint_sign_extend_then_preserves_negative() {
    let source = "
PROGRAM main
  VAR
    x : SINT;
    y : SINT;
  END_VAR
  x := -5;
  y := x + 1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // -5 truncated to i8 sign-extends back to -5 in i32; -5 + 1 = -4
    assert_eq!(bufs.vars[0].as_i32(), -5);
    assert_eq!(bufs.vars[1].as_i32(), -4);
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
fn end_to_end_when_int_sign_extend_then_preserves_negative() {
    let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := -100;
  y := x + 1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // -100 truncated to i16 sign-extends back to -100 in i32; -100 + 1 = -99
    assert_eq!(bufs.vars[0].as_i32(), -100);
    assert_eq!(bufs.vars[1].as_i32(), -99);
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

#[test]
fn end_to_end_when_lint_subtraction_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LINT;
    y : LINT;
  END_VAR
  x := 5000000000;
  y := x - 1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i64(), 4_999_999_999);
}

#[test]
fn end_to_end_when_lint_multiplication_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LINT;
    y : LINT;
  END_VAR
  x := 1000000;
  y := x * 1000000;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 1_000_000 * 1_000_000 = 1_000_000_000_000 (exceeds i32 range)
    assert_eq!(bufs.vars[1].as_i64(), 1_000_000_000_000);
}

#[test]
fn end_to_end_when_lint_division_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LINT;
    y : LINT;
  END_VAR
  x := 10000000000;
  y := x / 2;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i64(), 5_000_000_000);
}

#[test]
fn end_to_end_when_lint_modulo_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LINT;
    y : LINT;
  END_VAR
  x := 10000000001;
  y := x MOD 10000000000;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i64(), 1);
}

#[test]
fn end_to_end_when_lint_negation_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : LINT;
    y : LINT;
  END_VAR
  x := 3000000000;
  y := -x;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i64(), -3_000_000_000);
}

#[test]
fn end_to_end_when_lint_comparison_then_correct() {
    let source = "
PROGRAM main
  VAR
    result : LINT;
    a : LINT;
    b : LINT;
  END_VAR
  a := 5000000000;
  b := 3000000000;
  IF a > b THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64(), 1);
}

#[test]
fn end_to_end_when_lint_equal_then_correct() {
    let source = "
PROGRAM main
  VAR
    result : LINT;
    a : LINT;
    b : LINT;
  END_VAR
  a := 5000000000;
  b := 5000000000;
  IF a = b THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64(), 1);
}

#[test]
fn end_to_end_when_lint_not_equal_then_correct() {
    let source = "
PROGRAM main
  VAR
    result : LINT;
    a : LINT;
    b : LINT;
  END_VAR
  a := 5000000000;
  b := 3000000000;
  IF a <> b THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64(), 1);
}

#[test]
fn end_to_end_when_lint_less_than_then_correct() {
    let source = "
PROGRAM main
  VAR
    result : LINT;
    a : LINT;
    b : LINT;
  END_VAR
  a := 3000000000;
  b := 5000000000;
  IF a < b THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64(), 1);
}

#[test]
fn end_to_end_when_lint_less_equal_then_correct() {
    let source = "
PROGRAM main
  VAR
    result : LINT;
    a : LINT;
    b : LINT;
  END_VAR
  a := 5000000000;
  b := 5000000000;
  IF a <= b THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64(), 1);
}

#[test]
fn end_to_end_when_lint_greater_equal_then_correct() {
    let source = "
PROGRAM main
  VAR
    result : LINT;
    a : LINT;
    b : LINT;
  END_VAR
  a := 5000000000;
  b := 5000000000;
  IF a >= b THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64(), 1);
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
fn end_to_end_when_usint_zero_extend_then_preserves_high_value() {
    let source = "
PROGRAM main
  VAR
    x : USINT;
    y : USINT;
  END_VAR
  x := 200;
  y := x + 10;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 200 truncated to u8 zero-extends back to 200 in i32; 200 + 10 = 210
    assert_eq!(bufs.vars[0].as_i32(), 200);
    assert_eq!(bufs.vars[1].as_i32(), 210);
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
fn end_to_end_when_uint_zero_extend_then_preserves_high_value() {
    let source = "
PROGRAM main
  VAR
    x : UINT;
    y : UINT;
  END_VAR
  x := 50000;
  y := x + 1000;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 50000 truncated to u16 zero-extends back to 50000 in i32; 50000 + 1000 = 51000
    assert_eq!(bufs.vars[0].as_i32(), 50000);
    assert_eq!(bufs.vars[1].as_i32(), 51000);
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

#[test]
fn end_to_end_when_udint_division_then_unsigned() {
    let source = "
PROGRAM main
  VAR
    x : UDINT;
    y : UDINT;
  END_VAR
  x := 3000000000;
  y := x / 2;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 3B / 2 = 1.5B as unsigned (would be wrong if signed: 3B as i32 is negative)
    assert_eq!(bufs.vars[1].as_i32() as u32, 1_500_000_000);
}

#[test]
fn end_to_end_when_udint_modulo_then_unsigned() {
    let source = "
PROGRAM main
  VAR
    x : UDINT;
    y : UDINT;
  END_VAR
  x := 3000000001;
  y := x MOD 3000000000;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 3B+1 MOD 3B = 1 as unsigned (would be wrong if signed)
    assert_eq!(bufs.vars[1].as_i32() as u32, 1);
}

#[test]
fn end_to_end_when_udint_less_than_then_unsigned() {
    let source = "
PROGRAM main
  VAR
    result : UDINT;
    a : UDINT;
    b : UDINT;
  END_VAR
  a := 2000000000;
  b := 3000000000;
  IF a < b THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 2B < 3B is true unsigned (3B as i32 is negative, so signed LT would say false)
    assert_eq!(bufs.vars[0].as_i32(), 1);
}

#[test]
fn end_to_end_when_udint_less_equal_then_unsigned() {
    let source = "
PROGRAM main
  VAR
    result : UDINT;
    a : UDINT;
    b : UDINT;
  END_VAR
  a := 3000000000;
  b := 3000000000;
  IF a <= b THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i32(), 1);
}

#[test]
fn end_to_end_when_udint_greater_equal_then_unsigned() {
    let source = "
PROGRAM main
  VAR
    result : UDINT;
    a : UDINT;
    b : UDINT;
  END_VAR
  a := 3000000000;
  b := 2000000000;
  IF a >= b THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    // 3B >= 2B is true unsigned (3B as i32 is negative, so signed GE would say false)
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
fn end_to_end_when_ulint_subtraction_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : ULINT;
    y : ULINT;
  END_VAR
  x := 5000000000;
  y := x - 1;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i64(), 4_999_999_999);
}

#[test]
fn end_to_end_when_ulint_multiplication_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : ULINT;
    y : ULINT;
  END_VAR
  x := 1000000;
  y := x * 1000000;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i64(), 1_000_000_000_000);
}

#[test]
fn end_to_end_when_ulint_division_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : ULINT;
    y : ULINT;
  END_VAR
  x := 10000000000;
  y := x / 2;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i64(), 5_000_000_000);
}

#[test]
fn end_to_end_when_ulint_modulo_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : ULINT;
    y : ULINT;
  END_VAR
  x := 10000000001;
  y := x MOD 10000000000;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[1].as_i64(), 1);
}

#[test]
fn end_to_end_when_ulint_comparison_then_correct() {
    let source = "
PROGRAM main
  VAR
    result : ULINT;
    a : ULINT;
    b : ULINT;
  END_VAR
  a := 5000000000;
  b := 3000000000;
  IF a > b THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64(), 1);
}

#[test]
fn end_to_end_when_ulint_less_than_then_correct() {
    let source = "
PROGRAM main
  VAR
    result : ULINT;
    a : ULINT;
    b : ULINT;
  END_VAR
  a := 3000000000;
  b := 5000000000;
  IF a < b THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64(), 1);
}

#[test]
fn end_to_end_when_ulint_less_equal_then_correct() {
    let source = "
PROGRAM main
  VAR
    result : ULINT;
    a : ULINT;
    b : ULINT;
  END_VAR
  a := 5000000000;
  b := 5000000000;
  IF a <= b THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64(), 1);
}

#[test]
fn end_to_end_when_ulint_greater_equal_then_correct() {
    let source = "
PROGRAM main
  VAR
    result : ULINT;
    a : ULINT;
    b : ULINT;
  END_VAR
  a := 5000000000;
  b := 5000000000;
  IF a >= b THEN
    result := 1;
  ELSE
    result := 0;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);
    assert_eq!(bufs.vars[0].as_i64(), 1);
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
