//! Examples demonstrating bit-access paths that were previously not
//! implemented by PR #916 (partial-access bit syntax). Each test exercises a
//! specific NotImplemented branch that needs to be filled in:
//!
//!   1. Bit write on an LWORD/LINT array element (W64 array-element path).
//!   2. Bit access on an array that is a field of a struct (the
//!      "non-trivial array base" branch).
//!   3. Bit read/write on a struct field.
//!
//! These tests fail before implementation and pass after.

#[macro_use]
mod common;
use common::{parse_and_run, try_parse_and_compile};
use ironplc_parser::options::CompilerOptions;

fn opts_with_partial_access() -> CompilerOptions {
    CompilerOptions {
        allow_partial_access_syntax: true,
        ..CompilerOptions::default()
    }
}

// --- 1. Bit write on an LWORD/LINT array element. Before the fix, this produced a
//        NotImplemented diagnostic in compile_bit_access_assignment_on_array
//        because element_vti.op_width == OpWidth::W64. ---

// x = arr[1]; arr[1] bit 40 = 2^40 = 1099511627776.
e2e_i64!(
    end_to_end_when_write_bit_on_lword_array_element_then_correct,
    "PROGRAM main VAR arr : ARRAY[0..1] OF LWORD; x : LWORD; END_VAR arr[0].0 := TRUE; arr[1].40 := TRUE; x := arr[1]; END_PROGRAM",
    &[(1, 1_099_511_627_776)],
);

// bit 32 = 2^32 = 4294967296.
e2e_i64!(
    end_to_end_when_write_bit_on_lint_array_element_then_correct,
    "PROGRAM main VAR arr : ARRAY[0..1] OF LINT; x : LINT; END_VAR arr[0] := 0; arr[0].32 := TRUE; x := arr[0]; END_PROGRAM",
    &[(1, 4_294_967_296)],
);

// 0xFF00FF00FF00FF00 | 0x1 = 0xFF00FF00FF00FF01.
e2e_i64!(
    end_to_end_when_write_bit_on_lword_array_preserves_other_bits,
    "PROGRAM main VAR arr : ARRAY[0..0] OF LWORD; x : LWORD; END_VAR arr[0] := LWORD#16#FF00_FF00_FF00_FF00; arr[0].0 := TRUE; x := arr[0]; END_PROGRAM",
    &[(1, 0xFF00_FF00_FF00_FF01_u64 as i64)],
);

// --- 2. Bit access on a struct field (W8/W16/W32 struct fields). Before the fix,
//        compile_bit_access_assignment_on_array / compile_variable_read
//        could not resolve the field. ---

// 0x05 = 0b00000101: bit 0 = 1, bit 2 = 1.
e2e_i32!(
    end_to_end_when_read_bit_on_struct_field_then_correct,
    "TYPE MY_STRUCT : STRUCT flags : BYTE; END_STRUCT; END_TYPE PROGRAM main VAR s : MY_STRUCT; r0 : BOOL; r2 : BOOL; END_VAR s.flags := BYTE#16#05; r0 := s.flags.0; r2 := s.flags.2; END_PROGRAM",
    &[(1, 1), (2, 1)],
);

// 0xAA | 0x01 = 0xAB = 171.
e2e_i32!(
    end_to_end_when_write_bit_on_struct_field_then_correct,
    "TYPE MY_STRUCT : STRUCT flags : BYTE; END_STRUCT; END_TYPE PROGRAM main VAR s : MY_STRUCT; x : BYTE; END_VAR s.flags := BYTE#16#AA; s.flags.0 := TRUE; x := s.flags; END_PROGRAM",
    &[(1, 171)],
);

// s.a: 0xFF & 0xFE = 0xFE = 254. s.b: 0x00 | 0x80 = 0x80 = 128.
e2e_i32!(
    end_to_end_when_write_bit_on_struct_field_preserves_other_bits,
    "TYPE MY_STRUCT : STRUCT a : BYTE; b : BYTE; END_STRUCT; END_TYPE PROGRAM main VAR s : MY_STRUCT; va : BYTE; vb : BYTE; END_VAR s.a := BYTE#16#FF; s.b := BYTE#16#00; s.a.0 := FALSE; s.b.7 := TRUE; va := s.a; vb := s.b; END_PROGRAM",
    &[(1, 254), (2, 128)],
);

e2e_i32!(
    end_to_end_when_read_bit_on_word_struct_field_then_correct,
    "TYPE MY_STRUCT : STRUCT flags : WORD; END_STRUCT; END_TYPE PROGRAM main VAR s : MY_STRUCT; r : BOOL; END_VAR s.flags := WORD#16#8000; r := s.flags.15; END_PROGRAM",
    &[(1, 1)],
);

// Set bit 16 of 0 = 65536.
e2e_i32!(
    end_to_end_when_write_bit_on_dint_struct_field_then_correct,
    "TYPE MY_STRUCT : STRUCT value : DINT; END_STRUCT; END_TYPE PROGRAM main VAR s : MY_STRUCT; x : DINT; END_VAR s.value := 0; s.value.16 := TRUE; x := s.value; END_PROGRAM",
    &[(1, 65536)],
);

// --- 3. %Xn syntax on struct field and on LWORD array element. Gated on the
//        partial-access dialect flag. ---

e2e_i32_with!(
    end_to_end_when_percent_x_on_struct_field_read_then_correct,
    opts_with_partial_access(),
    "TYPE MY_STRUCT : STRUCT flags : BYTE; END_STRUCT; END_TYPE PROGRAM main VAR s : MY_STRUCT; r0 : BOOL; r1 : BOOL; END_VAR s.flags := BYTE#16#05; r0 := s.flags.%X0; r1 := s.flags.%X1; END_PROGRAM",
    &[(1, 1), (2, 0)],
);

e2e_i32_with!(
    end_to_end_when_percent_x_on_struct_field_write_then_correct,
    opts_with_partial_access(),
    "TYPE MY_STRUCT : STRUCT flags : BYTE; END_STRUCT; END_TYPE PROGRAM main VAR s : MY_STRUCT; x : BYTE; END_VAR s.flags := BYTE#16#00; s.flags.%X3 := TRUE; x := s.flags; END_PROGRAM",
    &[(1, 8)],
);

// LWORD partial-access: uses i64 assertion, so custom body.
#[test]
fn end_to_end_when_percent_x_on_lword_array_write_then_correct() {
    let source = "PROGRAM main VAR arr : ARRAY[0..0] OF LWORD; x : LWORD; END_VAR arr[0] := LWORD#0; arr[0].%X40 := TRUE; x := arr[0]; END_PROGRAM";
    let (_c, bufs) = parse_and_run(source, &opts_with_partial_access());
    assert_eq!(bufs.vars[1].as_i64(), 1_099_511_627_776);
}

// --- Sanity check: previously-implemented paths still work. ---

// bit 16 = 65536.
e2e_i32!(
    end_to_end_when_write_bit_on_dint_array_then_correct,
    "PROGRAM main VAR arr : ARRAY[0..1] OF DINT; x : DINT; END_VAR arr[0] := 0; arr[0].16 := TRUE; x := arr[0]; END_PROGRAM",
    &[(1, 65536)],
);

// --- Compilation-only sanity tests (previously errored before the fix). ---

#[test]
fn end_to_end_when_write_bit_on_lword_array_then_compiles() {
    let source =
        "PROGRAM main VAR arr : ARRAY[0..0] OF LWORD; END_VAR arr[0].0 := TRUE; END_PROGRAM";
    let result = try_parse_and_compile(source, &CompilerOptions::default());
    assert!(
        result.is_ok(),
        "expected compile to succeed, got error: {:?}",
        result.err()
    );
}

#[test]
fn end_to_end_when_write_bit_on_struct_field_then_compiles() {
    let source = "TYPE MY_STRUCT : STRUCT flags : BYTE; END_STRUCT; END_TYPE PROGRAM main VAR s : MY_STRUCT; END_VAR s.flags.0 := TRUE; END_PROGRAM";
    let result = try_parse_and_compile(source, &CompilerOptions::default());
    assert!(
        result.is_ok(),
        "expected compile to succeed, got error: {:?}",
        result.err()
    );
}

// --- 4. Bit access on an array that is a field of a struct. ---

// 0x05 = 0b00000101: bit 0 = 1, bit 2 = 1.
e2e_i32!(
    end_to_end_when_read_bit_on_struct_field_array_element_then_correct,
    "TYPE MY_STRUCT : STRUCT flags : ARRAY[0..1] OF BYTE; END_STRUCT; END_TYPE PROGRAM main VAR s : MY_STRUCT; r0 : BOOL; r2 : BOOL; END_VAR s.flags[0] := BYTE#16#05; r0 := s.flags[0].0; r2 := s.flags[0].2; END_PROGRAM",
    &[(1, 1), (2, 1)],
);

// 0xAA | 0x01 = 0xAB = 171.
e2e_i32!(
    end_to_end_when_write_bit_on_struct_field_array_element_then_correct,
    "TYPE MY_STRUCT : STRUCT flags : ARRAY[0..1] OF BYTE; END_STRUCT; END_TYPE PROGRAM main VAR s : MY_STRUCT; x : BYTE; END_VAR s.flags[0] := BYTE#16#AA; s.flags[0].0 := TRUE; x := s.flags[0]; END_PROGRAM",
    &[(1, 171)],
);

// s.flags[0]: 0xFF & 0xFE = 0xFE = 254. s.flags[1]: 0x00 | 0x80 = 0x80 = 128.
e2e_i32!(
    end_to_end_when_write_bit_on_struct_field_array_preserves_other_elements,
    "TYPE MY_STRUCT : STRUCT flags : ARRAY[0..1] OF BYTE; END_STRUCT; END_TYPE PROGRAM main VAR s : MY_STRUCT; v0 : BYTE; v1 : BYTE; END_VAR s.flags[0] := BYTE#16#FF; s.flags[1] := BYTE#16#00; s.flags[0].0 := FALSE; s.flags[1].7 := TRUE; v0 := s.flags[0]; v1 := s.flags[1]; END_PROGRAM",
    &[(1, 254), (2, 128)],
);

// bit 40 = 2^40 = 1099511627776.
e2e_i64!(
    end_to_end_when_write_bit_on_struct_field_lword_array_then_correct,
    "TYPE MY_STRUCT : STRUCT vals : ARRAY[0..1] OF LWORD; END_STRUCT; END_TYPE PROGRAM main VAR s : MY_STRUCT; x : LWORD; END_VAR s.vals[0] := LWORD#0; s.vals[0].40 := TRUE; x := s.vals[0]; END_PROGRAM",
    &[(1, 1_099_511_627_776)],
);

e2e_i32_with!(
    end_to_end_when_percent_x_on_struct_field_array_then_correct,
    opts_with_partial_access(),
    "TYPE MY_STRUCT : STRUCT flags : ARRAY[0..0] OF BYTE; END_STRUCT; END_TYPE PROGRAM main VAR s : MY_STRUCT; x : BYTE; END_VAR s.flags[0] := BYTE#16#00; s.flags[0].%X3 := TRUE; x := s.flags[0]; END_PROGRAM",
    &[(1, 8)],
);
