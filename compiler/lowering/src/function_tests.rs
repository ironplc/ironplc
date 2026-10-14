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

/// A call of a function with a `VAR_OUTPUT`, a `VAR_IN_OUT` or a string
/// parameter is not lowered yet.
#[test]
fn lower_statement_when_function_has_output_in_out_or_string_then_unsupported() {
    let outcome = lower(
        "FUNCTION O : DINT VAR_INPUT a : DINT; END_VAR VAR_OUTPUT o : DINT; END_VAR
            O := a;
         END_FUNCTION
         FUNCTION IO : DINT VAR_IN_OUT a : DINT; END_VAR IO := a; END_FUNCTION
         FUNCTION S : DINT VAR_INPUT a : STRING; END_VAR S := 1; END_FUNCTION
         PROGRAM main VAR x : DINT; y : DINT; t : STRING; END_VAR
            x := O(a := 1);
            x := IO(y);
            x := S(t);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        ["unsupported", "unsupported", "unsupported"]
    );
}
