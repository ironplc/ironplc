//! End-to-end integration tests for the REPLACE standard function.

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
fn end_to_end_when_replace_middle_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello World';
    s2 : STRING := 'Earth';
    result : STRING;
  END_VAR
  result := REPLACE(s1, s2, 5, 7);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // s1 at offset 0 (max 254), s2 at offset 258 (4+254), result at offset 516 (4+254+4+254)
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hello Earth");
}

#[test]
fn end_to_end_when_replace_insert_only_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    s2 : STRING := 'XY';
    result : STRING;
  END_VAR
  result := REPLACE(s1, s2, 0, 3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // L=0 means no deletion, just insert XY at position 3.
    // Result: AB + XY + CDE = ABXYCDE
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "ABXYCDE");
}

#[test]
fn end_to_end_when_replace_at_start_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello';
    s2 : STRING := 'Hi';
    result : STRING;
  END_VAR
  result := REPLACE(s1, s2, 5, 1);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Delete 5 chars from position 1 (all of 'Hello'), insert 'Hi'.
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hi");
}

#[test]
fn end_to_end_when_replace_delete_only_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    s2 : STRING;
    result : STRING;
  END_VAR
  result := REPLACE(s1, s2, 2, 2);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Delete 2 chars at position 2 ('BC'), insert empty string.
    // Result: A + DE = ADE
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "ADE");
}

#[test]
fn end_to_end_when_replace_at_end_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    s2 : STRING := 'XYZ';
    result : STRING;
  END_VAR
  result := REPLACE(s1, s2, 2, 4);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Delete 2 chars at position 4 ('DE'), insert 'XYZ'.
    // Result: ABC + XYZ = ABCXYZ
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "ABCXYZ");
}

#[test]
fn end_to_end_when_replace_with_integer_vars_then_correct_result() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello World';
    s2 : STRING := 'Beautiful ';
    n_len : INT := 0;
    n_pos : INT := 7;
    result : STRING;
  END_VAR
  result := REPLACE(s1, s2, n_len, n_pos);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Insert 'Beautiful ' at position 7, delete 0 chars.
    // Result: Hello Beautiful World
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(
        read_string(&bufs.data_region, result_offset),
        "Hello Beautiful World"
    );
}

#[test]
fn end_to_end_when_replace_result_truncated_by_short_destination_then_truncates() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    s2 : STRING := 'XXXXX';
    result : STRING[6];
  END_VAR
  result := REPLACE(s1, s2, 1, 3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Full result would be AB + XXXXX + DE = ABXXXXXDE (9 chars).
    // But result is STRING[6], so it truncates to 'ABXXXX' (6 chars).
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "ABXXXX");
}

#[test]
fn end_to_end_when_replace_result_fits_exactly_in_destination_then_no_truncation() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    s2 : STRING := 'XY';
    result : STRING[6];
  END_VAR
  result := REPLACE(s1, s2, 1, 3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Result: AB + XY + DE = ABXYDE (6 chars) — fits exactly in STRING[6].
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "ABXYDE");
}

#[test]
fn end_to_end_when_replace_result_exceeds_destination_by_one_then_truncates() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'ABCDE';
    s2 : STRING := 'XYZ';
    result : STRING[6];
  END_VAR
  result := REPLACE(s1, s2, 1, 3);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Result: AB + XYZ + DE = ABXYZDE (7 chars) — truncated to 'ABXYZD' (6 chars).
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "ABXYZD");
}

#[test]
fn end_to_end_when_replace_into_very_short_destination_then_heavily_truncated() {
    let source = "
PROGRAM main
  VAR
    s1 : STRING := 'Hello World';
    s2 : STRING := 'Earth';
    result : STRING[3];
  END_VAR
  result := REPLACE(s1, s2, 5, 7);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());

    // Full result would be 'Hello Earth' (11 chars).
    // STRING[3] truncates to 'Hel'.
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hel");
}
