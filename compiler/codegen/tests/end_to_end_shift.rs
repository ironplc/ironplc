//! End-to-end integration tests for bit shift/rotate functions (SHL, SHR, ROL, ROR).

#[macro_use]
mod common;

use common::parse_and_run;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

// SHL / SHR / ROL / ROR on BYTE, WORD, DWORD: share the envelope
//   VAR x : <ty>; y : <ty>; x := <ty>#<hex>; y := <fn>(x, <n>);
// and all assert via `as_i32`.
#[rstest]
#[case::shl_byte("BYTE", 0x0F, "SHL", 4, 0xF0)]
#[case::shl_dword("DWORD", 0x0F, "SHL", 16, 0x000F_0000_u32 as i32)]
#[case::shr_word("WORD", 0xFF00, "SHR", 8, 0x00FF)]
#[case::shr_byte("BYTE", 0xF0, "SHR", 4, 0x0F)]
#[case::rol_byte("BYTE", 0x81, "ROL", 1, 0x03)]
#[case::rol_word("WORD", 0x8001, "ROL", 1, 0x0003)]
#[case::rol_dword("DWORD", 0x8000_0001_u32 as i32, "ROL", 1, 0x0000_0003)]
#[case::ror_dword("DWORD", 0x0000_0001, "ROR", 1, 0x8000_0000_u32 as i32)]
#[case::ror_byte("BYTE", 0x01, "ROR", 1, 0x80)]
#[case::shl_byte_overflow_truncates("BYTE", 0xFF, "SHL", 4, 0xF0)]
fn end_to_end_shift_rotate(
    #[case] ty: &str,
    #[case] x: i32,
    #[case] op: &str,
    #[case] n: u32,
    #[case] expected_y: i32,
) {
    let source = format!(
        "PROGRAM main VAR x : {ty}; y : {ty}; END_VAR x := {ty}#16#{x:X}; y := {op}(x, {n}); END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), x);
    assert_eq!(bufs.vars[1].as_i32(), expected_y);
}

// LWORD (64-bit) shift has a wider assertion type; keep separate.
e2e_i64!(
    end_to_end_when_shl_lword_then_shifts_left_64bit,
    "PROGRAM main VAR x : LWORD; y : LWORD; END_VAR x := LWORD#16#01; y := SHL(x, 32); END_PROGRAM",
    &[(0, 0x01), (1, 0x1_0000_0000)],
);

// Zero-shift asserts `y == x`, not `y == <constant>`, so it needs a custom body.
#[test]
fn end_to_end_when_shl_with_zero_shift_then_unchanged() {
    let source = "PROGRAM main VAR x : DWORD; y : DWORD; END_VAR x := DWORD#16#DEADBEEF; y := SHL(x, 0); END_PROGRAM";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), bufs.vars[0].as_i32());
}

// Nested function call (SHR of ABS) - distinct enough from the shift-rotate matrix to keep separate.
e2e_i32!(
    end_to_end_when_shr_with_abs_then_computes_correctly,
    "PROGRAM main VAR a : DINT; result : DINT; END_VAR a := -8; result := SHR(ABS(a), 1); END_PROGRAM",
    &[(0, -8), (1, 4)],
);
