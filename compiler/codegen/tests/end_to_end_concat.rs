//! End-to-end integration tests for the CONCAT standard function.

mod common;

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
fn end_to_end_when_concat_two_strings_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello';
    s2 : STRING := ' World';
    result : STRING;
  END_VAR
  result := CONCAT(s1, s2);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hello World");
}

#[test]
fn end_to_end_when_concat_single_chars_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'A';
    s2 : STRING := 'B';
    result : STRING;
  END_VAR
  result := CONCAT(s1, s2);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "AB");
}

#[test]
fn end_to_end_when_concat_empty_first_then_second_string() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING;
    s2 : STRING := 'World';
    result : STRING;
  END_VAR
  result := CONCAT(s1, s2);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "World");
}

#[test]
fn end_to_end_when_concat_empty_second_then_first_string() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello';
    s2 : STRING;
    result : STRING;
  END_VAR
  result := CONCAT(s1, s2);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hello");
}

#[test]
fn end_to_end_when_concat_both_empty_then_empty() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING;
    s2 : STRING;
    result : STRING;
  END_VAR
  result := CONCAT(s1, s2);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "");
}

#[test]
fn end_to_end_when_concat_longer_strings_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'The quick brown ';
    s2 : STRING := 'fox jumps over';
    result : STRING;
  END_VAR
  result := CONCAT(s1, s2);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let result_offset = string_offset(&[254, 254]);
    assert_eq!(
        read_string(&bufs.data_region, result_offset),
        "The quick brown fox jumps over"
    );
}

#[test]
fn end_to_end_when_concat_same_variable_then_doubled() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABC';
    result : STRING;
  END_VAR
  result := CONCAT(s1, s1);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "ABCABC");
}

#[test]
fn end_to_end_when_concat_two_literals_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    result : STRING;
  END_VAR
  result := CONCAT('Hello', ' World');
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    // result is the first (and only) declared string variable at offset 0.
    let result_offset = string_offset(&[]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hello World");
}

#[test]
fn end_to_end_when_concat_literal_and_variable_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'World';
    result : STRING;
  END_VAR
  result := CONCAT('Hello ', s1);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hello World");
}

#[test]
fn end_to_end_when_concat_variable_and_literal_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello';
    result : STRING;
  END_VAR
  result := CONCAT(s1, ' World');
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hello World");
}

#[test]
fn end_to_end_when_concat_single_char_literals_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    result : STRING;
  END_VAR
  result := CONCAT('A', 'B');
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source);

    let result_offset = string_offset(&[]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "AB");
}
