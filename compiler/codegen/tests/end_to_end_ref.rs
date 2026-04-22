//! End-to-end integration tests for REF_TO reference types.
//!
//! These tests exercise the full pipeline: parse → semantic analysis → codegen → VM execution.
//! All programs use Edition 3 features (REF_TO, REF(), NULL, ^).

#[macro_use]
mod common;
use common::{parse_and_run, parse_and_try_run};
use ironplc_parser::options::{CompilerOptions, Dialect};
use ironplc_vm::error::Trap;

fn ed3() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3)
}

// --- Basic REF_TO read / write ---

// var layout: counter=0, r=1, value=2
e2e_i32_with!(
    end_to_end_when_ref_read_then_dereferences_value,
    ed3(),
    "PROGRAM main VAR counter : INT := 42; r : REF_TO INT := REF(counter); value : INT; END_VAR value := r^; END_PROGRAM",
    &[(2, 42)],
);

// counter (var[0]) should be 99 after writing through ref.
e2e_i32_with!(
    end_to_end_when_ref_write_then_modifies_target,
    ed3(),
    "PROGRAM main VAR counter : INT := 0; r : REF_TO INT := REF(counter); END_VAR r^ := 99; END_PROGRAM",
    &[(0, 99)],
);

// Two references to one var — write via one, read via the other.
e2e_i32_with!(
    end_to_end_when_ref_aliasing_then_both_see_same_value,
    ed3(),
    "PROGRAM main VAR counter : INT := 10; r1 : REF_TO INT := REF(counter); r2 : REF_TO INT := REF(counter); result : INT; END_VAR r1^ := 55; result := r2^; END_PROGRAM",
    &[(0, 55), (3, 55)],
);

// NULL check with IF prevents dereference: value (var[1]) stays 0.
e2e_i32_with!(
    end_to_end_when_null_check_then_skips_deref,
    ed3(),
    "PROGRAM main VAR r : REF_TO INT := NULL; value : INT := 0; END_VAR IF r <> NULL THEN value := r^; END_IF; END_PROGRAM",
    &[(1, 0)],
);

// REF(var) initializer produces a non-NULL reference.
e2e_i32_with!(
    end_to_end_when_ref_init_with_ref_then_not_null,
    ed3(),
    "PROGRAM main VAR counter : INT := 7; r : REF_TO INT := REF(counter); is_not_null : INT := 0; END_VAR IF r <> NULL THEN is_not_null := 1; END_IF; END_PROGRAM",
    &[(2, 1)],
);

// result1 (var[3]) = a = 10; result2 (var[4]) = b = 20 after reassign.
e2e_i32_with!(
    end_to_end_when_ref_reassign_then_points_to_new_target,
    ed3(),
    "PROGRAM main VAR a : INT := 10; b : INT := 20; r : REF_TO INT := REF(a); result1 : INT; result2 : INT; END_VAR result1 := r^; r := REF(b); result2 := r^; END_PROGRAM",
    &[(3, 10), (4, 20)],
);

// --- NULL dereference traps ---

fn assert_null_deref(source: &str) {
    let err = parse_and_try_run(source, &ed3()).unwrap_err();
    assert_eq!(err.trap, Trap::NullDereference);
}

#[test]
fn end_to_end_when_null_deref_read_then_trap() {
    assert_null_deref(
        "PROGRAM main VAR r : REF_TO INT := NULL; value : INT; END_VAR value := r^; END_PROGRAM",
    );
}

#[test]
fn end_to_end_when_null_deref_write_then_trap() {
    assert_null_deref("PROGRAM main VAR r : REF_TO INT := NULL; END_VAR r^ := 42; END_PROGRAM");
}

#[test]
fn end_to_end_when_default_init_deref_then_trap() {
    assert_null_deref(
        "PROGRAM main VAR r : REF_TO INT; value : INT; END_VAR value := r^; END_PROGRAM",
    );
}

// --- REF_TO as a FUNCTION/FUNCTION_BLOCK parameter ---

// var layout (globals): b=0, result=1
e2e_i32_with!(
    end_to_end_when_function_with_ref_to_input_then_reads_value,
    ed3(),
    "FUNCTION READ_REF : INT VAR_INPUT PT : REF_TO INT; END_VAR READ_REF := PT^; END_FUNCTION PROGRAM main VAR b : INT := 42; result : INT; END_VAR result := READ_REF(PT := REF(b)); END_PROGRAM",
    &[(1, 42)],
);

// b (var[0]) = 99 after write through ref inside function.
e2e_i32_with!(
    end_to_end_when_function_with_ref_to_write_then_modifies_target,
    ed3(),
    "FUNCTION WRITE_REF : BOOL VAR_INPUT PT : REF_TO INT; END_VAR PT^ := 99; WRITE_REF := TRUE; END_FUNCTION PROGRAM main VAR b : INT := 0; result : BOOL; END_VAR result := WRITE_REF(PT := REF(b)); END_PROGRAM",
    &[(0, 99)],
);

// result (var[1]) = 1 because local_ref defaults to NULL.
e2e_i32_with!(
    end_to_end_when_function_with_ref_to_local_then_null_initialized,
    ed3(),
    "FUNCTION CHECK_NULL : BOOL VAR_INPUT PT : REF_TO INT; END_VAR VAR local_ref : REF_TO INT; END_VAR CHECK_NULL := (local_ref = NULL); END_FUNCTION PROGRAM main VAR b : INT := 42; result : BOOL; END_VAR result := CHECK_NULL(PT := REF(b)); END_PROGRAM",
    &[(1, 1)],
);

// REF_TO of different element types (BYTE, DWORD) compiles.
e2e_i32_with!(
    end_to_end_when_function_with_multiple_ref_to_types_then_compiles,
    ed3(),
    "FUNCTION TEST : BOOL VAR_INPUT PT : REF_TO BYTE; END_VAR VAR ptw : REF_TO DWORD; END_VAR TEST := TRUE; END_FUNCTION PROGRAM main VAR result : BOOL; b : BYTE := 42; END_VAR result := TEST(PT := REF(b)); END_PROGRAM",
    &[(0, 1)],
);

// PT^[0] (deref + array subscript) writes arr[0] = 42 through the ref.
e2e_i32_with!(
    end_to_end_when_function_with_deref_array_subscript_then_writes_through_ref,
    ed3(),
    "FUNCTION write_array : INT VAR_INPUT PT : REF_TO ARRAY[0..10] OF BYTE; END_VAR PT^[0] := BYTE#42; write_array := 1; END_FUNCTION PROGRAM main VAR arr : ARRAY[0..10] OF BYTE; result : INT; check : BYTE; END_VAR result := write_array(PT := REF(arr)); check := arr[0]; END_PROGRAM",
    &[(1, 1), (2, 42)],
);

// Local REF_TO ARRAY variable used with deref+subscript: arr[0] = 77.
e2e_i32_with!(
    end_to_end_when_function_with_local_ref_to_array_deref_subscript_then_writes_through_ref,
    ed3(),
    "FUNCTION fill_first : INT VAR_INPUT arr_ref : REF_TO ARRAY[0..4] OF INT; END_VAR VAR local_pt : REF_TO ARRAY[0..4] OF INT; END_VAR local_pt := arr_ref; local_pt^[0] := 77; fill_first := 1; END_FUNCTION PROGRAM main VAR arr : ARRAY[0..4] OF INT; result : INT; check : INT; END_VAR result := fill_first(arr_ref := REF(arr)); check := arr[0]; END_PROGRAM",
    &[(1, 1), (2, 77)],
);

// pt^[i] reading REAL elements through a REF_TO ARRAY OF REAL.
#[test]
fn end_to_end_when_program_ref_to_array_deref_subscript_then_reads_element() {
    let source = "FUNCTION GET_ELEMENT : REAL VAR_INPUT pt : REF_TO ARRAY[0..100] OF REAL; i : INT; END_VAR GET_ELEMENT := pt^[i]; END_FUNCTION PROGRAM main VAR arr : ARRAY[0..100] OF REAL; r : REAL; END_VAR arr[0] := 42.0; r := GET_ELEMENT(pt := REF(arr), i := 0); END_PROGRAM";
    let (_c, bufs) = parse_and_run(source, &ed3());
    assert_eq!(bufs.vars[1].as_f32(), 42.0);
}

// b (var[1]) = 42 via deref+subscript; found (var[2]) = 1 since 42 > 0.
e2e_i32_with!(
    end_to_end_when_ref_to_byte_array_deref_subscript_in_comparison_then_correct,
    ed3(),
    "FUNCTION READ_ELEM : BYTE VAR_INPUT pt : REF_TO ARRAY[0..10] OF BYTE; i : INT; END_VAR READ_ELEM := pt^[i]; END_FUNCTION PROGRAM main VAR arr : ARRAY[0..10] OF BYTE; b : BYTE; found : BOOL; END_VAR arr[0] := BYTE#42; b := READ_ELEM(pt := REF(arr), i := 0); found := b > BYTE#0; END_PROGRAM",
    &[(1, 42), (2, 1)],
);

// PT^[0] inside a FUNCTION_BLOCK.
e2e_i32_with!(
    end_to_end_when_fb_with_deref_array_subscript_then_writes_through_ref,
    ed3(),
    "FUNCTION_BLOCK ARRAY_WRITER VAR_INPUT PT : REF_TO ARRAY[0..10] OF BYTE; END_VAR PT^[0] := BYTE#42; END_FUNCTION_BLOCK PROGRAM main VAR arr : ARRAY[0..10] OF BYTE; fb : ARRAY_WRITER; check : BYTE; END_VAR fb(PT := REF(arr)); check := arr[0]; END_PROGRAM",
    &[(2, 42)],
);

// Local REF_TO ARRAY in an FB: writes arr[0] = 99.
e2e_i32_with!(
    end_to_end_when_fb_with_local_ref_to_array_deref_subscript_then_writes_through_ref,
    ed3(),
    "FUNCTION_BLOCK COPY_FB VAR_INPUT src : REF_TO ARRAY[0..4] OF INT; END_VAR VAR local_pt : REF_TO ARRAY[0..4] OF INT; END_VAR local_pt := src; local_pt^[0] := 99; END_FUNCTION_BLOCK PROGRAM main VAR arr : ARRAY[0..4] OF INT; fb : COPY_FB; check : INT; END_VAR fb(src := REF(arr)); check := arr[0]; END_PROGRAM",
    &[(2, 99)],
);
