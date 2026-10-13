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
/// is not lowered until the analyzer records its conversion as checked.
#[test]
fn lower_statement_when_subscript_may_not_fit_dint_then_unsupported() {
    let outcome = lower(
        "PROGRAM main VAR a : ARRAY[1..5] OF DINT; ud : UDINT; l : LINT; END_VAR
            a[ud] := 1;
            a[l] := 1;
         END_PROGRAM",
        "main",
    );
    assert_eq!(outcome.statements, ["unsupported", "unsupported"]);
}

/// REQ-LOW-lowering-161: an unsigned subscript narrower than `DINT` is
/// converted to `DINT`, as the analyzer recorded.
#[test]
fn lower_statement_when_unsigned_subscript_narrower_than_dint_then_converted() {
    let outcome = lower(
        "PROGRAM main VAR a : ARRAY[1..5] OF DINT; u : UINT; w : WORD; END_VAR
            a[u] := 1;
            a[w] := 1;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0[(convert i32s (read i32u v0^.f1))] (const i32s 1)",
            "assign v0^.f0[(convert i32s (read i32u v0^.f2))] (const i32s 1)",
        ]
    );
}

/// A `BOOL` subscript, which analysis accepts, is not lowered.
#[test]
fn lower_statement_when_bool_subscript_then_unsupported() {
    let outcome = lower(
        "PROGRAM main VAR a : ARRAY[0..1] OF DINT; b : BOOL; END_VAR
            a[b] := 1;
         END_PROGRAM",
        "main",
    );
    assert_eq!(outcome.statements, ["unsupported"]);
}
