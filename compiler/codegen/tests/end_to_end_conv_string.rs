//! End-to-end tests for numeric ↔ STRING type conversions.

mod common;
use ironplc_container::STRING_HEADER_BYTES;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;

/// Reads a STRING value from the data region at the given byte offset.
fn read_string(data_region: &[u8], data_offset: usize) -> String {
    let cur_len =
        u16::from_le_bytes([data_region[data_offset + 2], data_region[data_offset + 3]]) as usize;
    let data_start = data_offset + STRING_HEADER_BYTES;
    let bytes = &data_region[data_start..data_start + cur_len];
    bytes.iter().map(|&b| b as char).collect()
}

// =========================================================================
// INT_TO_STRING
// =========================================================================

#[test]
fn int_to_string_when_positive_then_decimal() {
    let source = "
PROGRAM main
  VAR
    x : INT := 42;
    s : STRING;
  END_VAR
  s := INT_TO_STRING(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(read_string(&bufs.data_region, 0), "42");
}

#[test]
fn int_to_string_when_negative_then_negative_decimal() {
    let source = "
PROGRAM main
  VAR
    x : INT := -123;
    s : STRING;
  END_VAR
  s := INT_TO_STRING(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(read_string(&bufs.data_region, 0), "-123");
}

#[test]
fn int_to_string_when_zero_then_zero_string() {
    let source = "
PROGRAM main
  VAR
    x : INT := 0;
    s : STRING;
  END_VAR
  s := INT_TO_STRING(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(read_string(&bufs.data_region, 0), "0");
}

// =========================================================================
// DINT_TO_STRING
// =========================================================================

#[test]
fn dint_to_string_when_large_value_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : DINT := 2147483647;
    s : STRING;
  END_VAR
  s := DINT_TO_STRING(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(read_string(&bufs.data_region, 0), "2147483647");
}

#[test]
fn dint_to_string_when_negative_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : DINT := -100;
    s : STRING;
  END_VAR
  s := DINT_TO_STRING(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(read_string(&bufs.data_region, 0), "-100");
}

// =========================================================================
// SINT_TO_STRING
// =========================================================================

#[test]
fn sint_to_string_when_negative_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : SINT := -7;
    s : STRING;
  END_VAR
  s := SINT_TO_STRING(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(read_string(&bufs.data_region, 0), "-7");
}

// =========================================================================
// USINT_TO_STRING / UINT_TO_STRING / UDINT_TO_STRING
// =========================================================================

#[test]
fn usint_to_string_when_value_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : USINT := 255;
    s : STRING;
  END_VAR
  s := USINT_TO_STRING(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(read_string(&bufs.data_region, 0), "255");
}

#[test]
fn uint_to_string_when_value_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : UINT := 65535;
    s : STRING;
  END_VAR
  s := UINT_TO_STRING(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(read_string(&bufs.data_region, 0), "65535");
}

#[test]
fn udint_to_string_when_large_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : UDINT := 4294967295;
    s : STRING;
  END_VAR
  s := UDINT_TO_STRING(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(read_string(&bufs.data_region, 0), "4294967295");
}

// =========================================================================
// DWORD_TO_STRING / WORD_TO_STRING / BYTE_TO_STRING
// =========================================================================

#[test]
fn dword_to_string_when_value_then_unsigned_decimal() {
    let source = "
PROGRAM main
  VAR
    x : DWORD := 255;
    s : STRING;
  END_VAR
  s := DWORD_TO_STRING(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(read_string(&bufs.data_region, 0), "255");
}

#[test]
fn word_to_string_when_value_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : WORD := 1000;
    s : STRING;
  END_VAR
  s := WORD_TO_STRING(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(read_string(&bufs.data_region, 0), "1000");
}

#[test]
fn byte_to_string_when_value_then_correct() {
    let source = "
PROGRAM main
  VAR
    x : BYTE := 42;
    s : STRING;
  END_VAR
  s := BYTE_TO_STRING(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(read_string(&bufs.data_region, 0), "42");
}

// =========================================================================
// REAL_TO_STRING
// =========================================================================

#[test]
fn real_to_string_when_positive_then_decimal() {
    let source = "
PROGRAM main
  VAR
    x : REAL := 3.5;
    s : STRING;
  END_VAR
  s := REAL_TO_STRING(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(read_string(&bufs.data_region, 0), "3.5");
}

#[test]
fn real_to_string_when_negative_then_negative_decimal() {
    let source = "
PROGRAM main
  VAR
    x : REAL := -0.5;
    s : STRING;
  END_VAR
  s := REAL_TO_STRING(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(read_string(&bufs.data_region, 0), "-0.5");
}

#[test]
fn real_to_string_when_integer_value_then_no_trailing_dot() {
    let source = "
PROGRAM main
  VAR
    x : REAL := 100.0;
    s : STRING;
  END_VAR
  s := REAL_TO_STRING(x);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // Rust formats 100.0_f32 as "100", not "100.0"
    assert_eq!(read_string(&bufs.data_region, 0), "100");
}

// =========================================================================
// STRING_TO_INT
// =========================================================================

#[test]
fn string_to_int_when_valid_then_parsed() {
    let source = "
PROGRAM main
  VAR
    s : STRING := '123';
    x : INT;
  END_VAR
  x := STRING_TO_INT(s);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 123);
}

#[test]
fn string_to_int_when_negative_then_parsed() {
    let source = "
PROGRAM main
  VAR
    s : STRING := '-456';
    x : INT;
  END_VAR
  x := STRING_TO_INT(s);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    // INT is 16-bit signed, so -456 fits.
    assert_eq!(bufs.vars[1].as_i32() as i16, -456);
}

#[test]
fn string_to_int_when_invalid_then_zero() {
    let source = "
PROGRAM main
  VAR
    s : STRING := 'abc';
    x : INT;
  END_VAR
  x := STRING_TO_INT(s);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

// =========================================================================
// STRING_TO_DINT
// =========================================================================

#[test]
fn string_to_dint_when_large_then_correct() {
    let source = "
PROGRAM main
  VAR
    s : STRING := '2147483647';
    x : DINT;
  END_VAR
  x := STRING_TO_DINT(s);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 2147483647);
}

// =========================================================================
// STRING_TO_REAL
// =========================================================================

#[test]
fn string_to_real_when_valid_then_parsed() {
    let source = "
PROGRAM main
  VAR
    s : STRING := '2.5';
    x : REAL;
  END_VAR
  x := STRING_TO_REAL(s);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert!((bufs.vars[1].as_f32() - 2.5).abs() < 1e-5);
}

#[test]
fn string_to_real_when_invalid_then_zero() {
    let source = "
PROGRAM main
  VAR
    s : STRING := 'xyz';
    x : REAL;
  END_VAR
  x := STRING_TO_REAL(s);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_f32(), 0.0);
}
