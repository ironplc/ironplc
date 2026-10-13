//! Tests of lowering calls of standard functions, and the bit and partial
//! reads that use them.
//!
//! Each test that checks a requirement of `specs/design/lowered-program.md`
//! names it.

use crate::test_support::lower;

/// REQ-LOW-lowering-147: a shift or rotate is the intrinsic for its function
/// at the type of the value shifted, a rotate of a `BYTE` or `WORD` keeping
/// its own width.
#[test]
fn lower_statement_when_shift_or_rotate_then_intrinsic_call() {
    let outcome = lower(
        "PROGRAM main VAR b : BYTE; w : WORD; d : DWORD; l : LWORD; n : DINT; END_VAR
            d := SHL(d, n);
            l := SHR(l, 3);
            b := ROL(b, 1);
            w := ROR(w, n);
            l := ROL(l, 2);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f2 (call shl_u32 i32u (read i32u v0^.f2) (read i32s v0^.f4))",
            "assign v0^.f3 (call shr_u64 i64u (read i64u v0^.f3) (const i64s 3))",
            "assign v0^.f0 (truncate 8 (call rol_u8 i32u (read i32u v0^.f0) (const i32s 1)))",
            "assign v0^.f1 (truncate 16 (call ror_u16 i32u (read i32u v0^.f1) (read i32s v0^.f4)))",
            "assign v0^.f3 (call rol_u64 i64u (read i64u v0^.f3) (const i64s 2))",
        ]
    );
}

/// REQ-LOW-lowering-100: a bit or partial read is the same shift and mask
/// over a read of its base, whatever the base's shape; a bit is compared
/// with zero.
#[test]
fn lower_statement_when_bit_or_partial_read_then_shift_and_mask() {
    let outcome = lower(
        "PROGRAM main VAR w : WORD; a : ARRAY[0..3] OF WORD; i : DINT; x : BOOL; b : BYTE;
             n : INT; l : LWORD; END_VAR
            x := w.3;
            x := a[i].3;
            b := w.%B1;
            x := n.15;
            b := l.%B7;
            x := w.0;
         END_PROGRAM",
        "main",
    );
    let bit = |read: &str, shift: u8| {
        format!(
            "assign v0^.f3 (ne (bit_and i32u (call shr_u32 i32u {read} (const i32s {shift})) \
             (const i32u 1)) (const i32u 0))"
        )
    };
    assert_eq!(
        outcome.statements,
        [
            bit("(read i32u v0^.f0)", 3),
            bit("(read i32u v0^.f1[(read i32s v0^.f2)])", 3),
            "assign v0^.f4 (truncate 8 (bit_and i32u (call shr_u32 i32u (read i32u v0^.f0) \
             (const i32s 8)) (const i32u 255)))"
                .to_string(),
            bit("(convert i32u (read i32s v0^.f5))", 15),
            "assign v0^.f4 (truncate 8 (convert i32u (bit_and i64u (call shr_u64 i64u \
             (read i64u v0^.f6) (const i64s 56)) (const i64u 255))))"
                .to_string(),
            "assign v0^.f3 (ne (bit_and i32u (read i32u v0^.f0) (const i32u 1)) (const i32u 0))"
                .to_string(),
        ]
    );
}

/// The bits of a bit selection, `d.%B1.3`, are selected from the inner
/// selection's value.
#[test]
fn lower_statement_when_bits_of_bits_read_then_selected_twice() {
    let outcome = lower(
        "PROGRAM main VAR d : DWORD; x : BOOL; END_VAR
            x := d.%B1.3;
         END_PROGRAM",
        "main",
    );
    let byte = "(bit_and i32u (call shr_u32 i32u (read i32u v0^.f0) (const i32s 8)) \
                (const i32u 255))";
    assert_eq!(
        outcome.statements,
        [format!(
            "assign v0^.f1 (ne (bit_and i32u (call shr_u32 i32u {byte} (const i32s 3)) \
             (const i32u 1)) (const i32u 0))"
        )]
    );
}

/// REQ-LOW-lowering-094: an unsigned shift count is converted to its
/// parameter's type, the conversion the analyzer records for it.
#[test]
fn lower_statement_when_unsigned_shift_count_then_converted_to_parameter_type() {
    let outcome = lower(
        "PROGRAM main VAR d : DWORD; l : LWORD; u : UDINT; ul : ULINT; END_VAR
            d := SHL(d, u);
            l := SHR(l, ul);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0 (call shl_u32 i32u (read i32u v0^.f0) (convert i32s (read i32u v0^.f2)))",
            "assign v0^.f1 (call shr_u64 i64u (read i64u v0^.f1) (convert i64s (read i64u v0^.f3)))",
        ]
    );
}

/// A call of a user-defined function and bits read through a reference are
/// not lowered yet.
#[test]
fn lower_statement_when_call_or_read_not_supported_then_unsupported() {
    let outcome = lower(
        "FUNCTION F : DINT VAR_INPUT a : DINT; END_VAR F := a; END_FUNCTION
         PROGRAM main VAR x : BOOL; p : REF_TO WORD; r : DINT; END_VAR
            r := F(1);
            x := p^.3;
         END_PROGRAM",
        "main",
    );
    assert_eq!(outcome.statements, ["unsupported", "unsupported"]);
}
