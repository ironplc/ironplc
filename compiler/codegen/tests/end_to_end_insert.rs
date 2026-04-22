//! End-to-end integration tests for the INSERT standard function.

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

// All cases share the envelope
//   VAR s1 : STRING := <s1_init>; s2 : STRING := <s2_init>; result : <result_ty>; END_VAR
//   result := INSERT(s1, s2, <pos>);
// The two preceding STRINGs always occupy default 254 bytes, so `result` is at
// `string_offset(&[254, 254])`.
#[rstest]
#[case::in_middle("'HelloWorld'", "' '", "STRING", "5", "Hello World")]
#[case::at_start("'World'", "'Hello '", "STRING", "0", "Hello World")]
#[case::at_end("'Hello'", "' World'", "STRING", "5", "Hello World")]
#[case::empty_s2("'ABCDE'", "", "STRING", "3", "ABCDE")]
#[case::empty_s1("", "'Hello'", "STRING", "0", "Hello")]
#[case::truncates_short_dest("'ABCDE'", "'XXXXX'", "STRING[6]", "2", "ABXXXX")]
fn end_to_end_insert(
    #[case] s1_init: &str,
    #[case] s2_init: &str,
    #[case] result_ty: &str,
    #[case] pos: &str,
    #[case] expected: &str,
) {
    let s1_decl = if s1_init.is_empty() {
        "s1 : STRING;".to_string()
    } else {
        format!("s1 : STRING := {s1_init};")
    };
    let s2_decl = if s2_init.is_empty() {
        "s2 : STRING;".to_string()
    } else {
        format!("s2 : STRING := {s2_init};")
    };
    let source = format!(
        "PROGRAM main VAR {s1_decl} {s2_decl} result : {result_ty}; END_VAR result := INSERT(s1, s2, {pos}); END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), expected);
}

// INSERT where the position comes from an INT variable. The extra INT slot is a
// scalar so it doesn't affect the data_region layout of the preceding strings.
#[test]
fn end_to_end_when_insert_with_integer_var_then_correct_result() {
    let source = "PROGRAM main VAR s1 : STRING := 'ABCDE'; s2 : STRING := 'XY'; n_pos : INT := 2; result : STRING; END_VAR result := INSERT(s1, s2, n_pos); END_PROGRAM";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    let result_offset = string_offset(&[254, 254]);
    assert_eq!(read_string(&bufs.data_region, result_offset), "ABXYCDE");
}
