//! End-to-end integration tests for bit string types (BYTE, WORD, DWORD, LWORD).

#[macro_use]
mod common;

use common::parse_and_run;
use ironplc_parser::options::CompilerOptions;

// --- BYTE (8-bit unsigned, 0..255) ---

e2e_i32!(
    end_to_end_when_byte_assignment_then_correct,
    "PROGRAM main VAR x : BYTE; END_VAR x := 200; END_PROGRAM",
    &[(0, 200)],
);

// 256 truncated to u8 wraps to 0.
e2e_i32!(
    end_to_end_when_byte_overflow_then_wraps,
    "PROGRAM main VAR x : BYTE; END_VAR x := 255 + 1; END_PROGRAM",
    &[(0, 0)],
);

// 200 + 100 = 300, truncated to u8 = 44.
e2e_i32!(
    end_to_end_when_byte_arithmetic_then_truncates,
    "PROGRAM main VAR x : BYTE; y : BYTE; END_VAR x := 200; y := x + 100; END_PROGRAM",
    &[(0, 200), (1, 44)],
);

// --- WORD (16-bit unsigned, 0..65535) ---

e2e_i32!(
    end_to_end_when_word_assignment_then_correct,
    "PROGRAM main VAR x : WORD; END_VAR x := 50000; END_PROGRAM",
    &[(0, 50000)],
);

// 65536 truncated to u16 wraps to 0.
e2e_i32!(
    end_to_end_when_word_overflow_then_wraps,
    "PROGRAM main VAR x : WORD; END_VAR x := 65535 + 1; END_PROGRAM",
    &[(0, 0)],
);

// --- DWORD (32-bit unsigned, 0..4294967295) ---

e2e_i32!(
    end_to_end_when_dword_assignment_then_correct,
    "PROGRAM main VAR x : DWORD; END_VAR x := 1000; END_PROGRAM",
    &[(0, 1000)],
);

e2e_i32!(
    end_to_end_when_dword_comparison_then_correct,
    "PROGRAM main VAR x : DWORD; result : DWORD; END_VAR x := 100; IF x > 50 THEN result := 1; ELSE result := 0; END_IF; END_PROGRAM",
    &[(1, 1)],
);

// --- LWORD (64-bit unsigned, 0..2^64-1) ---

e2e_i64!(
    end_to_end_when_lword_assignment_then_correct,
    "PROGRAM main VAR x : LWORD; END_VAR x := 100000; END_PROGRAM",
    &[(0, 100000)],
);

e2e_i64!(
    end_to_end_when_lword_comparison_then_correct,
    "PROGRAM main VAR x : LWORD; y : LWORD; END_VAR x := 500; IF x > 100 THEN y := 1; ELSE y := 0; END_IF; END_PROGRAM",
    &[(1, 1)],
);

// --- Initial values ---

e2e_i32!(
    end_to_end_when_byte_initial_value_then_variable_initialized,
    "PROGRAM main VAR x : BYTE := 200; END_VAR END_PROGRAM",
    &[(0, 200)],
);

e2e_i32!(
    end_to_end_when_word_initial_value_then_variable_initialized,
    "PROGRAM main VAR x : WORD := 50000; END_VAR END_PROGRAM",
    &[(0, 50000)],
);

e2e_i32!(
    end_to_end_when_dword_initial_value_then_variable_initialized,
    "PROGRAM main VAR x : DWORD := 100000; END_VAR END_PROGRAM",
    &[(0, 100000)],
);

e2e_i64!(
    end_to_end_when_lword_initial_value_then_variable_initialized,
    "PROGRAM main VAR x : LWORD := 5000000; END_VAR END_PROGRAM",
    &[(0, 5000000)],
);

// --- Bit string literals with base prefixes ---

e2e_i32!(
    end_to_end_when_hex_bit_string_literal_then_correct,
    "PROGRAM main VAR x : DWORD; END_VAR x := DWORD#16#FF; END_PROGRAM",
    &[(0, 255)],
);

e2e_i32!(
    end_to_end_when_binary_bit_string_literal_then_correct,
    "PROGRAM main VAR x : BYTE; END_VAR x := BYTE#2#11111111; END_PROGRAM",
    &[(0, 255)],
);

e2e_i32!(
    end_to_end_when_octal_bit_string_literal_then_correct,
    "PROGRAM main VAR x : WORD; END_VAR x := WORD#8#377; END_PROGRAM",
    &[(0, 255)],
);

// --- Bitwise AND ---

e2e_i32!(
    end_to_end_when_byte_and_then_bitwise,
    "PROGRAM main VAR x : BYTE; y : BYTE; END_VAR x := BYTE#16#FF; y := x AND BYTE#16#0F; END_PROGRAM",
    &[(0, 0xFF), (1, 0x0F)],
);

// --- Bitwise OR ---

e2e_i32!(
    end_to_end_when_byte_or_then_bitwise,
    "PROGRAM main VAR x : BYTE; y : BYTE; END_VAR x := BYTE#16#F0; y := x OR BYTE#16#0F; END_PROGRAM",
    &[(0, 0xF0), (1, 0xFF)],
);

// --- Bitwise XOR ---

e2e_i32!(
    end_to_end_when_byte_xor_then_bitwise,
    "PROGRAM main VAR x : BYTE; y : BYTE; END_VAR x := BYTE#16#FF; y := x XOR BYTE#16#0F; END_PROGRAM",
    &[(0, 0xFF), (1, 0xF0)],
);

// --- Bitwise NOT ---

// NOT BYTE#16#0F should be BYTE#16#F0 (= 240), not 0xFFFFFFF0.
e2e_i32!(
    end_to_end_when_byte_not_then_truncated,
    "PROGRAM main VAR x : BYTE; y : BYTE; END_VAR x := BYTE#16#0F; y := NOT x; END_PROGRAM",
    &[(0, 0x0F), (1, 0xF0)],
);

// --- DWORD bitwise ops (full 32-bit) ---

#[test]
fn end_to_end_when_dword_and_then_bitwise() {
    // 0xFFFF0000 AND 0xFF00FF00 = 0xFF000000 (exceeds i32::MAX, so reinterpret as u32).
    let source = "PROGRAM main VAR x : DWORD; y : DWORD; END_VAR x := DWORD#16#FFFF0000; y := x AND DWORD#16#FF00FF00; END_PROGRAM";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32() as u32, 0xFF00_0000);
}

// NOT 0 = 0xFFFFFFFF (as i32: -1).
e2e_i32!(
    end_to_end_when_dword_not_then_bitwise,
    "PROGRAM main VAR x : DWORD; y : DWORD; END_VAR x := 0; y := NOT x; END_PROGRAM",
    &[(1, -1)],
);

// --- LWORD bitwise ops (64-bit) ---

e2e_i64!(
    end_to_end_when_lword_and_then_bitwise,
    "PROGRAM main VAR x : LWORD; y : LWORD; END_VAR x := LWORD#16#FF; y := x AND LWORD#16#0F; END_PROGRAM",
    &[(1, 0x0F)],
);

// NOT 0_i64 = -1_i64.
e2e_i64!(
    end_to_end_when_lword_not_then_bitwise,
    "PROGRAM main VAR x : LWORD; y : LWORD; END_VAR x := 0; y := NOT x; END_PROGRAM",
    &[(1, -1)],
);

// --- NOT in IF condition (inline truncation correctness) ---

// NOT 0xFF -> BIT_NOT -> 0xFFFFFF00 -> TRUNC_U8 -> 0x00 -> IF sees 0 -> skip body.
e2e_i32!(
    end_to_end_when_byte_not_in_if_then_correct,
    "PROGRAM main VAR x : BYTE; result : BYTE; END_VAR x := BYTE#16#FF; result := 0; IF NOT x THEN result := 1; END_IF; END_PROGRAM",
    &[(1, 0)],
);

// NOT 0x00 -> BIT_NOT -> 0xFFFFFFFF -> TRUNC_U8 -> 0xFF -> IF sees non-zero -> enter body.
e2e_i32!(
    end_to_end_when_byte_not_zero_in_if_then_enters_body,
    "PROGRAM main VAR x : BYTE; result : BYTE; END_VAR x := 0; result := 0; IF NOT x THEN result := 1; END_IF; END_PROGRAM",
    &[(1, 1)],
);

// --- WORD NOT with truncation ---

// NOT 0xFF00 at 32-bit = 0xFFFF00FF, truncated to u16 = 0x00FF.
e2e_i32!(
    end_to_end_when_word_not_then_truncated,
    "PROGRAM main VAR x : WORD; y : WORD; END_VAR x := WORD#16#FF00; y := NOT x; END_PROGRAM",
    &[(1, 0x00FF)],
);

// --- DWORD large unsigned values (above i32::MAX) ---

// 4294967292 as u32 stored as i32 bit pattern is -4.
e2e_i32!(
    end_to_end_when_dword_initial_value_near_max_then_correct,
    "PROGRAM main VAR x : DWORD := 4294967292; END_VAR END_PROGRAM",
    &[(0, -4)],
);

e2e_i32!(
    end_to_end_when_dword_large_literal_eq_right_then_correct,
    "PROGRAM main VAR mask : DWORD; b : BOOL; END_VAR mask := 4294967292; b := mask = 4294967292; END_PROGRAM",
    &[(1, 1)],
);

e2e_i32!(
    end_to_end_when_dword_large_literal_eq_left_then_correct,
    "PROGRAM main VAR mask : DWORD; b : BOOL; END_VAR mask := 4294967292; b := 4294967292 = mask; END_PROGRAM",
    &[(1, 1)],
);

// 0xFFFFFFFC = 4294967292, stored as i32 bit pattern = -4.
e2e_i32!(
    end_to_end_when_dword_hex_literal_near_max_then_correct,
    "PROGRAM main VAR x : DWORD; END_VAR x := DWORD#16#FFFFFFFC; END_PROGRAM",
    &[(0, -4)],
);

// 0xFFFFFFFC = 4294967292, returned from a FUNCTION call, stored as i32 bit pattern = -4.
e2e_i32!(
    end_to_end_when_dword_hex_literal_in_function_then_correct,
    "FUNCTION MY_FUNC : DWORD VAR mask : DWORD; END_VAR mask := DWORD#16#FFFFFFFC; MY_FUNC := mask; END_FUNCTION PROGRAM main VAR r : DWORD; END_VAR r := MY_FUNC(); END_PROGRAM",
    &[(0, -4)],
);

e2e_i32!(
    end_to_end_when_dword_hex_literal_eq_then_correct,
    "PROGRAM main VAR mask : DWORD; b : BOOL; END_VAR mask := DWORD#16#FFFFFFFC; b := mask = DWORD#16#FFFFFFFC; END_PROGRAM",
    &[(1, 1)],
);

// mask AND 16#FFFF_FFFC = 4294967292 AND 4294967292 = 4294967292 = last.
e2e_i32!(
    end_to_end_when_dword_and_large_hex_literal_in_comparison_then_correct,
    "PROGRAM main VAR mask : DWORD; last : DWORD; b : BOOL; END_VAR mask := 4294967292; last := mask AND 16#FFFF_FFFC; IF (mask AND 16#FFFF_FFFC) = last THEN b := TRUE; END_IF; END_PROGRAM",
    &[(2, 1)],
);

e2e_i32!(
    end_to_end_when_dword_or_large_hex_literal_in_comparison_then_correct,
    "PROGRAM main VAR mask : DWORD; last : DWORD; b : BOOL; END_VAR mask := 4294967292; last := mask OR 16#FFFF_FFFC; IF (mask OR 16#FFFF_FFFC) = last THEN b := TRUE; END_IF; END_PROGRAM",
    &[(2, 1)],
);
