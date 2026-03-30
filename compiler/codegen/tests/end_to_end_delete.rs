//! End-to-end integration tests for the DELETE standard function.

mod common;
use ironplc_container::VarIndex;
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
fn end_to_end_when_delete_middle_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello World';
    result : STRING;
  END_VAR
  result := DELETE(s1, 6, 1);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Delete 6 chars starting at position 1: remove 'Hello ' -> 'World'
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "World");
}

#[test]
fn end_to_end_when_delete_at_end_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello World';
    result : STRING;
  END_VAR
  result := DELETE(s1, 6, 6);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Delete 6 chars starting at position 6: remove ' World' -> 'Hello'
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hello");
}

#[test]
fn end_to_end_when_delete_all_then_empty_string() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    result : STRING;
  END_VAR
  result := DELETE(s1, 5, 1);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Delete all 5 chars starting at position 1.
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "");
}

#[test]
fn end_to_end_when_delete_zero_length_then_unchanged() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    result : STRING;
  END_VAR
  result := DELETE(s1, 0, 3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Delete 0 chars: nothing changes.
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "ABCDE");
}

#[test]
fn end_to_end_when_delete_exceeds_length_then_deletes_to_end() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    result : STRING;
  END_VAR
  result := DELETE(s1, 100, 3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Delete 100 chars starting at position 3, but only 3 chars remain: remove 'CDE' -> 'AB'
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "AB");
}

#[test]
fn end_to_end_when_delete_with_integer_vars_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello Beautiful World';
    n_len : INT := 10;
    n_pos : INT := 6;
    result : STRING;
  END_VAR
  result := DELETE(s1, n_len, n_pos);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Delete 10 chars starting at position 6: remove 'Beautiful ' -> 'Hello World'
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hello World");
}

#[test]
fn end_to_end_when_delete_single_char_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    result : STRING;
  END_VAR
  result := DELETE(s1, 1, 3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Delete 1 char at position 3: remove 'C' -> 'ABDE'
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "ABDE");
}
