//! End-to-end tests for a *top-level* `ARRAY OF <struct>` variable — an array
//! whose element type is a user-defined structure and which is not reached
//! through an enclosing structure. See issue #1383 and
//! `specs/plans/2026-08-22-top-level-array-of-struct.md`.
//!
//! Field access through an array-of-struct *field* (`h.items[i].a`) is covered
//! by `end_to_end_struct.rs`; these tests own the case where the variable
//! itself is the array.

use crate::common::try_parse_and_compile;
use ironplc_parser::options::CompilerOptions;

// --- Nominal read and write ---

// arr is var 0, result is var 1.
e2e_i32!(
    end_to_end_when_top_level_array_of_struct_written_with_literal_index_then_reads_back,
    "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE PROGRAM main VAR arr : ARRAY[1..3] OF Item; result : DINT; END_VAR arr[2].b := 42; result := arr[2].b; END_PROGRAM",
    &[(1, 42)],
);

// The reported repro: a BOOL field written through a literal index. Exercises
// the narrow-width truncation path on store.
e2e_i32!(
    end_to_end_when_top_level_array_of_struct_bool_field_written_then_reads_back,
    "TYPE Item : STRUCT Flag : BOOL; END_STRUCT; END_TYPE PROGRAM main VAR Arr : ARRAY[1..6] OF Item; result : DINT; END_VAR Arr[1].Flag := TRUE; result := BOOL_TO_DINT(Arr[1].Flag); END_PROGRAM",
    &[(1, 1)],
);

// Writing one element must not disturb its neighbours -- this is what a wrong
// element stride would break.
e2e_i32!(
    end_to_end_when_top_level_array_of_struct_elements_written_then_each_element_distinct,
    "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE PROGRAM main VAR arr : ARRAY[1..3] OF Item; r1 : DINT; r2 : DINT; r3 : DINT; END_VAR arr[1].a := 11; arr[2].a := 22; arr[3].a := 33; r1 := arr[1].a; r2 := arr[2].a; r3 := arr[3].a; END_PROGRAM",
    &[(1, 11), (2, 22), (3, 33)],
);

// Distinct fields within one element must not alias -- this is what a wrong
// leaf offset would break.
e2e_i32!(
    end_to_end_when_top_level_array_of_struct_sibling_fields_written_then_do_not_alias,
    "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE PROGRAM main VAR arr : ARRAY[1..3] OF Item; ra : DINT; rb : DINT; END_VAR arr[2].a := 7; arr[2].b := 9; ra := arr[2].a; rb := arr[2].b; END_PROGRAM",
    &[(1, 7), (2, 9)],
);

// Variable subscript exercises the runtime flat-index path rather than the
// compile-time constant-folded one.
e2e_i32!(
    end_to_end_when_top_level_array_of_struct_indexed_by_variable_then_correct_element,
    "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE PROGRAM main VAR arr : ARRAY[1..3] OF Item; i : INT; result : DINT; END_VAR i := 3; arr[i].b := 55; result := arr[i].b; END_PROGRAM",
    &[(2, 55)],
);

// A FOR loop over the array, the shape the issue's users actually write.
e2e_i32!(
    end_to_end_when_top_level_array_of_struct_written_in_for_loop_then_all_elements_set,
    "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE PROGRAM main VAR arr : ARRAY[1..3] OF Item; i : INT; total : DINT; END_VAR FOR i := 1 TO 3 DO arr[i].a := i; END_FOR; total := arr[1].a + arr[2].a + arr[3].a; END_PROGRAM",
    &[(2, 6)],
);

// Several element reads combined in one expression.
e2e_i32!(
    end_to_end_when_top_level_array_of_struct_elements_summed_then_correct_total,
    "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE PROGRAM main VAR arr : ARRAY[1..3] OF Item; total : DINT; END_VAR arr[1].a := 1; arr[2].a := 2; arr[3].a := 3; total := arr[1].a + arr[2].a + arr[3].a; END_PROGRAM",
    &[(1, 6)],
);

// Unwritten elements read as zero: the data region starts zeroed and nothing
// else is allowed to land on top of the array.
e2e_i32!(
    end_to_end_when_top_level_array_of_struct_not_written_then_fields_read_zero,
    "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE PROGRAM main VAR arr : ARRAY[1..3] OF Item; r1 : DINT; r2 : DINT; END_VAR arr[1].a := 5; r1 := arr[2].a; r2 := arr[3].b; END_PROGRAM",
    &[(1, 0), (2, 0)],
);

// --- Bounds shapes ---

// A zero lower bound: the subtracted lower bound is 0 so the emitted index is
// the subscript itself.
e2e_i32!(
    end_to_end_when_top_level_array_of_struct_zero_based_then_correct_element,
    "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE PROGRAM main VAR arr : ARRAY[0..2] OF Item; r1 : DINT; r2 : DINT; END_VAR arr[0].a := 4; arr[2].a := 6; r1 := arr[0].a; r2 := arr[2].a; END_PROGRAM",
    &[(1, 4), (2, 6)],
);

// A negative lower bound must be subtracted, not ignored.
e2e_i32!(
    end_to_end_when_top_level_array_of_struct_negative_lower_bound_then_correct_element,
    "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE PROGRAM main VAR arr : ARRAY[-1..1] OF Item; i : INT; r1 : DINT; r2 : DINT; END_VAR i := 1; arr[-1].a := 8; arr[i].a := 9; r1 := arr[-1].a; r2 := arr[1].a; END_PROGRAM",
    &[(2, 8), (3, 9)],
);

// Two-dimensional: both strides must be scaled by the element slot count.
e2e_i32!(
    end_to_end_when_two_dimensional_top_level_array_of_struct_then_correct_element,
    "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE PROGRAM main VAR arr : ARRAY[1..2, 1..3] OF Item; r1 : DINT; r2 : DINT; END_VAR arr[1,1].a := 1; arr[2,3].a := 6; r1 := arr[1,1].a; r2 := arr[2,3].a; END_PROGRAM",
    &[(1, 1), (2, 6)],
);

// The descriptor spans `total_elements * element_slots`, so an out-of-range
// subscript still trips the VM's array bounds check rather than reading a
// neighbouring variable's region.
#[test]
fn end_to_end_when_top_level_array_of_struct_index_out_of_range_then_traps() {
    let source = "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE PROGRAM main VAR arr : ARRAY[1..3] OF Item; i : INT; r : DINT; END_VAR i := 9; r := arr[i].a; END_PROGRAM";
    let result = crate::common::parse_and_try_run(source, &CompilerOptions::default());
    assert!(
        result.is_err(),
        "expected a trap for an out-of-bounds element"
    );
}

// --- Element shapes ---

// A structure with a nested structure: the element stride is the *total* slot
// count, so a wrong count here shifts every element after the first.
e2e_i32!(
    end_to_end_when_top_level_array_of_nested_struct_then_element_stride_correct,
    "TYPE Inner : STRUCT x : DINT; y : DINT; END_STRUCT; END_TYPE TYPE Item : STRUCT lead : DINT; inner : Inner; END_STRUCT; END_TYPE PROGRAM main VAR arr : ARRAY[1..3] OF Item; r1 : DINT; r2 : DINT; END_VAR arr[1].lead := 1; arr[2].lead := 2; r1 := arr[1].lead; r2 := arr[2].lead; END_PROGRAM",
    &[(1, 1), (2, 2)],
);

// A LINT field is a 64-bit leaf, so the load and store must be emitted at
// W64 rather than the default width.
e2e_i64!(
    end_to_end_when_top_level_array_of_struct_lint_field_then_full_width_preserved,
    "TYPE Item : STRUCT big : LINT; small : DINT; END_STRUCT; END_TYPE PROGRAM main VAR arr : ARRAY[1..2] OF Item; result : LINT; END_VAR arr[2].big := 4294967296; result := arr[2].big; END_PROGRAM",
    &[(1, 4294967296)],
);

// --- Named array type ---

// `arr : Items` where `Items` is a named ARRAY OF <struct> type resolves
// through the type environment rather than an inline specification.
e2e_i32!(
    end_to_end_when_top_level_named_array_of_struct_type_then_reads_back,
    "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE TYPE Items : ARRAY[1..3] OF Item; END_TYPE PROGRAM main VAR arr : Items; result : DINT; END_VAR arr[3].a := 21; result := arr[3].a; END_PROGRAM",
    &[(1, 21)],
);

// --- Neighbouring variables ---

// A second array-of-struct and a plain array declared alongside must each get
// their own data region run.
e2e_i32!(
    end_to_end_when_two_top_level_arrays_of_struct_then_regions_do_not_overlap,
    "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE PROGRAM main VAR first : ARRAY[1..2] OF Item; second : ARRAY[1..2] OF Item; plain : ARRAY[1..2] OF DINT; r1 : DINT; r2 : DINT; r3 : DINT; END_VAR first[1].a := 1; second[1].a := 2; plain[1] := 3; r1 := first[1].a; r2 := second[1].a; r3 := plain[1]; END_PROGRAM",
    &[(3, 1), (4, 2), (5, 3)],
);

// --- Global declaration ---

// A global array-of-struct is registered before program locals and aliased
// into the program through VAR_EXTERNAL.
e2e_i32!(
    end_to_end_when_global_array_of_struct_then_external_can_read_and_write,
    "
TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE

CONFIGURATION config
  VAR_GLOBAL
    devices : ARRAY[1..3] OF Item;
  END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL
    devices : ARRAY[1..3] OF Item;
  END_VAR
  VAR
    result : DINT;
  END_VAR
  devices[1].a := 100;
  devices[3].b := 200;
  result := devices[1].a + devices[3].b;
END_PROGRAM
",
    &[(1, 300)],
);

// A function body sees the global through the re-inserted global metadata;
// without it the array resolves as a plain scalar and the field access fails.
// Functions reach globals by name (VAR_EXTERNAL is a POU-level construct the
// parser accepts only on programs and function blocks).
e2e_i32!(
    end_to_end_when_function_reads_global_array_of_struct_then_correct_value,
    "
TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE

CONFIGURATION config
  VAR_GLOBAL
    devices : ARRAY[1..3] OF Item;
  END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

FUNCTION second_a : DINT
  second_a := devices[2].a;
END_FUNCTION

PROGRAM main
  VAR_EXTERNAL
    devices : ARRAY[1..3] OF Item;
  END_VAR
  VAR
    result : DINT;
  END_VAR
  devices[2].a := 17;
  result := second_a();
END_PROGRAM
",
    &[(1, 17)],
);

// --- Rejected shapes ---
//
// Each of these is accepted by `ironplcc check` and rejected by codegen, so
// the assertion is on the compile result rather than on run values.

/// Compiles `source` with default options and asserts codegen rejects it
/// with the not-implemented problem code.
fn assert_codegen_rejects(source: &str, what: &str) {
    let result = try_parse_and_compile(source, &CompilerOptions::default());
    assert!(result.is_err(), "expected compilation to fail for {}", what);
    assert_eq!(result.unwrap_err().code, "P9999", "for {}", what);
}

#[test]
fn compile_when_top_level_array_of_struct_element_assigned_whole_then_not_implemented() {
    assert_codegen_rejects(
        "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE PROGRAM main VAR arr : ARRAY[1..3] OF Item; one : Item; END_VAR arr[1] := one; END_PROGRAM",
        "a whole-element assignment",
    );
}

#[test]
fn compile_when_top_level_array_of_struct_has_initial_values_then_not_implemented() {
    // Element fields are left zeroed, so an explicit initializer must be
    // rejected rather than silently dropped.
    assert_codegen_rejects(
        "TYPE Item : STRUCT a : DINT; END_STRUCT; END_TYPE PROGRAM main VAR arr : ARRAY[1..2] OF Item := [1, 2]; result : DINT; END_VAR result := arr[1].a; END_PROGRAM",
        "an array-of-struct with initial values",
    );
}

#[test]
fn compile_when_top_level_array_of_struct_string_field_read_then_not_implemented() {
    assert_codegen_rejects(
        "TYPE Item : STRUCT name : STRING[8]; END_STRUCT; END_TYPE PROGRAM main VAR arr : ARRAY[1..3] OF Item; result : STRING[8]; END_VAR result := arr[1].name; END_PROGRAM",
        "a STRING field of an element",
    );
}

#[test]
fn compile_when_top_level_array_of_struct_composite_field_read_then_not_implemented() {
    assert_codegen_rejects(
        "TYPE Inner : STRUCT x : DINT; END_STRUCT; END_TYPE TYPE Item : STRUCT inner : Inner; END_STRUCT; END_TYPE PROGRAM main VAR arr : ARRAY[1..3] OF Item; result : DINT; END_VAR result := arr[1].inner.x; END_PROGRAM",
        "a composite field of an element",
    );
}

#[test]
fn compile_when_top_level_array_of_struct_unknown_field_then_not_implemented() {
    assert_codegen_rejects(
        "TYPE Item : STRUCT a : DINT; b : DINT; END_STRUCT; END_TYPE PROGRAM main VAR arr : ARRAY[1..3] OF Item; result : DINT; END_VAR result := arr[1].missing; END_PROGRAM",
        "an unknown field of an element",
    );
}
