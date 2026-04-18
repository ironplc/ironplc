//! End-to-end integration tests for bit access on integer variables (e.g., `a.0`).

#[macro_use]
mod common;

use ironplc_parser::options::CompilerOptions;

// --- Bit read tests ---

// 5 = 0b101, bit 0 is 1 -> TRUE.
e2e_i32!(
    end_to_end_when_dint_bit_access_0_on_odd_then_true,
    "PROGRAM main VAR a : DINT; result : BOOL; END_VAR a := 5; result := a.0; END_PROGRAM",
    &[(1, 1)],
);

// 4 = 0b100, bit 0 is 0 -> FALSE.
e2e_i32!(
    end_to_end_when_dint_bit_access_0_on_even_then_false,
    "PROGRAM main VAR a : DINT; result : BOOL; END_VAR a := 4; result := a.0; END_PROGRAM",
    &[(1, 0)],
);

// 5 = 0b101, bit 2 is 1 -> TRUE.
e2e_i32!(
    end_to_end_when_dint_bit_access_2_then_correct,
    "PROGRAM main VAR a : DINT; result : BOOL; END_VAR a := 5; result := a.2; END_PROGRAM",
    &[(1, 1)],
);

// 5 = 0b101, bit 1 is 0 -> FALSE.
e2e_i32!(
    end_to_end_when_dint_bit_access_1_then_false,
    "PROGRAM main VAR a : DINT; result : BOOL; END_VAR a := 5; result := a.1; END_PROGRAM",
    &[(1, 0)],
);

// 0x80 = 0b10000000, bit 7 is 1 -> TRUE.
e2e_i32!(
    end_to_end_when_byte_bit_access_7_then_correct,
    "PROGRAM main VAR x : BYTE; result : BOOL; END_VAR x := BYTE#16#80; result := x.7; END_PROGRAM",
    &[(1, 1)],
);

// 0x8000 bit 15 is 1 -> TRUE.
e2e_i32!(
    end_to_end_when_word_bit_access_then_correct,
    "PROGRAM main VAR x : WORD; result : BOOL; END_VAR x := WORD#16#8000; result := x.15; END_PROGRAM",
    &[(1, 1)],
);

// 65536 = 0x10000, bit 16 = 1.
e2e_i32!(
    end_to_end_when_read_bit_of_dword_then_correct,
    "PROGRAM main VAR x : DWORD; y : BOOL; END_VAR x := 65536; y := x.16; END_PROGRAM",
    &[(1, 1)],
);

// 10 = 0b1010, bit 1 = 1.
e2e_i32!(
    end_to_end_when_read_bit_of_int_then_correct,
    "PROGRAM main VAR x : INT; y : BOOL; END_VAR x := 10; y := x.1; END_PROGRAM",
    &[(1, 1)],
);

// 5 = 0b101, bit 0 is 1 -> TRUE, so FOO returns 1.
e2e_i32!(
    end_to_end_when_function_with_dint_bit_access_then_correct,
    "FUNCTION FOO : INT VAR_INPUT A : DINT; END_VAR IF A.0 THEN FOO := 1; END_IF; END_FUNCTION PROGRAM main VAR result : INT; END_VAR result := FOO(A := 5); END_PROGRAM",
    &[(0, 1)],
);

// --- Bit write tests ---

// Set bit 0: 0 -> 1.
e2e_i32!(
    end_to_end_when_write_bit_0_set_then_correct,
    "PROGRAM main VAR x : BYTE; END_VAR x := 0; x.0 := TRUE; END_PROGRAM",
    &[(0, 1)],
);

// Set bit 3: 0 -> 8.
e2e_i32!(
    end_to_end_when_write_bit_3_set_then_correct,
    "PROGRAM main VAR x : BYTE; END_VAR x := 0; x.3 := TRUE; END_PROGRAM",
    &[(0, 8)],
);

// Clear bit 0: 255 -> 254.
e2e_i32!(
    end_to_end_when_write_bit_0_clear_then_correct,
    "PROGRAM main VAR x : BYTE; END_VAR x := 255; x.0 := FALSE; END_PROGRAM",
    &[(0, 254)],
);

// Clear bit 7: 255 -> 127.
e2e_i32!(
    end_to_end_when_write_bit_7_clear_then_correct,
    "PROGRAM main VAR x : BYTE; END_VAR x := 255; x.7 := FALSE; END_PROGRAM",
    &[(0, 127)],
);

// 170 = 0b10101010, set bit 0 -> 0b10101011 = 171.
e2e_i32!(
    end_to_end_when_write_bit_preserves_other_bits_then_correct,
    "PROGRAM main VAR x : BYTE; END_VAR x := 170; x.0 := TRUE; END_PROGRAM",
    &[(0, 171)],
);

// Set bit 8: 0 -> 256.
e2e_i32!(
    end_to_end_when_write_bit_of_word_then_correct,
    "PROGRAM main VAR x : WORD; END_VAR x := 0; x.8 := TRUE; END_PROGRAM",
    &[(0, 256)],
);

// Set bit 16: 0 -> 65536.
e2e_i32!(
    end_to_end_when_write_bit_of_dint_then_correct,
    "PROGRAM main VAR x : DINT; END_VAR x := 0; x.16 := TRUE; END_PROGRAM",
    &[(0, 65536)],
);

// --- Multiple bit operations ---

// Set bits 0, 2, 4: 0b00010101 = 21.
e2e_i32!(
    end_to_end_when_set_multiple_bits_then_correct,
    "PROGRAM main VAR x : BYTE; END_VAR x := 0; x.0 := TRUE; x.2 := TRUE; x.4 := TRUE; END_PROGRAM",
    &[(0, 21)],
);

// x = 8 after setting bit 3; y = TRUE after reading it.
e2e_i32!(
    end_to_end_when_read_after_write_then_correct,
    "PROGRAM main VAR x : BYTE; y : BOOL; END_VAR x := 0; x.3 := TRUE; y := x.3; END_PROGRAM",
    &[(0, 8), (1, 1)],
);

// --- IEC 61131-3:2013 partial-access syntax (.%Xn) — see REQ-PAB in
//     specs/design/partial-access-bit-syntax.md.
//     These tests gate on --allow-partial-access-syntax.

fn opts_with_partial_access() -> CompilerOptions {
    CompilerOptions {
        allow_partial_access_syntax: true,
        ..CompilerOptions::default()
    }
}

e2e_i32_with!(
    /// REQ-PAB-040: Reading `x.%Xn` on a BYTE returns the value of bit n.
    /// 0x05 = 0b00000101 — bits 0 and 2 are set.
    codegen_spec_req_pab_040_read_percent_x_on_byte_returns_bit,
    opts_with_partial_access(),
    "PROGRAM main VAR x : BYTE; r0 : BOOL; r2 : BOOL; END_VAR x := BYTE#16#05; r0 := x.%X0; r2 := x.%X2; END_PROGRAM",
    &[(1, 1), (2, 1)],
);

e2e_i32_with!(
    /// REQ-PAB-041: Reading `arr[i].%Xn` returns bit n of element i. This is the
    /// user's exact failing program (from a rusty source). myByteArray[0] =
    /// 0b00000101 — bit 0 = 1, bit 1 = 0, bit 2 = 1.
    codegen_spec_req_pab_041_read_percent_x_on_byte_array_element_returns_bit,
    opts_with_partial_access(),
    "PROGRAM main VAR myByteArray : ARRAY[0..1] OF BYTE := [2#00000101, 2#00000000]; r0 : BOOL; r1 : BOOL; r2 : BOOL; END_VAR r0 := myByteArray[0].%X0; r1 := myByteArray[0].%X1; r2 := myByteArray[0].%X2; END_PROGRAM",
    &[(1, 1), (2, 0), (3, 1)],
);

e2e_i32_with!(
    /// REQ-PAB-042: Writing `arr[i].%Xn := TRUE/FALSE` updates bit n without
    /// altering other bits or other elements. Array contents live in the data
    /// region, so the test copies the updated bytes out to scalar vars for
    /// verification.
    /// arr[0]: 0b10101010 | 0b00000001 = 0b10101011 = 171.
    /// arr[1]: 0b00000000 | 0b10000000 = 0b10000000 = 128.
    codegen_spec_req_pab_042_write_percent_x_on_byte_array_preserves_other_bits,
    opts_with_partial_access(),
    "PROGRAM main VAR arr : ARRAY[0..1] OF BYTE := [2#10101010, 2#00000000]; b0 : BYTE; b1 : BYTE; END_VAR arr[0].%X0 := TRUE; arr[1].%X7 := TRUE; b0 := arr[0]; b1 := arr[1]; END_PROGRAM",
    &[(1, 171), (2, 128)],
);
