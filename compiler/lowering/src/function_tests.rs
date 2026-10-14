//! Tests of lowering calls of user-defined functions.
//!
//! Each test that checks a requirement of `specs/design/lowered-program.md`
//! names it.

use crate::test_support::{lower, lower_bodies};

/// REQ-LOW-lowering-071, REQ-LOW-lowering-072: a call of a function passes
/// one value for each parameter, in the order the function declares them,
/// whatever order the call names them in. Its value is the function's result,
/// narrowed where it is stored. The function's body reads and writes the
/// variables the call passes and returns: `a` is `v1` and the result `v3`.
#[test]
fn lower_statement_when_function_called_then_call_of_its_pou() {
    let outcome = lower_bodies(
        "FUNCTION F : INT VAR_INPUT a : DINT; b : LREAL; END_VAR VAR t : DINT; END_VAR
            t := a;
            F := 1;
         END_FUNCTION
         PROGRAM main VAR r : INT; x : DINT; s : SINT; END_VAR
            r := F(b := 2.5, a := x);
            r := F(s, 1.0);
            x := F(x, 0.0) + 1;
         END_PROGRAM",
        &["main", "F"],
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0 (truncate 16 (call pou1 i32s (read i32s v0^.f1) (const f64 2.5)))",
            "assign v0^.f0 (truncate 16 (call pou1 i32s (read i32s v0^.f2) (const f64 1.0)))",
            "assign v0^.f1 (add i32s (call pou1 i32s (read i32s v0^.f1) (const f64 0.0)) \
             (const i32s 1))",
            "assign v4 (read i32s v1)",
            "assign v3 (const i32s 1)",
        ]
    );
}

/// REQ-LOW-lowering-033, REQ-LOW-lowering-075: a function's `VAR_IN_OUT` is
/// a non-null reference to the caller's variable. Its body reaches the
/// variable through it, and a call passes a reference to the place its
/// argument names: a variable of the caller, or what a `VAR_IN_OUT` of the
/// caller refers to, passed on.
#[test]
fn lower_statement_when_function_has_in_out_then_body_derefs_and_call_passes_ref() {
    let outcome = lower_bodies(
        "FUNCTION INC : DINT VAR_IN_OUT v : DINT; END_VAR VAR_INPUT n : DINT; END_VAR
            v := v + n;
            INC := v;
         END_FUNCTION
         FUNCTION TWICE : DINT VAR_IN_OUT w : DINT; END_VAR
            TWICE := INC(w, 1) + INC(w, 1);
         END_FUNCTION
         PROGRAM main VAR x : DINT; r : DINT; END_VAR
            r := INC(v := x, n := 2);
            r := TWICE(x);
         END_PROGRAM",
        &["main", "INC", "TWICE"],
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f1 (call pou1 i32s (ref v0^.f0) (const i32s 2))",
            "assign v0^.f1 (call pou2 i32s (ref v0^.f0))",
            "assign v1^ (add i32s (read i32s v1^) (read i32s v2))",
            "assign v3 (read i32s v1^)",
            "assign v5 (add i32s (call pou1 i32s (ref v4^) (const i32s 1)) \
             (call pou1 i32s (ref v4^) (const i32s 1)))",
        ]
    );
}

/// A call of a function with a `VAR_OUTPUT` or a string parameter is not
/// lowered yet.
#[test]
fn lower_statement_when_function_has_output_or_string_then_unsupported() {
    let outcome = lower(
        "FUNCTION O : DINT VAR_INPUT a : DINT; END_VAR VAR_OUTPUT o : DINT; END_VAR
            O := a;
         END_FUNCTION
         FUNCTION S : DINT VAR_INPUT a : STRING; END_VAR S := 1; END_FUNCTION
         PROGRAM main VAR x : DINT; t : STRING; END_VAR
            x := O(a := 1);
            x := S(t);
         END_PROGRAM",
        "main",
    );
    assert_eq!(outcome.statements, ["unsupported", "unsupported"]);
}

/// What a `VAR_IN_OUT` refers to is not a `FOR` loop's control variable yet,
/// but has bits written as any other place does.
#[test]
fn lower_statement_when_in_out_is_loop_control_then_unsupported() {
    let outcome = lower(
        "FUNCTION FILL : DINT VAR_IN_OUT n : DINT; w : WORD; END_VAR
           FOR n := 1 TO 3 DO
             FILL := FILL + n;
           END_FOR;
           w.0 := TRUE;
         END_FUNCTION",
        "FILL",
    );
    assert_eq!(
        outcome.statements,
        ["unsupported", "assign-bits v1^ 0 1 (const bool true)"]
    );
}
