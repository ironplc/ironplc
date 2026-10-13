//! Tests of lowering places named in the source.
//!
//! Each test that checks a requirement of `specs/design/lowered-program.md`
//! names it.

use crate::test_support::lower;

/// REQ-LOW-lowering-031: an element of an array is an `Index` with one
/// subscript per dimension, in order, each a `DINT`.
#[test]
fn lower_statement_when_array_element_then_index_with_subscripts() {
    let outcome = lower(
        "PROGRAM main VAR a : ARRAY[1..5] OF DINT; m : ARRAY[1..2, 0..3] OF DINT; i : DINT; END_VAR
            a[i] := a[i + 1];
            m[1, i] := 2;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0[(read i32s v0^.f2)] \
             (read i32s v0^.f0[(add i32s (read i32s v0^.f2) (const i32s 1))])",
            "assign v0^.f1[(const i32s 1), (read i32s v0^.f2)] (const i32s 2)",
        ]
    );
}

/// REQ-LOW-lowering-030: a field of a structure is a `Field`, and a store to
/// a narrow field is narrowed.
#[test]
fn lower_statement_when_structure_field_then_field() {
    let outcome = lower(
        "TYPE P : STRUCT x : DINT; y : INT; END_STRUCT; END_TYPE
         PROGRAM main VAR p : P; END_VAR
            p.y := p.y + 1;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        ["assign v0^.f0.f1 (truncate 16 (add i32s (read i32s v0^.f0.f1) (const i32s 1)))"]
    );
}

/// An element of an array of structures, and a field of it, compose.
#[test]
fn lower_statement_when_field_of_array_element_then_index_then_field() {
    let outcome = lower(
        "TYPE P : STRUCT x : DINT; END_STRUCT; END_TYPE
         PROGRAM main VAR ps : ARRAY[0..3] OF P; i : DINT; END_VAR
            ps[i].x := ps[2].x;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        ["assign v0^.f0[(read i32s v0^.f1)].f0 (read i32s v0^.f0[(const i32s 2)].f0)"]
    );
}

/// REQ-LOW-lowering-161: a subscript whose type holds values outside `DINT`
/// is not lowered until the analyzer records its conversion as checked, and
/// an unsigned subscript the analyzer does not convert to `DINT` is not
/// lowered either.
#[test]
fn lower_statement_when_subscript_not_dint_then_unsupported() {
    let outcome = lower(
        "PROGRAM main VAR a : ARRAY[1..5] OF DINT; u : UINT; ud : UDINT; l : LINT; END_VAR
            a[u] := 1;
            a[ud] := 1;
            a[l] := 1;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        ["unsupported", "unsupported", "unsupported"]
    );
}

/// REQ-LOW-lowering-032: what a reference refers to is a `Deref` of the
/// place holding the reference, read and written, and a store through a
/// reference to a narrow variable is narrowed.
#[test]
fn lower_statement_when_dereference_then_deref() {
    let outcome = lower(
        "PROGRAM main VAR x : DINT; n : INT; p : REF_TO DINT; q : REF_TO INT; END_VAR
            p^ := p^ + 1;
            q^ := q^ + 1;
            x := p^;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f2^ (add i32s (read i32s v0^.f2^) (const i32s 1))",
            "assign v0^.f3^ (truncate 16 (add i32s (read i32s v0^.f3^) (const i32s 1)))",
            "assign v0^.f0 (read i32s v0^.f2^)",
        ]
    );
}

/// An element of an array a reference refers to is a `Deref`, then an
/// `Index`.
#[test]
fn lower_statement_when_element_through_reference_then_deref_then_index() {
    let outcome = lower(
        "PROGRAM main VAR pa : REF_TO ARRAY[1..3] OF DINT; i : DINT; END_VAR
            pa^[i] := pa^[3];
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        ["assign v0^.f0^[(read i32s v0^.f1)] (read i32s v0^.f0^[(const i32s 3)])"]
    );
}

/// REQ-LOW-lowering-102: a bit or partial write is one `AssignBits` that
/// names its place once, whatever the shape of the place.
#[test]
fn lower_statement_when_bit_or_partial_write_then_assign_bits() {
    let outcome = lower(
        "PROGRAM main VAR w : WORD; b : BYTE; a : ARRAY[0..3] OF DWORD; i : DINT; END_VAR
            w.3 := TRUE;
            w.%B1 := b;
            a[i].%W1 := w;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign-bits v0^.f0 3 1 (const bool true)",
            "assign-bits v0^.f0 8 8 (read i32u v0^.f1)",
            "assign-bits v0^.f2[(read i32s v0^.f3)] 16 16 (read i32u v0^.f0)",
        ]
    );
}

/// Bits reached through a reference, `p^.3`, are not lowered while codegen's
/// AST route does not address them.
#[test]
fn lower_statement_when_bits_written_through_reference_then_unsupported() {
    let outcome = lower(
        "PROGRAM main VAR w : WORD; p : REF_TO WORD; END_VAR
            p^.3 := TRUE;
         END_PROGRAM",
        "main",
    );
    assert_eq!(outcome.statements, ["unsupported"]);
}

/// The bits of a bit selection, `d.%B1.3`, are not a place to write.
#[test]
fn lower_statement_when_bits_of_bits_written_then_unsupported() {
    let outcome = lower(
        "PROGRAM main VAR d : DWORD; END_VAR
            d.%B1.3 := TRUE;
         END_PROGRAM",
        "main",
    );
    assert_eq!(outcome.statements, ["unsupported"]);
}
