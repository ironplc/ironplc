//! End-to-end integration tests for REF_TO ARRAY support.
//! Compiles ST programs with references to array types and runs them through the VM.

mod common;

use common::{parse_and_compile_edition3, parse_and_run_edition3};

#[test]
fn end_to_end_when_ref_to_array_declared_then_compiles() {
    let source = "
PROGRAM main
  VAR
    data : REF_TO ARRAY[1..5] OF INT;
    marker : INT := 42;
  END_VAR
END_PROGRAM
";
    let _container = parse_and_compile_edition3(source);
}

#[test]
fn end_to_end_when_ref_to_array_type_decl_then_compiles() {
    let source = "
TYPE ArrRef : REF_TO ARRAY[0..3] OF DINT; END_TYPE

PROGRAM main
  VAR
    arr : ArrRef;
    result : DINT := 7;
  END_VAR
END_PROGRAM
";
    let _container = parse_and_compile_edition3(source);
}

#[test]
fn end_to_end_when_ref_to_array_declared_then_runs() {
    let source = "
PROGRAM main
  VAR
    data : REF_TO ARRAY[0..3] OF INT;
    x : INT := 99;
  END_VAR
END_PROGRAM
";
    let (_c, bufs) = parse_and_run_edition3(source);
    // x is at var index 1 (data is var 0, x is var 1)
    assert_eq!(bufs.vars[1].as_i32(), 99);
}
