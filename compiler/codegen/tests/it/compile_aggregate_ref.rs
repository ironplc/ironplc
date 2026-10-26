//! Tests of a whole array or structure assigned to the one a reference
//! refers to, `p^ := z`, compiled from the lowered program
//! (REQ-LOW-codegen-193).

use ironplc_vm::error::Trap;

use crate::common::{compile_analyzed, parse, run_one_scan};
use crate::compile_body::{compile_and_run, options};

/// A copy through a reference gives the referenced array or structure the
/// source's values, and storage of its own: a later store to the source
/// leaves it as it is (REQ-LOW-codegen-165).
#[test]
fn run_when_aggregate_copied_through_reference_then_referenced_holds_copy() {
    let snapshot = compile_and_run(
        "TYPE P : STRUCT x : DINT; y : DINT; END_STRUCT; END_TYPE
         PROGRAM main
           VAR a : ARRAY[1..3] OF DINT := [1, 2, 3]; z : ARRAY[1..3] OF DINT := [7, 8, 9];
               s : P; t : P := (x := 5, y := 6);
               sa : ARRAY[1..2] OF STRING[4]; sz : ARRAY[1..2] OF STRING[4] := ['ab', 'cd'];
               p : REF_TO ARRAY[1..3] OF DINT; q : REF_TO P;
               ps : REF_TO ARRAY[1..2] OF STRING[4];
               a2 : DINT; z2 : DINT; sy : DINT; ty : DINT; sa2 : STRING[4]; END_VAR
           p := REF(a);
           q := REF(s);
           ps := REF(sa);
           p^ := z;
           q^ := t;
           ps^ := sz;
           z[2] := 0;
           t.y := 0;
           sz[2] := 'zz';
           a2 := a[2];
           z2 := z[2];
           sy := s.y;
           ty := t.y;
           sa2 := sa[2];
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i32>("a2"), 8);
    assert_eq!(snapshot.read_as::<i32>("z2"), 0);
    assert_eq!(snapshot.read_as::<i32>("sy"), 6);
    assert_eq!(snapshot.read_as::<i32>("ty"), 0);
    assert_eq!(snapshot.read("sa2"), "cd");
}

/// A `REFERENCE TO` an array names the array it is bound to, and a whole
/// array assigned to it is copied into that array.
#[test]
fn run_when_array_assigned_to_reference_to_then_bound_array_holds_copy() {
    let snapshot = compile_and_run(
        "PROGRAM main
           VAR a : ARRAY[1..3] OF DINT; z : ARRAY[1..3] OF DINT := [4, 5, 6];
               r : REFERENCE TO ARRAY[1..3] OF DINT; a3 : DINT; END_VAR
           r REF= a;
           r := z;
           z[3] := 0;
           a3 := a[3];
         END_PROGRAM",
    );
    assert_eq!(snapshot.read_as::<i32>("a3"), 6);
}

/// A copy through a null reference traps, as any store through one does.
#[test]
fn run_when_aggregate_copied_through_null_reference_then_null_dereference() {
    let options = options();
    let (library, context) = parse(
        "PROGRAM main
           VAR z : ARRAY[1..3] OF DINT; p : REF_TO ARRAY[1..3] OF DINT; END_VAR
           p^ := z;
         END_PROGRAM",
        &options,
    );
    let container = compile_analyzed(&library, &context, &options).expect("compiles");
    let Err(fault) = run_one_scan(&container) else {
        panic!("the scan does not trap");
    };
    assert_eq!(fault.trap, Trap::NullDereference);
}
