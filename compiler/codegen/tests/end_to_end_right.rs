//! End-to-end integration tests for the RIGHT standard function.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;
use ironplc_container::STRING_HEADER_BYTES;

/// Reads a STRING value from the data region at the given byte offset.
fn read_string(data_region: &[u8], data_offset: usize) -> String {
    let cur_len =
        u16::from_le_bytes([data_region[data_offset + 2], data_region[data_offset + 3]]) as usize;
    let data_start = data_offset + STRING_HEADER_BYTES;
    let bytes = &data_region[data_start..data_start + cur_len];
    bytes.iter().map(|&b| b as char).collect()
}

/// Computes the data_offset of a STRING variable given its position
/// in the declaration order and preceding string max lengths.
/// Each STRING variable occupies STRING_HEADER_BYTES + max_length bytes.
fn string_offset(preceding_max_lengths: &[u16]) -> usize {
    preceding_max_lengths
        .iter()
        .map(|&ml| STRING_HEADER_BYTES + ml as usize)
        .sum()
}

#[test]
fn end_to_end_when_right_partial_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello World';
    result : STRING;
  END_VAR
  result := RIGHT(s1, 5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // RIGHT 5 chars of 'Hello World' -> 'World'
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "World");
}

#[test]
fn end_to_end_when_right_exceeds_length_then_entire_string() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hi';
    result : STRING;
  END_VAR
  result := RIGHT(s1, 100);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // RIGHT 100 chars of 'Hi' -> 'Hi' (clamped to string length)
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hi");
}

#[test]
fn end_to_end_when_right_zero_then_empty_string() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello';
    result : STRING;
  END_VAR
  result := RIGHT(s1, 0);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "");
}

#[test]
fn end_to_end_when_right_single_char_then_last_char() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    result : STRING;
  END_VAR
  result := RIGHT(s1, 1);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "E");
}

#[test]
fn end_to_end_when_right_exact_length_then_entire_string() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    result : STRING;
  END_VAR
  result := RIGHT(s1, 5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "ABCDE");
}

#[test]
fn end_to_end_when_right_with_integer_var_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello World';
    n : INT := 3;
    result : STRING;
  END_VAR
  result := RIGHT(s1, n);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "rld");
}

#[test]
fn end_to_end_when_right_empty_string_then_empty() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING;
    result : STRING;
  END_VAR
  result := RIGHT(s1, 5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "");
}
