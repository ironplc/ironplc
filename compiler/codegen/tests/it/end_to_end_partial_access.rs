//! End-to-end tests for byte/word/dword/lword partial access
//! (`.%Bn`, `.%Wn`, `.%Dn`, `.%Ln`).

use crate::common::{parse_and_run, try_parse_and_compile, VmBuffers};
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;
use spec_test_macro::spec_test;

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
///
/// REQ-PAB-codegen-120: a slice reads as the bit-string type of its width.
/// REQ-PAB-codegen-130: slice `n` is bits `n * width ..= (n + 1) * width - 1`.
/// REQ-PAB-codegen-131: a write replaces only the addressed slice.
/// REQ-PAB-codegen-132: array elements and structure fields behave the same.
#[spec_test(REQ_PAB_codegen_120)]
#[spec_test(REQ_PAB_codegen_130)]
#[spec_test(REQ_PAB_codegen_131)]
#[spec_test(REQ_PAB_codegen_132)]
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
#[case::write_word_0_of_dint(
    "",
    "n : DINT",
    "DINT",
    "n := 16#12345678; n.%W0 := WORD#16#BEEF; r := n;",
    0x1234BEEF
)]
// A slice as wide as its base replaces the whole value.
#[case::write_dword_0_of_dword(
    "",
    "d : DWORD",
    "DWORD",
    "d := DWORD#16#AABBCCDD; d.%D0 := DWORD#16#11223344; r := d;",
    0x11223344
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

/// Slice reads and writes whose result is a 64-bit value.
///
/// REQ-PAB-codegen-130 and REQ-PAB-codegen-131 as above, on an `LWORD` base.
/// REQ-PAB-codegen-133: the value written is compiled at the slice's width.
#[spec_test(REQ_PAB_codegen_133)]
#[rstest]
#[case::lword_0_of_lword(
    "l : LWORD",
    "l := LWORD#16#0102030405060708; r := l.%L0;",
    0x0102030405060708
)]
#[case::write_word_1_of_lword(
    "l : LWORD",
    "l := LWORD#16#0000000000000000; l.%W1 := WORD#16#ABCD; r := l;",
    0x00000000ABCD0000
)]
#[case::write_dword_1_of_lword(
    "l : LWORD",
    "l := LWORD#16#0000000011111111; l.%D1 := DWORD#16#AABBCCDD; r := l;",
    0xAABBCCDD11111111
)]
// A 32-bit literal with its top bit set is a bit pattern, not a negative
// number.
#[case::write_dword_0_of_lword(
    "l : LWORD",
    "l := LWORD#16#AABBCCDD11223344; l.%D0 := DWORD#16#FFFFFFFF; r := l;",
    0xAABBCCDDFFFFFFFF
)]
#[case::write_dword_1_of_lword_from_variable(
    "l : LWORD; d : DWORD",
    "d := DWORD#16#AABBCCDD; l := LWORD#16#0000000011111111; l.%D1 := d; r := l;",
    0xAABBCCDD11111111
)]
// A 64-bit slice takes a 64-bit right-hand side, whether a literal or a
// variable; a 32-bit path would keep only the low half.
#[case::write_lword_0_of_lword(
    "l : LWORD",
    "l.%L0 := LWORD#16#0102030405060708; r := l;",
    0x0102030405060708
)]
#[case::write_lword_0_of_lword_from_variable(
    "l : LWORD; l2 : LWORD",
    "l2 := LWORD#16#0102030405060708; l.%L0 := l2; r := l;",
    0x0102030405060708
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
