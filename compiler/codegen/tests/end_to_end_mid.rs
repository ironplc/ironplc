//! End-to-end integration tests for the MID standard function.

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

// Envelope: VAR s1 : STRING := <init>; result : STRING; END_VAR;
//           result := MID(s1, <len>, <pos>);
#[rstest]
#[case::beginning("'Hello World'", 5, 1, "Hello")]
#[case::end("'Hello World'", 5, 7, "World")]
#[case::middle("'ABCDEFGH'", 3, 3, "CDE")]
#[case::exceeds_length_clamps("'ABCDE'", 100, 3, "CDE")]
#[case::zero_length_empty("'ABCDE'", 0, 2, "")]
#[case::single_char("'ABCDE'", 1, 3, "C")]
#[case::position_beyond_end_empty("'ABC'", 5, 10, "")]
fn end_to_end_mid(
    #[case] s1_init: &str,
    #[case] len: i32,
    #[case] pos: i32,
    #[case] expected: &str,
) {
    let source = format!(
        "PROGRAM main VAR s1 : STRING := {s1_init}; result : STRING; END_VAR result := MID(s1, {len}, {pos}); END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), expected);
}

#[test]
fn end_to_end_when_mid_with_integer_vars_then_correct_result() {
    let source = "PROGRAM main VAR s1 : STRING := 'Hello Beautiful World'; n_len : INT := 9; n_pos : INT := 7; result : STRING; END_VAR result := MID(s1, n_len, n_pos); END_PROGRAM";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Beautiful");
}
