//! End-to-end integration tests for REF_TO reference types.
//!
//! These tests exercise the full pipeline: parse → semantic analysis → codegen → VM execution.
//! All programs use Edition 3 features (REF_TO, REF(), NULL, ^).

mod common;
use common::{parse_and_run_edition3, parse_and_try_run_edition3};
use ironplc_vm::error::Trap;

#[test]
fn end_to_end_when_ref_read_then_dereferences_value() {
    let source = "
PROGRAM main
  VAR
    counter : INT := 42;
    r : REF_TO INT := REF(counter);
    value : INT;
  END_VAR
  value := r^;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run_edition3(source);
    // var layout: counter=0, r=1, value=2
    assert_eq!(bufs.vars[2].as_i32(), 42);
}

#[test]
fn end_to_end_when_ref_write_then_modifies_target() {
    let source = "
PROGRAM main
  VAR
    counter : INT := 0;
    r : REF_TO INT := REF(counter);
  END_VAR
  r^ := 99;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run_edition3(source);
    // counter (var[0]) should be 99 after writing through ref
    assert_eq!(bufs.vars[0].as_i32(), 99);
}

#[test]
fn end_to_end_when_null_deref_read_then_trap() {
    let source = "
PROGRAM main
  VAR
    r : REF_TO INT := NULL;
    value : INT;
  END_VAR
  value := r^;
END_PROGRAM
";
    let err = parse_and_try_run_edition3(source).unwrap_err();
    assert_eq!(err.trap, Trap::NullDereference);
}

#[test]
fn end_to_end_when_null_deref_write_then_trap() {
    let source = "
PROGRAM main
  VAR
    r : REF_TO INT := NULL;
  END_VAR
  r^ := 42;
END_PROGRAM
";
    let err = parse_and_try_run_edition3(source).unwrap_err();
    assert_eq!(err.trap, Trap::NullDereference);
}

#[test]
fn end_to_end_when_default_init_deref_then_trap() {
    // Uninitialized REF_TO should default to NULL and trap on dereference.
    let source = "
PROGRAM main
  VAR
    r : REF_TO INT;
    value : INT;
  END_VAR
  value := r^;
END_PROGRAM
";
    let err = parse_and_try_run_edition3(source).unwrap_err();
    assert_eq!(err.trap, Trap::NullDereference);
}

#[test]
fn end_to_end_when_ref_aliasing_then_both_see_same_value() {
    // Two references to the same variable — write through one, read through the other.
    let source = "
PROGRAM main
  VAR
    counter : INT := 10;
    r1 : REF_TO INT := REF(counter);
    r2 : REF_TO INT := REF(counter);
    result : INT;
  END_VAR
  r1^ := 55;
  result := r2^;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run_edition3(source);
    // counter (var[0]) should be 55
    assert_eq!(bufs.vars[0].as_i32(), 55);
    // result (var[3]) should also be 55 (read through r2)
    assert_eq!(bufs.vars[3].as_i32(), 55);
}

#[test]
fn end_to_end_when_null_check_then_skips_deref() {
    // NULL check with IF prevents dereference.
    let source = "
PROGRAM main
  VAR
    r : REF_TO INT := NULL;
    value : INT := 0;
  END_VAR
  IF r <> NULL THEN
    value := r^;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run_edition3(source);
    // value (var[1]) should remain 0 since r is NULL
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_ref_init_with_ref_then_not_null() {
    // REF_TO with REF(var) initializer should not be NULL.
    let source = "
PROGRAM main
  VAR
    counter : INT := 7;
    r : REF_TO INT := REF(counter);
    is_not_null : INT := 0;
  END_VAR
  IF r <> NULL THEN
    is_not_null := 1;
  END_IF;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run_edition3(source);
    assert_eq!(bufs.vars[2].as_i32(), 1);
}

#[test]
fn end_to_end_when_ref_reassign_then_points_to_new_target() {
    // Reassign a reference to point to a different variable.
    let source = "
PROGRAM main
  VAR
    a : INT := 10;
    b : INT := 20;
    r : REF_TO INT := REF(a);
    result1 : INT;
    result2 : INT;
  END_VAR
  result1 := r^;
  r := REF(b);
  result2 := r^;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run_edition3(source);
    // result1 (var[3]) should be 10 (from a)
    assert_eq!(bufs.vars[3].as_i32(), 10);
    // result2 (var[4]) should be 20 (from b after reassign)
    assert_eq!(bufs.vars[4].as_i32(), 20);
}

// --- REF_TO in FUNCTION context tests ---

#[test]
fn end_to_end_when_function_with_ref_to_input_then_reads_value() {
    let source = "
FUNCTION READ_REF : INT
  VAR_INPUT
    PT : REF_TO INT;
  END_VAR
  READ_REF := PT^;
END_FUNCTION

PROGRAM main
  VAR
    b : INT := 42;
    result : INT;
  END_VAR
  result := READ_REF(PT := REF(b));
END_PROGRAM
";
    let (_c, bufs) = parse_and_run_edition3(source);
    // var layout (globals): b=0, result=1
    assert_eq!(bufs.vars[1].as_i32(), 42);
}

#[test]
fn end_to_end_when_function_with_ref_to_write_then_modifies_target() {
    let source = "
FUNCTION WRITE_REF : BOOL
  VAR_INPUT
    PT : REF_TO INT;
  END_VAR
  PT^ := 99;
  WRITE_REF := TRUE;
END_FUNCTION

PROGRAM main
  VAR
    b : INT := 0;
    result : BOOL;
  END_VAR
  result := WRITE_REF(PT := REF(b));
END_PROGRAM
";
    let (_c, bufs) = parse_and_run_edition3(source);
    // b (var[0]) should be 99 after write through ref
    assert_eq!(bufs.vars[0].as_i32(), 99);
}

#[test]
fn end_to_end_when_function_with_ref_to_local_then_null_initialized() {
    // REF_TO local variables in a function should default to NULL.
    let source = "
FUNCTION CHECK_NULL : BOOL
  VAR_INPUT
    PT : REF_TO INT;
  END_VAR
  VAR
    local_ref : REF_TO INT;
  END_VAR
  CHECK_NULL := (local_ref = NULL);
END_FUNCTION

PROGRAM main
  VAR
    b : INT := 42;
    result : BOOL;
  END_VAR
  result := CHECK_NULL(PT := REF(b));
END_PROGRAM
";
    let (_c, bufs) = parse_and_run_edition3(source);
    // result should be TRUE (1) since local_ref is NULL
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_function_with_multiple_ref_to_types_then_compiles() {
    // Verifies that REF_TO works with different types (BYTE, DWORD)
    // in both VAR_INPUT and VAR blocks of a FUNCTION.
    let source = "
FUNCTION TEST : BOOL
  VAR_INPUT
    PT : REF_TO BYTE;
  END_VAR
  VAR
    ptw : REF_TO DWORD;
  END_VAR
  TEST := TRUE;
END_FUNCTION

PROGRAM main
  VAR
    result : BOOL;
    b : BYTE := 42;
  END_VAR
  result := TEST(PT := REF(b));
END_PROGRAM
";
    let (_c, bufs) = parse_and_run_edition3(source);
    // result (var[0]) should be TRUE (1)
    assert_eq!(bufs.vars[0].as_i32(), 1);
}
