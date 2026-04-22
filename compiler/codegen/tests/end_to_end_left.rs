//! End-to-end integration tests for the LEFT standard function.

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

// All cases share: VAR s1 : STRING := <init>; result : STRING; END_VAR;
//                  result := LEFT(s1, <n>);
// `result` sits after one default-sized STRING.
#[rstest]
#[case::partial("'Hello World'", "5", "Hello")]
#[case::exceeds_length_clamped("'Hi'", "100", "Hi")]
#[case::zero_gives_empty("'Hello'", "0", "")]
#[case::single_char("'ABCDE'", "1", "A")]
#[case::exact_length("'ABCDE'", "5", "ABCDE")]
#[case::empty_string_gives_empty("", "5", "")]
fn end_to_end_left(#[case] s1_init: &str, #[case] n: &str, #[case] expected: &str) {
    let s1_decl = if s1_init.is_empty() {
        "s1 : STRING;".to_string()
    } else {
        format!("s1 : STRING := {s1_init};")
    };
    let source = format!(
        "PROGRAM main VAR {s1_decl} result : STRING; END_VAR result := LEFT(s1, {n}); END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), expected);
}

// LEFT where the count is passed via an INT variable: adds a scalar slot but
// leaves the preceding STRINGs unchanged.
#[test]
fn end_to_end_when_left_with_integer_var_then_correct_result() {
    let source = "PROGRAM main VAR s1 : STRING := 'Hello World'; n : INT := 3; result : STRING; END_VAR result := LEFT(s1, n); END_PROGRAM";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    let result_offset = string_offset(&[254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "Hel");
}
