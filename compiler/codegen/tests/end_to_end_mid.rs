//! End-to-end integration tests for the MID standard function.

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
fn end_to_end_when_mid_beginning_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello World';
    result : STRING;
  END_VAR
  result := MID(s1, 5, 1);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // MID 5 chars starting at position 1 -> 'Hello'
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hello");
}

#[test]
fn end_to_end_when_mid_end_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello World';
    result : STRING;
  END_VAR
  result := MID(s1, 5, 7);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // MID 5 chars starting at position 7 -> 'World'
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "World");
}

#[test]
fn end_to_end_when_mid_middle_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDEFGH';
    result : STRING;
  END_VAR
  result := MID(s1, 3, 3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // MID 3 chars starting at position 3 -> 'CDE'
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "CDE");
}

#[test]
fn end_to_end_when_mid_exceeds_length_then_clamps() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    result : STRING;
  END_VAR
  result := MID(s1, 100, 3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // MID 100 chars from position 3, but only 3 remain -> 'CDE'
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "CDE");
}

#[test]
fn end_to_end_when_mid_zero_length_then_empty() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    result : STRING;
  END_VAR
  result := MID(s1, 0, 2);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "");
}

#[test]
fn end_to_end_when_mid_single_char_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    result : STRING;
  END_VAR
  result := MID(s1, 1, 3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "C");
}

#[test]
fn end_to_end_when_mid_with_integer_vars_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello Beautiful World';
    n_len : INT := 9;
    n_pos : INT := 7;
    result : STRING;
  END_VAR
  result := MID(s1, n_len, n_pos);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // MID 9 chars starting at position 7 -> 'Beautiful'
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Beautiful");
}

#[test]
fn end_to_end_when_mid_position_beyond_end_then_empty() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABC';
    result : STRING;
  END_VAR
  result := MID(s1, 5, 10);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Position 10 is beyond end of 3-char string -> empty
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "");
}
