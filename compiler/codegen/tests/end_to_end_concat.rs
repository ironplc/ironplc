//! End-to-end integration tests for the CONCAT standard function.

mod common;

use common::parse_and_run;
use ironplc_container::STRING_HEADER_BYTES;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

/// Reads a STRING value from the data region at the given byte offset.
fn read_string(data_region: &[u8], data_offset: usize) -> String {
    let cur_len =
        u16::from_le_bytes([data_region[data_offset + 2], data_region[data_offset + 3]]) as usize;
    let data_start = data_offset + STRING_HEADER_BYTES;
    let bytes = &data_region[data_start..data_start + cur_len];
    bytes.iter().map(|&b| b as char).collect()
}

/// Byte offset of a STRING variable in the data region, computed from the
/// maximum lengths of the STRING variables that precede it in declaration order.
fn string_offset(preceding_max_lengths: &[u16]) -> usize {
    preceding_max_lengths
        .iter()
        .map(|&ml| STRING_HEADER_BYTES + ml as usize)
        .sum()
}

/// Runs `PROGRAM main VAR {decl} result : STRING; END_VAR result := CONCAT({args}); END_PROGRAM`
/// and asserts `result` equals `expected`. `pre_lens` lists the max lengths of the
/// STRING variables declared before `result` so the helper can locate `result` in
/// the data region.
fn assert_concat(decl: &str, args: &str, pre_lens: &[u16], expected: &str) {
    let source = format!(
        "PROGRAM main VAR {decl} result : STRING; END_VAR result := CONCAT({args}); END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    let result_offset = string_offset(pre_lens);
    assert_eq!(read_string(&bufs.data_region, result_offset), expected);
}

// CONCAT of two STRING variables: two strings precede `result`, each default-sized (254).
#[rstest]
#[case::two_strings(
    "s1 : STRING := 'Hello'; s2 : STRING := ' World';",
    "s1, s2",
    "Hello World"
)]
#[case::single_chars("s1 : STRING := 'A'; s2 : STRING := 'B';", "s1, s2", "AB")]
#[case::empty_first("s1 : STRING; s2 : STRING := 'World';", "s1, s2", "World")]
#[case::empty_second("s1 : STRING := 'Hello'; s2 : STRING;", "s1, s2", "Hello")]
#[case::both_empty("s1 : STRING; s2 : STRING;", "s1, s2", "")]
#[case::longer_strings(
    "s1 : STRING := 'The quick brown '; s2 : STRING := 'fox jumps over';",
    "s1, s2",
    "The quick brown fox jumps over"
)]
fn end_to_end_concat_two_vars(#[case] decl: &str, #[case] args: &str, #[case] expected: &str) {
    assert_concat(decl, args, &[254, 254], expected);
}

// CONCAT with one preceding STRING variable (literal on one side, or var used twice).
#[rstest]
#[case::same_variable("s1 : STRING := 'ABC';", "s1, s1", "ABCABC")]
#[case::literal_and_variable("s1 : STRING := 'World';", "'Hello ', s1", "Hello World")]
#[case::variable_and_literal("s1 : STRING := 'Hello';", "s1, ' World'", "Hello World")]
fn end_to_end_concat_one_var(#[case] decl: &str, #[case] args: &str, #[case] expected: &str) {
    assert_concat(decl, args, &[254], expected);
}

// CONCAT with two literal arguments: no preceding STRING variables.
#[rstest]
#[case::two_literals("'Hello', ' World'", "Hello World")]
#[case::single_char_literals("'A', 'B'", "AB")]
fn end_to_end_concat_literals_only(#[case] args: &str, #[case] expected: &str) {
    assert_concat("", args, &[], expected);
}
