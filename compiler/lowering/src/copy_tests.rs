//! Tests of lowering whole-aggregate assignments.
//!
//! Each test that checks a requirement of `specs/design/lowered-program.md`
//! names it.

use crate::test_support::{lower, lower_bodies};

/// REQ-LOW-lowering-061: an array assigned from an array of its type or of
/// the same shape is a `Copy`, also of one-dimensional, two-dimensional and
/// string arrays, and of itself.
#[test]
fn lower_statement_when_array_assigned_then_copy() {
    let outcome = lower(
        "PROGRAM main
           VAR x : ARRAY[1..3] OF DINT; y : ARRAY[1..3] OF DINT;
               m : ARRAY[1..2, 1..3] OF INT; n : ARRAY[1..2, 1..3] OF INT;
               s : ARRAY[1..2] OF STRING[8]; t : ARRAY[1..2] OF STRING[8]; END_VAR
            x := y;
            m := n;
            s := t;
            x := x;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "copy v0^.f0 v0^.f1",
            "copy v0^.f2 v0^.f3",
            "copy v0^.f4 v0^.f5",
            "copy v0^.f0 v0^.f0",
        ]
    );
}

/// REQ-LOW-lowering-061: a structure assigned from a structure of its type
/// is a `Copy`, also with an array field.
#[test]
fn lower_statement_when_structure_assigned_then_copy() {
    let outcome = lower(
        "TYPE S : STRUCT n : DINT; a : ARRAY[1..2] OF INT; END_STRUCT; END_TYPE
         PROGRAM main VAR a : S; b : S; END_VAR
            a := b;
         END_PROGRAM",
        "main",
    );
    assert_eq!(outcome.statements, ["copy v0^.f0 v0^.f1"]);
}

/// REQ-LOW-lowering-061: an array global reached through `VAR_EXTERNAL` is
/// copied to and from.
#[test]
fn lower_statement_when_external_array_assigned_then_copy_of_global() {
    let outcome = lower_bodies(
        "CONFIGURATION config
            VAR_GLOBAL g : ARRAY[1..2] OF DINT; END_VAR
            RESOURCE res ON PLC
              TASK t (INTERVAL := T#100ms, PRIORITY := 1);
              PROGRAM p1 WITH t : main;
            END_RESOURCE
         END_CONFIGURATION
         PROGRAM main
           VAR_EXTERNAL g : ARRAY[1..2] OF DINT; END_VAR
           VAR a : ARRAY[1..2] OF DINT; END_VAR
            g := a;
            a := g;
         END_PROGRAM",
        &["main"],
    );
    assert_eq!(outcome.statements, ["copy v1 v0^.f0", "copy v0^.f0 v1"]);
}

/// REQ-LOW-lowering-061: an element that is an aggregate, and a block's
/// output that is a structure, are copied like any other place.
#[test]
fn lower_statement_when_element_or_block_output_then_copy() {
    let outcome = lower(
        "TYPE S : STRUCT n : DINT; END_STRUCT; END_TYPE
         FUNCTION_BLOCK B VAR_OUTPUT p : S; END_VAR p.n := 1; END_FUNCTION_BLOCK
         PROGRAM main
           VAR arr : ARRAY[1..2] OF S; one : S; f : B; r : S; END_VAR
            arr[1] := one;
            r := f.p;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "copy v0^.f0[(const i32s 1)] v0^.f1",
            "copy v0^.f3 v0^.f2.f0",
        ]
    );
}

/// A function block instance, and a structure a function returns, are not
/// copied yet.
#[test]
fn lower_statement_when_instance_or_function_result_assigned_then_unsupported() {
    let outcome = lower(
        "TYPE S : STRUCT n : DINT; END_STRUCT; END_TYPE
         FUNCTION F : S F.n := 1; END_FUNCTION
         FUNCTION_BLOCK B VAR_INPUT i : DINT; END_VAR END_FUNCTION_BLOCK
         PROGRAM main VAR a : B; b : B; s : S; END_VAR
            a := b;
            s := F();
         END_PROGRAM",
        "main",
    );
    assert_eq!(outcome.statements, ["unsupported", "unsupported"]);
}

/// REQ-LOW-lowering-061: an element of an array of arrays is an array, and
/// is copied to, from, and to another element, as a whole.
#[test]
fn lower_statement_when_inner_array_of_array_of_arrays_assigned_then_copy() {
    let outcome = lower(
        "TYPE Row : ARRAY[1..3] OF DINT; END_TYPE
         PROGRAM main VAR rows : ARRAY[1..2] OF Row; row : Row; END_VAR
            rows[1] := row;
            row := rows[2];
            rows[1] := rows[2];
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "copy v0^.f0[(const i32s 1)] v0^.f1",
            "copy v0^.f1 v0^.f0[(const i32s 2)]",
            "copy v0^.f0[(const i32s 1)] v0^.f0[(const i32s 2)]",
        ]
    );
}
