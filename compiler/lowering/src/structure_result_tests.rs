//! Tests of lowering calls of functions whose result is a structure.
//!
//! Each test that checks a requirement of `specs/design/lowered-program.md`
//! names it.

use crate::test_support::lower_bodies;

/// A structure, a function of an input and an output whose result is one,
/// and a function of no parameters whose result is one.
const FUNCTIONS: &str = "
TYPE P : STRUCT x : DINT; y : DINT; END_STRUCT; END_TYPE
FUNCTION MK : P
  VAR_INPUT v : DINT; END_VAR
  VAR_OUTPUT n : DINT; END_VAR
  MK.x := v;
  MK.y := MK.x * 2;
  n := v + 1;
END_FUNCTION
FUNCTION ORIGIN : P
  ORIGIN.x := 0;
END_FUNCTION";

/// REQ-LOW-lowering-079, REQ-LOW-lowering-186: a call of a function whose
/// result is a structure, assigned to a structure, is a `Call` statement
/// that passes a reference to a new temporary for the result parameter,
/// then the stores of its outputs, then the copy of the temporary to the
/// target. The function's body reaches its result through the result
/// parameter.
#[test]
fn lower_statement_when_structure_result_assigned_then_call_stores_and_copy() {
    let outcome = lower_bodies(
        &format!(
            "{FUNCTIONS}
             PROGRAM main VAR p : P; k : DINT; END_VAR
               p := MK(v := 3, n => k);
               p := ORIGIN();
             END_PROGRAM"
        ),
        &["main", "MK"],
    );
    assert_eq!(
        outcome.statements,
        [
            "call pou1 (ref v4) (const i32s 3) (ref v5)\n\
             assign v0^.f1 (read i32s v5)\n\
             copy v0^.f0 v4",
            "call pou2 (ref v7)\n\
             copy v0^.f0 v7",
            "assign v3^.f0 (read i32s v1)",
            "assign v3^.f1 (mul i32s (read i32s v3^.f0) (const i32s 2))",
            "assign v2^ (add i32s (read i32s v1) (const i32s 1))",
        ]
    );
}
