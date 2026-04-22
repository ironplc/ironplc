//! End-to-end integration tests for the REPLACE standard function.

mod common;

use common::parse_and_run;
use ironplc_container::STRING_HEADER_BYTES;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

fn read_string(data_region: &[u8], data_offset: usize) -> String {
    let cur_len =
        u16::from_le_bytes([data_region[data_offset + 2], data_region[data_offset + 3]]) as usize;
    let data_start = data_offset + STRING_HEADER_BYTES;
    let bytes = &data_region[data_start..data_start + cur_len];
    bytes.iter().map(|&b| b as char).collect()
}

fn string_offset(preceding_max_lengths: &[u16]) -> usize {
    preceding_max_lengths
        .iter()
        .map(|&ml| STRING_HEADER_BYTES + ml as usize)
        .sum()
}

// Every case declares two default-sized STRINGs followed by `result`, so `result`
// always sits at offset `string_offset(&[254, 254])`.
// The varying pieces are the source/replacement values, the result type (to exercise
// truncation), the L/P arguments to REPLACE, and the expected output.
#[rstest]
// Basic replacements: default-sized destination, no truncation.
#[case::middle("'Hello World'", "'Earth'", "STRING", 5, 7, "Hello Earth")]
#[case::insert_only("'ABCDE'", "'XY'", "STRING", 0, 3, "ABXYCDE")]
#[case::at_start("'Hello'", "'Hi'", "STRING", 5, 1, "Hi")]
#[case::delete_only("'ABCDE'", "", "STRING", 2, 2, "ADE")]
#[case::at_end("'ABCDE'", "'XYZ'", "STRING", 2, 4, "ABCXYZ")]
// Truncation via STRING[N] destination.
#[case::truncated_short_dest("'ABCDE'", "'XXXXX'", "STRING[6]", 1, 3, "ABXXXX")]
#[case::fits_exactly("'ABCDE'", "'XY'", "STRING[6]", 1, 3, "ABXYDE")]
#[case::exceeds_by_one("'ABCDE'", "'XYZ'", "STRING[6]", 1, 3, "ABXYZD")]
#[case::very_short_dest("'Hello World'", "'Earth'", "STRING[3]", 5, 7, "Hel")]
fn end_to_end_replace(
    #[case] s1_init: &str,
    #[case] s2_init: &str,
    #[case] result_ty: &str,
    #[case] len: i32,
    #[case] pos: i32,
    #[case] expected: &str,
) {
    let s2_decl = if s2_init.is_empty() {
        "s2 : STRING;".to_string()
    } else {
        format!("s2 : STRING := {s2_init};")
    };
    let source = format!(
        "PROGRAM main VAR s1 : STRING := {s1_init}; {s2_decl} result : {result_ty}; END_VAR result := REPLACE(s1, s2, {len}, {pos}); END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), expected);
}

// REPLACE where the L/P arguments come from INT variables rather than literals.
// Has two extra vars (n_len, n_pos) between the strings and the result, but those
// scalars don't live in the data_region so the offset calc is unchanged.
#[test]
fn end_to_end_when_replace_with_integer_vars_then_correct_result() {
    let source = "PROGRAM main VAR s1 : STRING := 'Hello World'; s2 : STRING := 'Beautiful '; n_len : INT := 0; n_pos : INT := 7; result : STRING; END_VAR result := REPLACE(s1, s2, n_len, n_pos); END_PROGRAM";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(
        read_string(&bufs.data_region, result_offset),
        "Hello Beautiful World"
    );
}
