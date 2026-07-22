//! End-to-end integration tests for TwinCAT `REFERENCE TO` reference types.
//!
//! These exercise the full pipeline — parse -> semantic analysis -> codegen ->
//! VM execution — for the TwinCAT surface syntax (`REFERENCE TO` declarations
//! and the `REF=` binding operator) accessed via the explicit `^` operator.
//! They prove the feature reuses the existing `REF_TO` backend unchanged.
//! See `specs/design/reference-to-twincat.md`.

use crate::common::{parse_and_run, parse_and_try_run};
use ironplc_parser::options::CompilerOptions;
use ironplc_vm::error::Trap;

fn reference_to_options() -> CompilerOptions {
    CompilerOptions {
        allow_reference_to: true,
        ..CompilerOptions::default()
    }
}

#[test]
fn end_to_end_when_reference_to_bound_then_reads_through_caret() {
    let source = "
PROGRAM main
  VAR
    x : INT := 42;
    r : REFERENCE TO INT;
    y : INT;
  END_VAR
  r REF= x;
  y := r^;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &reference_to_options());
    // vars: x=0, r=1, y=2
    assert_eq!(bufs.vars[2].as_i32(), 42);
}

#[test]
fn end_to_end_when_reference_to_written_then_updates_referent() {
    let source = "
PROGRAM main
  VAR
    x : INT := 1;
    r : REFERENCE TO INT;
  END_VAR
  r REF= x;
  r^ := 99;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &reference_to_options());
    assert_eq!(bufs.vars[0].as_i32(), 99);
}

#[test]
fn end_to_end_when_reference_to_named_type_then_runs() {
    let source = "
TYPE IntRef : REFERENCE TO INT; END_TYPE

PROGRAM main
  VAR
    x : INT := 7;
    r : IntRef;
    y : INT;
  END_VAR
  r REF= x;
  y := r^;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &reference_to_options());
    // vars: x=0, r=1, y=2
    assert_eq!(bufs.vars[2].as_i32(), 7);
}

#[test]
fn end_to_end_when_array_of_reference_to_element_bound_then_reads() {
    let source = "
PROGRAM main
  VAR
    val : INT := 77;
    refs : ARRAY[0..2] OF REFERENCE TO INT;
    result : INT;
  END_VAR
  refs[0] REF= val;
  result := refs[0]^;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &reference_to_options());
    // vars: val=0, refs=1, result=2
    assert_eq!(bufs.vars[2].as_i32(), 77);
}

#[test]
fn end_to_end_when_unbound_reference_to_dereferenced_then_traps() {
    let source = "
PROGRAM main
  VAR
    r : REFERENCE TO INT;
    y : INT;
  END_VAR
  y := r^;
END_PROGRAM
";
    let err = parse_and_try_run(source, &reference_to_options()).unwrap_err();
    assert_eq!(err.trap, Trap::NullDereference);
}
