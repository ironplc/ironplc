//! End-to-end integration tests for REF_TO reference types.
//!
//! These tests exercise the full pipeline: parse → semantic analysis → codegen → VM execution.
//! All programs use Edition 3 features (REF_TO, REF(), NULL, ^).

mod common;
use common::{parse_and_run, parse_and_try_run};
use ironplc_parser::options::{CompilerOptions, Dialect};
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
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
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
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
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
    let err = parse_and_try_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    )
    .unwrap_err();
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
    let err = parse_and_try_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    )
    .unwrap_err();
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
    let err = parse_and_try_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    )
    .unwrap_err();
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
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
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
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
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
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
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
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
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
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
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
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
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
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
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
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
    // result (var[0]) should be TRUE (1)
    assert_eq!(bufs.vars[0].as_i32(), 1);
}

#[test]
fn end_to_end_when_function_with_deref_array_subscript_then_writes_through_ref() {
    // Verifies that PT^[0] syntax (dereference + array subscript) actually
    // writes to the target array through the reference at runtime.
    // Exercises the STORE_ARRAY_DEREF codegen and VM path.
    let source = "
FUNCTION write_array : INT
  VAR_INPUT
      PT : REF_TO ARRAY[0..10] OF BYTE;
  END_VAR
      PT^[0] := BYTE#42;
      write_array := 1;
END_FUNCTION

PROGRAM main
VAR
    arr : ARRAY[0..10] OF BYTE;
    result : INT;
    check : BYTE;
END_VAR
    result := write_array(PT := REF(arr));
    check := arr[0];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
    // result (var[1]) should be 1 (function return value)
    assert_eq!(bufs.vars[1].as_i32(), 1);
    // check (var[2]) should be 42 (written through REF)
    assert_eq!(bufs.vars[2].as_i32(), 42);
}

#[test]
fn end_to_end_when_function_with_local_ref_to_array_deref_subscript_then_writes_through_ref() {
    // Verifies that a local REF_TO ARRAY variable (not a parameter) can be used
    // with deref array subscript syntax (local_pt^[0]).
    let source = "
FUNCTION fill_first : INT
  VAR_INPUT
      arr_ref : REF_TO ARRAY[0..4] OF INT;
  END_VAR
  VAR
      local_pt : REF_TO ARRAY[0..4] OF INT;
  END_VAR
      local_pt := arr_ref;
      local_pt^[0] := 77;
      fill_first := 1;
END_FUNCTION

PROGRAM main
VAR
    arr : ARRAY[0..4] OF INT;
    result : INT;
    check : INT;
END_VAR
    result := fill_first(arr_ref := REF(arr));
    check := arr[0];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
    // result (var[1]) should be 1 (function return value)
    assert_eq!(bufs.vars[1].as_i32(), 1);
    // check (var[2]) should be 77 (written through local REF)
    assert_eq!(bufs.vars[2].as_i32(), 77);
}

#[test]
fn end_to_end_when_program_ref_to_array_deref_subscript_then_reads_element() {
    // Verifies that a program-level REF_TO ARRAY variable can be used
    // with deref array subscript syntax (pt^[i]) to read array elements.
    // This is the pattern used in OSCAT buffer/list functions.
    let source = "
FUNCTION GET_ELEMENT : REAL
VAR_INPUT
    pt : REF_TO ARRAY[0..100] OF REAL;
    i : INT;
END_VAR
    GET_ELEMENT := pt^[i];
END_FUNCTION

PROGRAM main
VAR
    arr : ARRAY[0..100] OF REAL;
    r : REAL;
END_VAR
    arr[0] := 42.0;
    r := GET_ELEMENT(pt := REF(arr), i := 0);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
    // r (var[1]) should be 42.0 (REAL is 32-bit float)
    assert_eq!(bufs.vars[1].as_f32(), 42.0);
}

#[test]
fn end_to_end_when_ref_to_byte_array_deref_subscript_in_comparison_then_correct() {
    // Verifies that pt^[i] used in a comparison context works end-to-end.
    // This exercises the resolved_type on deref+array subscript expressions,
    // which is needed for op_type() in comparison codegen.
    let source = "
FUNCTION READ_ELEM : BYTE
VAR_INPUT
    pt : REF_TO ARRAY[0..10] OF BYTE;
    i : INT;
END_VAR
    READ_ELEM := pt^[i];
END_FUNCTION

PROGRAM main
VAR
    arr : ARRAY[0..10] OF BYTE;
    b : BYTE;
    found : BOOL;
END_VAR
    arr[0] := BYTE#42;
    b := READ_ELEM(pt := REF(arr), i := 0);
    found := b > BYTE#0;
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
    // b (var[1]) should be 42
    assert_eq!(bufs.vars[1].as_i32(), 42);
    // found (var[2]) should be TRUE (42 > 0)
    assert_eq!(bufs.vars[2].as_i32(), 1);
}

#[test]
fn end_to_end_when_fb_with_deref_array_subscript_then_writes_through_ref() {
    // Verifies that PT^[0] syntax (dereference + array subscript) works
    // inside a FUNCTION_BLOCK, not just inside a FUNCTION.
    let source = "
FUNCTION_BLOCK ARRAY_WRITER
  VAR_INPUT
      PT : REF_TO ARRAY[0..10] OF BYTE;
  END_VAR
      PT^[0] := BYTE#42;
END_FUNCTION_BLOCK

PROGRAM main
VAR
    arr : ARRAY[0..10] OF BYTE;
    fb : ARRAY_WRITER;
    check : BYTE;
END_VAR
    fb(PT := REF(arr));
    check := arr[0];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
    // check (var[2]) should be 42 (written through REF in FB)
    assert_eq!(bufs.vars[2].as_i32(), 42);
}

#[test]
fn end_to_end_when_fb_with_local_ref_to_array_deref_subscript_then_writes_through_ref() {
    // Verifies that a local REF_TO ARRAY variable inside a FUNCTION_BLOCK
    // can be used with deref array subscript syntax (local_pt^[0]).
    let source = "
FUNCTION_BLOCK COPY_FB
  VAR_INPUT
      src : REF_TO ARRAY[0..4] OF INT;
  END_VAR
  VAR
      local_pt : REF_TO ARRAY[0..4] OF INT;
  END_VAR
      local_pt := src;
      local_pt^[0] := 99;
END_FUNCTION_BLOCK

PROGRAM main
VAR
    arr : ARRAY[0..4] OF INT;
    fb : COPY_FB;
    check : INT;
END_VAR
    fb(src := REF(arr));
    check := arr[0];
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(
        source,
        &CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3),
    );
    // check (var[2]) should be 99 (written through local REF in FB)
    assert_eq!(bufs.vars[2].as_i32(), 99);
}
