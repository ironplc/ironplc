//! End-to-end integration tests for the `ADR()` address-of operator.
//!
//! These tests exercise the full pipeline: parse → semantic analysis (where
//! `ADR(x)` is rewritten to the reference address-of expression) → codegen →
//! VM execution. `ADR` reuses the `REF_TO` backend, so no new opcodes are
//! involved — the pointer holds the addressed variable's table index and `^`
//! loads/stores indirectly.

use ironplc_dsl::core::FileId;
use ironplc_parser::options::{CompilerOptions, Dialect};
use ironplc_parser::parse_program;
use ironplc_problems::Problem;
use ironplc_vm::error::Trap;

use crate::common::{parse_and_run, parse_and_try_run};

/// The minimal flag set for `ADR`: the pointer type plus the operator,
/// deliberately without `allow_ref_to` (as in the `twincat` dialect).
fn adr_options() -> CompilerOptions {
    CompilerOptions {
        allow_pointer_to: true,
        allow_adr: true,
        ..CompilerOptions::default()
    }
}

/// The Goal example from `specs/design/adr-and-pointer-to.md`: an FB instance
/// binding a pointer to one of its own members and reading it back through
/// `^`. The member value is assigned
/// in the body because declared initial values are not yet applied to user
/// FB instance fields (a pre-existing gap unrelated to `ADR`).
///
/// var layout: point=0, then the FB body's slots pNumber=1, iNumber1=2,
/// iNumber2=3.
const GOAL_EXAMPLE: &str = "
FUNCTION_BLOCK FB_Point
VAR
   pNumber : POINTER TO INT;
   iNumber1 : INT;
   iNumber2 : INT;
END_VAR
iNumber1 := 5;
pNumber := ADR(iNumber1);
iNumber2 := pNumber^;
END_FUNCTION_BLOCK

PROGRAM main
VAR
    point : FB_Point;
END_VAR
    point();
END_PROGRAM
";

#[test]
fn end_to_end_when_adr_goal_example_then_deref_yields_value() {
    let (_c, bufs) = parse_and_run(GOAL_EXAMPLE, &adr_options());
    assert_eq!(bufs.vars[2].as_i32(), 5);
    assert_eq!(bufs.vars[3].as_i32(), 5);
}

// Store-through: writing `p^ := v` updates the addressed variable.
// var layout: x=0, p=1, v=2
e2e_i32_with!(
    end_to_end_when_adr_store_through_then_target_updated,
    adr_options(),
    "
PROGRAM main
VAR
    x : INT := 1;
    p : POINTER TO INT;
    v : INT := 99;
END_VAR
    p := ADR(x);
    p^ := v;
END_PROGRAM
",
    &[(0, 99)],
);

// Two instances of one FB: ADR inside each call addresses that call's own
// member value, so each instance's output reflects its own input.
// var layout: inst1=0, inst2=1, r1=2, r2=3, then the FB body's slots.
e2e_i32_with!(
    end_to_end_when_adr_in_two_fb_instances_then_each_addresses_own_member,
    adr_options(),
    "
FUNCTION_BLOCK FB_Echo
VAR_INPUT
    x : INT;
END_VAR
VAR
    held : INT;
    p : POINTER TO INT;
END_VAR
VAR_OUTPUT
    y : INT;
END_VAR
held := x;
p := ADR(held);
y := p^;
END_FUNCTION_BLOCK

PROGRAM main
VAR
    inst1 : FB_Echo;
    inst2 : FB_Echo;
    r1 : INT;
    r2 : INT;
END_VAR
    inst1(x := 7, y => r1);
    inst2(x := 9, y => r2);
END_PROGRAM
",
    &[(2, 7), (3, 9)],
);

// NULL guard: the guarded dereference only runs once the pointer is bound.
// NULL is the allow_ref_to keyword, as in the codesys dialect.
// var layout: x=0, p=1, y=2
e2e_i32_with!(
    end_to_end_when_adr_null_guard_then_deref_only_when_bound,
    CompilerOptions {
        allow_ref_to: true,
        ..adr_options()
    },
    "
PROGRAM main
VAR
    x : INT := 42;
    p : POINTER TO INT;
    y : INT;
END_VAR
    IF p <> NULL THEN
        y := 1;
    END_IF;
    p := ADR(x);
    IF p <> NULL THEN
        y := p^;
    END_IF;
END_PROGRAM
",
    &[(2, 42)],
);

#[test]
fn end_to_end_when_adr_unbound_pointer_deref_then_trap() {
    // An unbound POINTER TO defaults to NULL and traps on dereference.
    let source = "
PROGRAM main
VAR
    p : POINTER TO INT;
    y : INT;
END_VAR
    y := p^;
END_PROGRAM
";
    let err = parse_and_try_run(source, &adr_options()).unwrap_err();
    assert_eq!(err.trap, Trap::NullDereference);
}

#[test]
fn end_to_end_when_adr_flag_off_then_undeclared_function() {
    // Without allow_adr, ADR stays an ordinary identifier and the call is
    // reported as an undeclared function (P4017), like SIZEOF.
    let source = "
PROGRAM main
VAR
    x : INT;
    p : POINTER TO INT;
END_VAR
    p := ADR(x);
END_PROGRAM
";
    let options = CompilerOptions {
        allow_pointer_to: true,
        ..CompilerOptions::default()
    };
    let library = parse_program(source, &FileId::default(), &options).unwrap();
    let (_library, context) = ironplc_analyzer::stages::analyze(&[&library], &options).unwrap();
    let codes: Vec<&str> = context
        .diagnostics()
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert!(
        codes.contains(&Problem::FunctionCallUndeclared.code()),
        "expected P4017 (FunctionCallUndeclared), got {codes:?}"
    );
}

#[test]
fn end_to_end_when_twincat_dialect_then_goal_example_runs() {
    // The pure twincat preset enables the whole Goal example with no
    // explicit flags — and without any REF_TO/REF()/NULL keywords.
    let options = CompilerOptions::from_dialect(Dialect::TwinCat);
    let (_c, bufs) = parse_and_run(GOAL_EXAMPLE, &options);
    assert_eq!(bufs.vars[3].as_i32(), 5);
}

#[test]
fn end_to_end_when_codesys_dialect_then_goal_example_runs() {
    let options = CompilerOptions::from_dialect(Dialect::Codesys);
    let (_c, bufs) = parse_and_run(GOAL_EXAMPLE, &options);
    assert_eq!(bufs.vars[3].as_i32(), 5);
}
