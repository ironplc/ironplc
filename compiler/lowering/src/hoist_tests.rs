//! Tests of lowering calls with outputs ahead of the assignment whose value
//! they are in.
//!
//! Each test that checks a requirement of `specs/design/lowered-program.md`
//! names it.

use crate::test_support::lower;

/// A function and a method, each returning its input and with it as an
/// output.
const CALLEES: &str = "
    FUNCTION F : DINT
      VAR_INPUT x : DINT; END_VAR
      VAR_OUTPUT n : DINT; END_VAR
      n := x;
      F := x;
    END_FUNCTION
    FUNCTION_BLOCK K
      VAR count : DINT; END_VAR
      METHOD M : DINT
        VAR_INPUT x : DINT; END_VAR
        VAR_OUTPUT mn : DINT; END_VAR
        count := count + x;
        mn := count;
        M := x;
      END_METHOD
    END_FUNCTION_BLOCK";

/// Lowers each statement of `body` in a program with the callees, and the
/// variables `r`, `n1`, `n2` and `a` of type `DINT`, `b` and `c` of type
/// `BOOL`, `w` of type `DWORD`, `arr` of type `ARRAY[0..3] OF DINT` and the
/// instance `k` of `K`.
fn lower_main(body: &str) -> Vec<String> {
    lower(
        &format!(
            "{CALLEES}
             PROGRAM main
               VAR r : DINT; n1 : DINT; n2 : DINT; a : DINT; b : BOOL; c : BOOL;
                   w : DWORD; arr : ARRAY[0..3] OF DINT; k : K; END_VAR
               {body}
             END_PROGRAM"
        ),
        "main",
    )
    .statements
}

/// REQ-LOW-lowering-172, REQ-LOW-lowering-174: each call with outputs in an
/// assignment's value is lowered ahead of it, in the order the value
/// evaluates them, as the call stored in a temporary then the assignment of
/// its outputs, and the value reads each result from its temporary.
#[test]
fn lower_statement_when_two_calls_with_outputs_in_value_then_each_part_then_assignment() {
    let statements = lower_main("r := F(x := 3, n => n1) + F(x := 4, n => n2);");
    assert_eq!(
        statements,
        [[
            "assign v5 (call pou2 i32s (const i32s 3) (ref v4))",
            "assign v0^.f1 (read i32s v4)",
            "assign v7 (call pou2 i32s (const i32s 4) (ref v6))",
            "assign v0^.f2 (read i32s v6)",
            "assign v0^.f0 (add i32s (read i32s v5) (read i32s v7))",
        ]
        .join("\n")]
    );
}

/// REQ-LOW-lowering-174: a call nested in another's arguments has its part
/// first, and a value may go on after the results it reads first.
#[test]
fn lower_statement_when_call_nested_or_followed_then_lowered_in_order() {
    let statements = lower_main(
        "r := F(x := F(x := 1, n => n1), n => n2);
         r := F(x := 1, n => n1) + 1;
         r := -F(x := 1, n => n1);
         r := k.M(x := 1, mn => n1) + k.M(x := 2, mn => n2);",
    );
    assert_eq!(
        statements,
        [
            [
                "assign v5 (call pou2 i32s (const i32s 1) (ref v4))",
                "assign v0^.f1 (read i32s v4)",
                "assign v7 (call pou2 i32s (read i32s v5) (ref v6))",
                "assign v0^.f2 (read i32s v6)",
                "assign v0^.f0 (read i32s v7)",
            ]
            .join("\n"),
            [
                "assign v9 (call pou2 i32s (const i32s 1) (ref v8))",
                "assign v0^.f1 (read i32s v8)",
                "assign v0^.f0 (add i32s (read i32s v9) (const i32s 1))",
            ]
            .join("\n"),
            [
                "assign v11 (call pou2 i32s (const i32s 1) (ref v10))",
                "assign v0^.f1 (read i32s v10)",
                "assign v0^.f0 (neg i32s (read i32s v11))",
            ]
            .join("\n"),
            [
                "assign v17 (call pou3 i32s (ref v0^.f8) (const i32s 1) (ref v16))",
                "assign v0^.f1 (read i32s v16)",
                "assign v19 (call pou3 i32s (ref v0^.f8) (const i32s 2) (ref v18))",
                "assign v0^.f2 (read i32s v18)",
                "assign v0^.f0 (add i32s (read i32s v17) (read i32s v19))",
            ]
            .join("\n"),
        ]
    );
}

/// REQ-LOW-lowering-174: an assignment whose value runs something before a
/// call with outputs, such as a read, a constant, an operation or a
/// conversion of an earlier result, calls one only on a condition, or reads
/// its result anywhere but first, is not lowered. Nor is an assignment of
/// bits, which reads its place before its value on the route that reads the
/// AST, or a call with outputs in a condition.
#[test]
fn lower_statement_when_hoisting_would_change_order_then_unsupported() {
    let statements = lower_main(
        "r := a + F(x := 1, n => a);
         r := 1 + F(x := 1, n => n1);
         r := F(x := 1, n => n1) * 2 + F(x := 2, n => n2);
         r := F(x := 1, n => n1) / F(x := 2, n => n2) + F(x := 3, n => a);
         r := LINT_TO_DINT(DINT_TO_LINT(F(x := 1, n => n1)) + DINT_TO_LINT(F(x := 2, n => n2)));
         b := c AND_THEN F(x := 1, n => n1) > 0;
         b := F(x := 1, n => n1) > 0 OR_ELSE F(x := 2, n => n2) > 0;
         r := arr[F(x := 1, n => n1)];
         arr[F(x := 1, n => n1)] := 1;
         w.3 := F(x := 1, n => n1) > 0;
         IF F(x := 1, n => n1) > 0 THEN r := 1; END_IF;",
    );
    assert_eq!(statements, ["unsupported"; 11]);
}
