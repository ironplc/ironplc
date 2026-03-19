//! End-to-end integration tests for ARRAY OF REF_TO support.
//! Compiles ST programs with arrays of reference types and runs them through the VM.

mod common;

use common::{parse_and_compile_edition3, parse_and_run_edition3};

#[test]
fn end_to_end_when_array_of_ref_to_declared_then_compiles_and_runs() {
    let source = "
PROGRAM main
  VAR
    data : ARRAY[0..3] OF REF_TO BYTE;
    x : INT := 42;
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run_edition3(source);
    // x is at var index 1 (data is var 0, x is var 1)
    assert_eq!(bufs.vars[1].as_i32(), 42);
}

#[test]
fn end_to_end_when_array_of_ref_to_store_ref_then_roundtrips() {
    let source = "
PROGRAM main
  VAR
    val : INT := 77;
    refs : ARRAY[0..2] OF REF_TO INT;
    result : INT;
  END_VAR
  refs[0] := REF(val);
  result := refs[0]^;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run_edition3(source);
    // val=0, refs=1, result=2
    assert_eq!(bufs.vars[2].as_i32(), 77);
}

#[test]
fn end_to_end_when_array_of_ref_to_int_then_compiles() {
    let source = "
PROGRAM main
  VAR
    refs : ARRAY[1..5] OF REF_TO INT;
    marker : INT := 99;
  END_VAR
END_PROGRAM
";
    let _container = parse_and_compile_edition3(source);
}

#[test]
fn end_to_end_when_array_of_ref_to_type_decl_then_compiles() {
    let source = "
TYPE RefArray : ARRAY[0..2] OF REF_TO DINT; END_TYPE

PROGRAM main
  VAR
    arr : RefArray;
    result : DINT := 7;
  END_VAR
END_PROGRAM
";
    let _container = parse_and_compile_edition3(source);
}
