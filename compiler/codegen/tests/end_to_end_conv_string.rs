//! End-to-end tests for numeric ↔ STRING type conversions.

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

// <TYPE>_TO_STRING: declare a typed input, a STRING destination, run the conversion,
// and verify the STRING bytes. The STRING is the only one declared, so it sits at
// data_region offset 0.
#[rstest]
#[case::int_positive("INT", "42", "INT", "42")]
#[case::int_negative("INT", "-123", "INT", "-123")]
#[case::int_zero("INT", "0", "INT", "0")]
#[case::dint_large("DINT", "2147483647", "DINT", "2147483647")]
#[case::dint_negative("DINT", "-100", "DINT", "-100")]
#[case::sint_negative("SINT", "-7", "SINT", "-7")]
#[case::usint("USINT", "255", "USINT", "255")]
#[case::uint("UINT", "65535", "UINT", "65535")]
#[case::udint_large("UDINT", "4294967295", "UDINT", "4294967295")]
#[case::dword("DWORD", "255", "DWORD", "255")]
#[case::word("WORD", "1000", "WORD", "1000")]
#[case::byte("BYTE", "42", "BYTE", "42")]
#[case::real_positive("REAL", "3.5", "REAL", "3.5")]
#[case::real_negative("REAL", "-0.5", "REAL", "-0.5")]
// Rust formats 100.0_f32 as "100", not "100.0".
#[case::real_integer_value_no_trailing_dot("REAL", "100.0", "REAL", "100")]
fn end_to_end_num_to_string(
    #[case] ty: &str,
    #[case] init: &str,
    #[case] fn_prefix: &str,
    #[case] expected: &str,
) {
    let source = format!(
        "PROGRAM main VAR x : {ty} := {init}; s : STRING; END_VAR s := {fn_prefix}_TO_STRING(x); END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(read_string(&bufs.data_region, 0), expected);
}

// STRING_TO_<TYPE> into an integer target. `x` is var index 1 (the STRING occupies
// the data_region; `s` is the first scalar slot, at var index 0 but unused here).
#[rstest]
#[case::int_valid("INT", "123", 123)]
#[case::int_invalid_zero("INT", "abc", 0)]
#[case::dint_large("DINT", "2147483647", 2_147_483_647)]
fn end_to_end_string_to_int(#[case] ty: &str, #[case] s_init: &str, #[case] expected: i32) {
    let source = format!(
        "PROGRAM main VAR s : STRING := '{s_init}'; x : {ty}; END_VAR x := STRING_TO_{ty}(s); END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), expected);
}

// INT is 16-bit signed; validate via truncated comparison.
#[test]
fn string_to_int_when_negative_then_parsed() {
    let source = "PROGRAM main VAR s : STRING := '-456'; x : INT; END_VAR x := STRING_TO_INT(s); END_PROGRAM";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32() as i16, -456);
}

// STRING_TO_REAL: float value via approximate comparison; invalid → exact 0.0.
#[rstest]
#[case::valid("2.5", 2.5, true)]
#[case::invalid_zero("xyz", 0.0, false)]
fn end_to_end_string_to_real(#[case] s_init: &str, #[case] expected: f32, #[case] approx: bool) {
    let source = format!(
        "PROGRAM main VAR s : STRING := '{s_init}'; x : REAL; END_VAR x := STRING_TO_REAL(s); END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    let actual = bufs.vars[1].as_f32();
    if approx {
        assert!(
            (actual - expected).abs() < 1e-5,
            "expected ≈{expected}, got {actual}"
        );
    } else {
        assert_eq!(actual, expected);
    }
}
