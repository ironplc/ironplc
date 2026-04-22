//! End-to-end integration tests for STRING initial values.

mod common;
use common::parse_and_run;
use ironplc_container::STRING_HEADER_BYTES;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

/// Data region layout per ADR-0015:
///   [max_length: u16 LE][cur_length: u16 LE][data: cur_length bytes]
/// STRING uses Latin-1 (ADR-0016), which maps 1:1 to Unicode for 0x00-0xFF.
fn read_string(data_region: &[u8], data_offset: usize) -> String {
    let cur_len =
        u16::from_le_bytes([data_region[data_offset + 2], data_region[data_offset + 3]]) as usize;
    let data_start = data_offset + STRING_HEADER_BYTES;
    let bytes = &data_region[data_start..data_start + cur_len];
    bytes.iter().map(|&b| b as char).collect()
}

fn read_max_length(data_region: &[u8], data_offset: usize) -> u16 {
    u16::from_le_bytes([data_region[data_offset], data_region[data_offset + 1]])
}

// Initial-value / default tests: declare one STRING and check (contents,
// max_length) at data_region offset 0.
#[rstest]
#[case::initial_value("x : STRING := 'hello';", "hello", 254)]
#[case::no_initial_value_empty("x : STRING;", "", 254)]
#[case::with_length_set("x : STRING[10] := 'hi';", "hi", 10)]
#[case::empty_literal("x : STRING := '';", "", 254)]
fn end_to_end_string_initial(
    #[case] decl: &str,
    #[case] expected_contents: &str,
    #[case] expected_max: u16,
) {
    let source = format!("PROGRAM main VAR {decl} END_VAR END_PROGRAM");
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(read_string(&bufs.data_region, 0), expected_contents);
    assert_eq!(read_max_length(&bufs.data_region, 0), expected_max);
}

// Two STRING variables: second one lives at offset STRING_HEADER_BYTES + 254.
#[test]
fn end_to_end_when_two_string_variables_then_both_initialized() {
    let source = "PROGRAM main VAR a : STRING := 'foo'; b : STRING := 'bar'; END_VAR END_PROGRAM";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(read_string(&bufs.data_region, 0), "foo");
    assert_eq!(read_string(&bufs.data_region, STRING_HEADER_BYTES + 254), "bar");
}

// DINT + STRING mixed: scalar at vars[0], string at data_region 0.
#[test]
fn end_to_end_when_string_and_int_then_both_work() {
    let source = "PROGRAM main VAR x : DINT := 42; s : STRING := 'test'; END_VAR END_PROGRAM";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), 42);
    assert_eq!(read_string(&bufs.data_region, 0), "test");
}

// STRING-returning FUNCTION: both `STRING[255]` and `STRING[80]` shapes.
#[rstest]
#[case::returns_literal(
    "FUNCTION my_func : STRING[255] VAR_INPUT x : INT; END_VAR my_func := 'hello'; END_FUNCTION PROGRAM main VAR result : STRING; END_VAR result := my_func(1); END_PROGRAM",
    "hello"
)]
#[case::returns_input(
    "FUNCTION MY_FUNC : STRING[80] VAR_INPUT str : STRING[80]; END_VAR MY_FUNC := str; END_FUNCTION PROGRAM main VAR result : STRING[80]; END_VAR result := MY_FUNC(str := 'Hello'); END_PROGRAM",
    "Hello"
)]
fn end_to_end_string_returning_function(#[case] source: &str, #[case] expected: &str) {
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(read_string(&bufs.data_region, 0), expected);
}
