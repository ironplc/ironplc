//! End-to-end tests for byte/word/dword/lword partial access
//! (`.%Bn`, `.%Wn`, `.%Dn`, `.%Ln`).

#[macro_use]
mod common;

use common::{parse_and_run, try_parse_and_compile};
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

fn opts() -> CompilerOptions {
    CompilerOptions {
        allow_partial_access_syntax: true,
        ..CompilerOptions::default()
    }
}

// Partial-access *read* from a DWORD/LWORD into a byte/word/dword target.
// Envelope: `VAR d : <ty>; r : <tgt>; END_VAR d := <ty>#16#<lit>; r := d.<access>;`.
// `r` is at vars[1].
#[rstest]
#[case::byte_0_from_dword("DWORD", "AABBCCDD", "BYTE", "%B0", 0xDD)]
#[case::byte_3_from_dword("DWORD", "AABBCCDD", "BYTE", "%B3", 0xAA)]
#[case::byte_from_lword("LWORD", "0102030405060708", "BYTE", "%B7", 0x01)]
#[case::word_0_from_dword("DWORD", "AABBCCDD", "WORD", "%W0", 0xCCDD)]
#[case::word_1_from_dword("DWORD", "AABBCCDD", "WORD", "%W1", 0xAABB)]
#[case::word_from_lword("LWORD", "0102030405060708", "WORD", "%W2", 0x0304)]
#[case::dword_1_from_lword("LWORD", "AABBCCDD11223344", "DWORD", "%D1", 0xAABBCCDDu32 as i32)]
fn end_to_end_partial_read(
    #[case] src_ty: &str,
    #[case] src_lit: &str,
    #[case] tgt_ty: &str,
    #[case] access: &str,
    #[case] expected: i32,
) {
    let var_name = if src_ty == "LWORD" { "l" } else { "d" };
    let source = format!(
        "PROGRAM main VAR {var_name} : {src_ty}; r : {tgt_ty}; END_VAR {var_name} := {src_ty}#16#{src_lit}; r := {var_name}.{access}; END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &opts());
    assert_eq!(bufs.vars[1].as_i32(), expected);
}

// Partial-access *write*: declare a variable, set a literal, overwrite one
// sub-range, read back into `r`. Envelope identical apart from the access /
// width / assign.
#[rstest]
#[case::write_byte_1("AABBCCDD", "%B1", "BYTE#16#FF", 0xAABBFFDDu32 as i32)]
#[case::write_byte_0("AABB0000", "%B0", "BYTE#16#42", 0xAABB0042u32 as i32)]
fn end_to_end_partial_write_dword(
    #[case] init: &str,
    #[case] access: &str,
    #[case] new_val: &str,
    #[case] expected: i32,
) {
    let source = format!(
        "PROGRAM main VAR d : DWORD; r : DWORD; END_VAR d := DWORD#16#{init}; d.{access} := {new_val}; r := d; END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &opts());
    assert_eq!(bufs.vars[1].as_i32(), expected);
}

// Writing into an LWORD target requires i64 assertion; keep as a dedicated test.
#[test]
fn end_to_end_when_write_word_to_lword_then_correct() {
    let source = "PROGRAM main VAR l : LWORD; r : LWORD; END_VAR l := LWORD#16#0000000000000000; l.%W1 := WORD#16#ABCD; r := l; END_PROGRAM";
    let (_c, bufs) = parse_and_run(source, &opts());
    assert_eq!(bufs.vars[1].as_i64(), 0x00000000ABCD0000u64 as i64);
}

// Partial access on an array element: read.
e2e_i32_with!(
    end_to_end_when_read_byte_from_dword_array_then_correct,
    opts(),
    "PROGRAM main VAR arr : ARRAY[0..1] OF DWORD; r : BYTE; END_VAR arr[0] := DWORD#16#AABBCCDD; r := arr[0].%B2; END_PROGRAM",
    &[(1, 0xBB)],
);

// Partial access on an array element: write.
e2e_i32_with!(
    end_to_end_when_write_byte_to_dword_array_then_correct,
    opts(),
    "PROGRAM main VAR arr : ARRAY[0..0] OF DWORD; r : DWORD; END_VAR arr[0] := DWORD#16#00000000; arr[0].%B3 := BYTE#16#FF; r := arr[0]; END_PROGRAM",
    &[(1, 0xFF000000u32 as i32)],
);

// Partial access on a struct field: read.
e2e_i32_with!(
    end_to_end_when_read_byte_from_struct_field_then_correct,
    opts(),
    "TYPE MY_STRUCT : STRUCT value : DWORD; END_STRUCT; END_TYPE PROGRAM main VAR s : MY_STRUCT; r : BYTE; END_VAR s.value := DWORD#16#12345678; r := s.value.%B1; END_PROGRAM",
    &[(1, 0x56)],
);

// Partial access on a struct field: write. 0x12345678 byte-2 replaced → 0x12FF5678.
e2e_i32_with!(
    end_to_end_when_write_byte_to_struct_field_then_correct,
    opts(),
    "TYPE MY_STRUCT : STRUCT value : DWORD; END_STRUCT; END_TYPE PROGRAM main VAR s : MY_STRUCT; r : DWORD; END_VAR s.value := DWORD#16#12345678; s.value.%B2 := BYTE#16#FF; r := s.value; END_PROGRAM",
    &[(1, 0x12FF5678u32 as i32)],
);

// --- Compilation gating ---

#[test]
fn end_to_end_when_partial_access_byte_flag_off_then_parse_fails() {
    let source = "PROGRAM main VAR d : DWORD; r : BYTE; END_VAR r := d.%B0; END_PROGRAM";
    let result = ironplc_parser::parse_program(
        source,
        &ironplc_dsl::core::FileId::default(),
        &CompilerOptions::default(),
    );
    assert!(result.is_err());
}

#[test]
fn end_to_end_when_partial_access_byte_flag_on_then_compiles() {
    let source = "PROGRAM main VAR d : DWORD; r : BYTE; END_VAR r := d.%B0; END_PROGRAM";
    let result = try_parse_and_compile(source, &opts());
    assert!(
        result.is_ok(),
        "expected compile to succeed, got error: {:?}",
        result.err()
    );
}
