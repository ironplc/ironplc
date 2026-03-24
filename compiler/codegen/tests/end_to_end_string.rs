//! End-to-end integration tests for STRING initial values.

mod common;

use common::{parse_and_compile, parse_and_run};
use ironplc_container::STRING_HEADER_BYTES;

/// Reads a STRING value from the data region at the given byte offset.
///
/// Data region layout per ADR-0015:
///   [max_length: u16 LE][cur_length: u16 LE][data: cur_length bytes]
fn read_string(data_region: &[u8], data_offset: usize) -> String {
    let cur_len =
        u16::from_le_bytes([data_region[data_offset + 2], data_region[data_offset + 3]]) as usize;
    let data_start = data_offset + STRING_HEADER_BYTES;
    let bytes = &data_region[data_start..data_start + cur_len];
    // STRING uses Latin-1 encoding (ADR-0016), which maps 1:1 to Unicode for 0x00-0xFF.
    bytes.iter().map(|&b| b as char).collect()
}

/// Reads the max_length field of a STRING in the data region.
fn read_max_length(data_region: &[u8], data_offset: usize) -> u16 {
    u16::from_le_bytes([data_region[data_offset], data_region[data_offset + 1]])
}

#[test]
fn end_to_end_when_string_initial_value_then_variable_initialized() {
    let source = "
PROGRAM main
  VAR
    x : STRING := 'hello';
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let s = read_string(&bufs.data_region, 0);
    assert_eq!(s, "hello");
    // Default STRING max length is 254.
    assert_eq!(read_max_length(&bufs.data_region, 0), 254);
}

#[test]
fn end_to_end_when_string_no_initial_value_then_empty() {
    let source = "
PROGRAM main
  VAR
    x : STRING;
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let s = read_string(&bufs.data_region, 0);
    assert_eq!(s, "");
    assert_eq!(read_max_length(&bufs.data_region, 0), 254);
}

#[test]
fn end_to_end_when_string_with_length_then_max_length_set() {
    let source = "
PROGRAM main
  VAR
    x : STRING[10] := 'hi';
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let s = read_string(&bufs.data_region, 0);
    assert_eq!(s, "hi");
    assert_eq!(read_max_length(&bufs.data_region, 0), 10);
}

#[test]
fn end_to_end_when_two_string_variables_then_both_initialized() {
    let source = "
PROGRAM main
  VAR
    a : STRING := 'foo';
    b : STRING := 'bar';
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // First string at offset 0: [max:254][cur:3][data]
    let s1 = read_string(&bufs.data_region, 0);
    assert_eq!(s1, "foo");

    // Second string at offset 4 + 254 = 258
    let s2 = read_string(&bufs.data_region, 258);
    assert_eq!(s2, "bar");
}

#[test]
fn end_to_end_when_string_and_int_then_both_work() {
    let source = "
PROGRAM main
  VAR
    x : DINT := 42;
    s : STRING := 'test';
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // Integer variable at slot 0.
    assert_eq!(bufs.vars[0].as_i32(), 42);

    // String variable at data region offset 0.
    let s = read_string(&bufs.data_region, 0);
    assert_eq!(s, "test");
}

#[test]
fn end_to_end_when_string_empty_literal_then_cur_length_zero() {
    let source = "
PROGRAM main
  VAR
    x : STRING := '';
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let s = read_string(&bufs.data_region, 0);
    assert_eq!(s, "");
    assert_eq!(read_max_length(&bufs.data_region, 0), 254);
}

#[test]
fn end_to_end_when_function_returns_string_with_length_then_compiles() {
    // Verify that FUNCTION : STRING[255] compiles through the full pipeline
    // (parse -> analyze -> codegen) without errors.
    let source = "
FUNCTION my_func : STRING[255]
  VAR_INPUT
    x : INT;
  END_VAR
  my_func := 'hello';
END_FUNCTION

PROGRAM main
  VAR
    r : BOOL;
  END_VAR
  r := TRUE;
END_PROGRAM
";
    // This previously failed at parse time because STRING[255] was not
    // accepted as a function return type.
    let _container = parse_and_compile(source);
}
