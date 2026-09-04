//! End-to-end tests for byte/word/dword/lword partial access
//! (`.%Bn`, `.%Wn`, `.%Dn`, `.%Ln`).

use crate::common::{parse_and_run, try_parse_and_compile, VmBuffers};
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

fn opts() -> CompilerOptions {
    CompilerOptions {
        allow_partial_access_syntax: true,
        ..CompilerOptions::default()
    }
}

/// A structure with one `DWORD` field, for the struct-field cases.
const DWORD_STRUCT: &str = "
TYPE MY_STRUCT : STRUCT
    value : DWORD;
END_STRUCT;
END_TYPE
";

/// Runs a program that declares `r : {result_type}` and then `decls`,
/// executes `body`, and leaves the result for the caller in `r` (variable
/// slot 0).
fn run_partial_access(prelude: &str, decls: &str, result_type: &str, body: &str) -> VmBuffers {
    let source = format!(
        "
{prelude}
PROGRAM main
  VAR
    r : {result_type};
    {decls};
  END_VAR
  {body}
END_PROGRAM
"
    );
    let (_c, bufs) = parse_and_run(&source, &opts());
    bufs
}

/// Slice reads and writes whose result fits a 32-bit slot.
#[rstest]
// --- reads ---
#[case::byte_0_of_dword("", "d : DWORD", "BYTE", "d := DWORD#16#AABBCCDD; r := d.%B0;", 0xDD)]
#[case::byte_3_of_dword("", "d : DWORD", "BYTE", "d := DWORD#16#AABBCCDD; r := d.%B3;", 0xAA)]
#[case::byte_7_of_lword(
    "",
    "l : LWORD",
    "BYTE",
    "l := LWORD#16#0102030405060708; r := l.%B7;",
    0x01
)]
#[case::word_0_of_dword("", "d : DWORD", "WORD", "d := DWORD#16#AABBCCDD; r := d.%W0;", 0xCCDD)]
#[case::word_1_of_dword("", "d : DWORD", "WORD", "d := DWORD#16#AABBCCDD; r := d.%W1;", 0xAABB)]
#[case::word_2_of_lword(
    "",
    "l : LWORD",
    "WORD",
    "l := LWORD#16#0102030405060708; r := l.%W2;",
    0x0304
)]
#[case::dword_1_of_lword(
    "",
    "l : LWORD",
    "DWORD",
    "l := LWORD#16#AABBCCDD11223344; r := l.%D1;",
    0xAABBCCDD
)]
#[case::byte_2_of_array_element(
    "",
    "arr : ARRAY[0..1] OF DWORD",
    "BYTE",
    "arr[0] := DWORD#16#AABBCCDD; r := arr[0].%B2;",
    0xBB
)]
#[case::byte_1_of_struct_field(
    DWORD_STRUCT,
    "s : MY_STRUCT",
    "BYTE",
    "s.value := DWORD#16#12345678; r := s.value.%B1;",
    0x56
)]
// --- writes: only the addressed slice changes ---
#[case::write_byte_1_of_dword(
    "",
    "d : DWORD",
    "DWORD",
    "d := DWORD#16#AABBCCDD; d.%B1 := BYTE#16#FF; r := d;",
    0xAABBFFDD
)]
#[case::write_byte_0_of_dword(
    "",
    "d : DWORD",
    "DWORD",
    "d := DWORD#16#AABB0000; d.%B0 := BYTE#16#42; r := d;",
    0xAABB0042
)]
#[case::write_byte_3_of_array_element(
    "",
    "arr : ARRAY[0..0] OF DWORD",
    "DWORD",
    "arr[0] := DWORD#16#00000000; arr[0].%B3 := BYTE#16#FF; r := arr[0];",
    0xFF000000
)]
#[case::write_byte_2_of_struct_field(
    DWORD_STRUCT,
    "s : MY_STRUCT",
    "DWORD",
    "s.value := DWORD#16#12345678; s.value.%B2 := BYTE#16#FF; r := s.value;",
    0x12FF5678
)]
fn partial_access_when_narrow_result_then_expected(
    #[case] prelude: &str,
    #[case] decls: &str,
    #[case] result_type: &str,
    #[case] body: &str,
    #[case] expected: u32,
) {
    let bufs = run_partial_access(prelude, decls, result_type, body);
    assert_eq!(bufs.vars[0].as_i32() as u32, expected);
}

/// Slice writes whose result is a 64-bit value.
#[rstest]
#[case::write_word_1_of_lword(
    "l : LWORD",
    "l := LWORD#16#0000000000000000; l.%W1 := WORD#16#ABCD; r := l;",
    0x00000000ABCD0000
)]
fn partial_access_when_wide_result_then_expected(
    #[case] decls: &str,
    #[case] body: &str,
    #[case] expected: u64,
) {
    let bufs = run_partial_access("", decls, "LWORD", body);
    assert_eq!(bufs.vars[0].as_i64() as u64, expected);
}

// --- Compilation gating ---

const BYTE_SLICE_PROGRAM: &str = "
PROGRAM main
  VAR
    d : DWORD;
    r : BYTE;
  END_VAR
  r := d.%B0;
END_PROGRAM
";

#[test]
fn end_to_end_when_partial_access_byte_flag_off_then_parse_fails() {
    let result = ironplc_parser::parse_program(
        BYTE_SLICE_PROGRAM,
        &ironplc_dsl::core::FileId::default(),
        &CompilerOptions::default(),
    );
    assert!(result.is_err());
}

#[test]
fn end_to_end_when_partial_access_byte_flag_on_then_compiles() {
    let result = try_parse_and_compile(BYTE_SLICE_PROGRAM, &opts());
    assert!(
        result.is_ok(),
        "expected compile to succeed, got error: {:?}",
        result.err()
    );
}
