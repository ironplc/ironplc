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

/// REQ-LOW-lowering-161: an unsigned subscript whose values all fit a `DINT`
/// is converted to `DINT`, the conversion the analyzer records for it
/// (REQ-LOW-lowering-094).
#[test]
fn lower_statement_when_unsigned_subscript_fits_dint_then_converted_to_dint() {
    let outcome = lower(
        "PROGRAM main VAR a : ARRAY[0..5] OF DINT; u : UINT; b : BYTE; END_VAR
            a[u] := 1;
            a[b] := a[u];
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0[(convert i32s (read i32u v0^.f1))] (const i32s 1)",
            "assign v0^.f0[(convert i32s (read i32u v0^.f2))] \
             (read i32s v0^.f0[(convert i32s (read i32u v0^.f1))])",
        ]
    );
}

/// REQ-LOW-lowering-163: a subscript whose type holds values outside
/// `DINT`, whose conversion the analyzer records as checked
/// (REQ-LOW-analyzer-162), is converted to `DINT` by a `ConvertChecked`.
#[test]
fn lower_statement_when_subscript_exceeds_dint_then_convert_checked() {
    let outcome = lower(
        "PROGRAM main VAR a : ARRAY[1..5] OF DINT; ud : UDINT; l : LINT; END_VAR
            a[ud] := 1;
            a[l] := a[l + 1];
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0[(convert_checked i32s (read i32u v0^.f1))] (const i32s 1)",
            "assign v0^.f0[(convert_checked i32s (read i64s v0^.f2))] \
             (read i32s v0^.f0[(convert_checked i32s \
             (add i64s (read i64s v0^.f2) (const i64s 1)))])",
        ]
    );
}

/// `REF(x)` is a reference to the place `x` names, and `NULL` the null
/// reference, each stored in and compared with a `REF_TO`.
#[test]
fn lower_statement_when_ref_and_null_then_ref_to_and_null() {
    let outcome = lower(
        "PROGRAM main VAR x : DINT; p : REF_TO DINT; b : BOOL; END_VAR
            p := REF(x);
            b := p = NULL;
            p := NULL;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f1 (ref_to v0^.f0)",
            "assign v0^.f2 (eq (read ref v0^.f1) (null))",
            "assign v0^.f1 (null)",
        ]
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

/// Bits of an element reached through a reference are one `AssignBits` on
/// the element, which is a `Deref` then an `Index`.
#[test]
fn lower_statement_when_bits_of_element_through_reference_written_then_assign_bits() {
    let outcome = lower(
        "PROGRAM main VAR p : REF_TO ARRAY[0..1] OF DWORD; b : BYTE; END_VAR
            p^[1].0 := TRUE;
            p^[1].%B2 := b;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign-bits v0^.f0^[(const i32s 1)] 0 1 (const bool true)",
            "assign-bits v0^.f0^[(const i32s 1)] 16 8 (read i32u v0^.f1)",
        ]
    );
}

/// Bits of the whole of what a reference refers to, `p^.3`, are not lowered
/// while codegen's AST route does not address them.
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

/// A `BOOL` subscript, which analysis accepts, is not lowered.
#[test]
fn lower_statement_when_bool_subscript_then_unsupported() {
    let outcome = lower(
        "PROGRAM main VAR a : ARRAY[0..1] OF DINT; b : BOOL; x : DINT; END_VAR
            a[b] := 1;
            x := a[b];
         END_PROGRAM",
        "main",
    );
    assert_eq!(outcome.statements, ["unsupported", "unsupported"]);
}

/// A field of an instance of a standard function block is a field of the
/// instance's field, as one of a user-defined block is: `PV` and `CV` are
/// the third and fifth fields of a `CTU`, and `Q` the third of a `TON`.
#[test]
fn lower_statement_when_field_of_standard_block_then_field_of_instance() {
    let outcome = lower(
        "PROGRAM main VAR c : CTU; t : TON; x : INT; q : BOOL; END_VAR
            x := c.CV;
            q := t.Q;
            c.PV := 7;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f2 (read i32s v0^.f0.f4)",
            "assign v0^.f3 (read bool v0^.f1.f2)",
            "assign v0^.f0.f2 (const i32s 7)",
        ]
    );
}

/// REQ-LOW-lowering-031: an element of an array of arrays is an `Index` for
/// each array, outermost first, whether the outer array's type is named or
/// declared in place.
#[test]
fn lower_statement_when_element_of_array_of_arrays_then_index_for_each_array() {
    let outcome = lower(
        "TYPE Row : ARRAY[1..3] OF DINT; Rows : ARRAY[0..1] OF Row; END_TYPE
         PROGRAM main VAR rows : ARRAY[1..2] OF Row; named : Rows; i : DINT; END_VAR
            rows[i][2] := named[0][i];
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        ["assign v0^.f0[(read i32s v0^.f2)][(const i32s 2)] \
          (read i32s v0^.f1[(const i32s 0)][(read i32s v0^.f2)])"]
    );
}

/// An element of a two-dimensional array of arrays takes both subscripts of
/// the outer array in one `Index`, then one for the inner array.
#[test]
fn lower_statement_when_two_dimensional_array_of_arrays_then_index_for_each_array() {
    let outcome = lower(
        "TYPE Row : ARRAY[1..3] OF DINT; END_TYPE
         PROGRAM main VAR grid : ARRAY[1..2, 1..2] OF Row; END_VAR
            grid[2, 1][3] := 8;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        ["assign v0^.f0[(const i32s 2), (const i32s 1)][(const i32s 3)] (const i32s 8)"]
    );
}

/// Fields and elements compose through arrays of arrays: an array of arrays
/// in a structure, a structure in an array of arrays, with a string field,
/// and an array in an element of an array of structures.
#[test]
fn lower_statement_when_fields_and_arrays_of_arrays_then_projections_compose() {
    let outcome = lower(
        "TYPE Row : ARRAY[1..3] OF DINT;
              S : STRUCT rows : ARRAY[1..2] OF Row; END_STRUCT;
              N : STRUCT n : DINT; t : STRING[5]; END_STRUCT;
              NRow : ARRAY[1..3] OF N;
              R : STRUCT v : ARRAY[1..3] OF DINT; END_STRUCT; END_TYPE
         PROGRAM main VAR s : S; g : ARRAY[1..2] OF NRow; recs : ARRAY[1..2] OF R; END_VAR
            s.rows[2][3] := 1;
            g[2][3].n := 2;
            g[1][1].t := 'ab';
            recs[2].v[3] := 3;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0.f0[(const i32s 2)][(const i32s 3)] (const i32s 1)",
            "assign v0^.f1[(const i32s 2)][(const i32s 3)].f0 (const i32s 2)",
            "assign-str v0^.f1[(const i32s 1)][(const i32s 1)].f1 (literal string[2] \"ab\")",
            "assign v0^.f2[(const i32s 2)].f0[(const i32s 3)] (const i32s 3)",
        ]
    );
}
