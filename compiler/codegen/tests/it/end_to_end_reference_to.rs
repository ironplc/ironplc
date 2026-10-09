//! End-to-end integration tests for TwinCAT `REFERENCE TO` reference types.
//!
//! These exercise the full pipeline — parse -> semantic analysis -> codegen ->
//! VM execution — for the TwinCAT surface syntax (`REFERENCE TO` declarations
//! and the `REF=` binding operator) accessed via the explicit `^` operator.
//! They prove the feature reuses the existing `REF_TO` backend unchanged.
//! See `specs/design/reference-to-twincat.md`.

use crate::common::parse_and_try_run;
use ironplc_parser::options::CompilerOptions;
use ironplc_vm::error::Trap;

fn reference_to_options() -> CompilerOptions {
    CompilerOptions {
        allow_reference_to: true,
        ..CompilerOptions::default()
    }
}

fn reference_to_method_options() -> CompilerOptions {
    CompilerOptions {
        allow_reference_to: true,
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    }
}

// vars: x=0, r=1, y=2
e2e_i32_with!(
    end_to_end_when_reference_to_bound_then_reads_through_caret,
    reference_to_options(),
    "
PROGRAM main
  VAR
    x : INT := 42;
    r : REFERENCE TO INT;
    y : INT;
  END_VAR
  r REF= x;
  y := r^;
END_PROGRAM
",
    &[("y", 42)],
);

e2e_i32_with!(
    end_to_end_when_reference_to_written_then_updates_referent,
    reference_to_options(),
    "
PROGRAM main
  VAR
    x : INT := 1;
    r : REFERENCE TO INT;
  END_VAR
  r REF= x;
  r^ := 99;
END_PROGRAM
",
    &[("x", 99)],
);

// vars: x=0, r=1, y=2
e2e_i32_with!(
    end_to_end_when_reference_to_named_type_then_runs,
    reference_to_options(),
    "
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
",
    &[("y", 7)],
);

// vars: val=0, refs=1, result=2
e2e_i32_with!(
    end_to_end_when_array_of_reference_to_element_bound_then_reads,
    reference_to_options(),
    "
PROGRAM main
  VAR
    val : INT := 77;
    refs : ARRAY[0..2] OF REFERENCE TO INT;
    result : INT;
  END_VAR
  refs[0] REF= val;
  result := refs[0]^;
END_PROGRAM
",
    &[("result", 77)],
);

// TwinCAT `REFERENCE TO` auto-dereferences: a bare read reads through it.
// vars: x=0, r=1, y=2
e2e_i32_with!(
    end_to_end_when_reference_to_read_without_caret_then_auto_dereferences,
    reference_to_options(),
    "
PROGRAM main
  VAR
    x : INT := 42;
    r : REFERENCE TO INT;
    y : INT;
  END_VAR
  r REF= x;
  y := r;
END_PROGRAM
",
    &[("y", 42)],
);

// A bare write stores through the reference to the referent.
e2e_i32_with!(
    end_to_end_when_reference_to_written_without_caret_then_auto_dereferences,
    reference_to_options(),
    "
PROGRAM main
  VAR
    x : INT := 1;
    r : REFERENCE TO INT;
  END_VAR
  r REF= x;
  r := 99;
END_PROGRAM
",
    &[("x", 99)],
);

// vars: x=0, r1=1, r2=2, y=3
e2e_i32_with!(
    end_to_end_when_two_references_alias_then_writes_are_observed,
    reference_to_options(),
    "
PROGRAM main
  VAR
    x : INT := 10;
    r1 : REFERENCE TO INT;
    r2 : REFERENCE TO INT;
    y : INT;
  END_VAR
  r1 REF= x;
  r2 REF= x;
  r1 := 55;
  y := r2;
END_PROGRAM
",
    &[("y", 55)],
);

// vars: x=0, r=1, before=2, after=3
// before: an unbound reference is not valid; after: a bound reference is valid
e2e_i32_with!(
    end_to_end_when_isvalidref_then_reflects_binding_state,
    reference_to_options(),
    "
PROGRAM main
  VAR
    x : INT := 5;
    r : REFERENCE TO INT;
    before : BOOL;
    after : BOOL;
  END_VAR
  before := __ISVALIDREF(r);
  r REF= x;
  after := __ISVALIDREF(r);
END_PROGRAM
",
    &[("before", 0), ("after", 1)],
);

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

// A `REFERENCE TO` declared in a method is read through its bare name, as one
// declared in the function block or program is.
e2e_i32_with!(
    end_to_end_when_reference_to_is_method_local_then_bare_read_reads_through_it,
    reference_to_method_options(),
    "
FUNCTION_BLOCK FB_A
VAR
    b : INT := 1;
END_VAR
METHOD M
VAR
    target : INT := 42;
    r : REFERENCE TO INT;
END_VAR
    r REF= target;
    b := r;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    a : FB_A;
    result : INT;
END_VAR
    a.M();
    result := a.b;
END_PROGRAM
",
    &[("result", 42)],
);
