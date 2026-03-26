//! End-to-end integration tests for the INSERT standard function.

mod common;
use ironplc_parser::options::ParseOptions;

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
fn end_to_end_when_insert_in_middle_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'HelloWorld';
    s2 : STRING := ' ';
    result : STRING;
  END_VAR
  result := INSERT(s1, s2, 5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    // Insert ' ' after position 5: Hello + ' ' + World = 'Hello World'
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hello World");
}

#[test]
fn end_to_end_when_insert_at_start_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'World';
    s2 : STRING := 'Hello ';
    result : STRING;
  END_VAR
  result := INSERT(s1, s2, 0);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    // Insert 'Hello ' at position 0 (before everything): 'Hello World'
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hello World");
}

#[test]
fn end_to_end_when_insert_at_end_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello';
    s2 : STRING := ' World';
    result : STRING;
  END_VAR
  result := INSERT(s1, s2, 5);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    // Insert ' World' after position 5 (end of string): 'Hello World'
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hello World");
}

#[test]
fn end_to_end_when_insert_empty_string_then_unchanged() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    s2 : STRING;
    result : STRING;
  END_VAR
  result := INSERT(s1, s2, 3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    // Inserting empty string changes nothing.
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "ABCDE");
}

#[test]
fn end_to_end_when_insert_into_empty_string_then_returns_inserted() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING;
    s2 : STRING := 'Hello';
    result : STRING;
  END_VAR
  result := INSERT(s1, s2, 0);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hello");
}

#[test]
fn end_to_end_when_insert_with_integer_var_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    s2 : STRING := 'XY';
    n_pos : INT := 2;
    result : STRING;
  END_VAR
  result := INSERT(s1, s2, n_pos);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    // Insert 'XY' after position 2: AB + XY + CDE = 'ABXYCDE'
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "ABXYCDE");
}

#[test]
fn end_to_end_when_insert_result_truncated_by_short_destination_then_truncates() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    s2 : STRING := 'XXXXX';
    result : STRING[6];
  END_VAR
  result := INSERT(s1, s2, 2);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &ParseOptions::default());

    // Full result would be AB + XXXXX + CDE = ABXXXXXCDE (10 chars).
    // But result is STRING[6], so it truncates to 'ABXXXX' (6 chars).
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "ABXXXX");
}
