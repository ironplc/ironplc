//! Tests of lowering calls with outputs in place, where the expression that
//! holds them evaluates them.
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
/// expression is in place, in the order the expression evaluates them: the
/// call, with a reference to a temporary for each output, then the store of
/// each output from its temporary.
#[test]
fn lower_statement_when_two_calls_with_outputs_in_value_then_each_in_place() {
    let statements = lower_main("r := F(x := 3, n => n1) + F(x := 4, n => n2);");
    assert_eq!(
        statements,
        ["assign v0^.f0 (add i32s \
          (call-in-place pou2 i32s (const i32s 3) (ref v4) {assign v0^.f1 (read i32s v4)}) \
          (call-in-place pou2 i32s (const i32s 4) (ref v5) {assign v0^.f2 (read i32s v5)}))"]
    );
}

/// REQ-LOW-lowering-174: a call nested in another's arguments is in place in
/// that argument, and a value goes on after the call.
#[test]
fn lower_statement_when_call_nested_or_followed_then_in_place_where_evaluated() {
    let statements = lower_main(
        "r := F(x := F(x := 1, n => n1), n => n2);
         r := F(x := 1, n => n1) + 1;
         r := -F(x := 1, n => n1);
         r := k.M(x := 1, mn => n1) + k.M(x := 2, mn => n2);",
    );
    assert_eq!(
        statements,
        [
            "assign v0^.f0 (call-in-place pou2 i32s \
             (call-in-place pou2 i32s (const i32s 1) (ref v4) {assign v0^.f1 (read i32s v4)}) \
             (ref v5) {assign v0^.f2 (read i32s v5)})",
            "assign v0^.f0 (add i32s (call-in-place pou2 i32s (const i32s 1) (ref v6) \
             {assign v0^.f1 (read i32s v6)}) (const i32s 1))",
            "assign v0^.f0 (neg i32s (call-in-place pou2 i32s (const i32s 1) (ref v7) \
             {assign v0^.f1 (read i32s v7)}))",
            "assign v0^.f0 (add i32s \
             (call-in-place pou3 i32s (ref v0^.f8) (const i32s 1) (ref v12) \
             {assign v0^.f1 (read i32s v12)}) \
             (call-in-place pou3 i32s (ref v0^.f8) (const i32s 2) (ref v13) \
             {assign v0^.f2 (read i32s v13)}))",
        ]
    );
}

/// REQ-LOW-lowering-174: a call with outputs is in place wherever the source
/// has it: after a read, a constant or an operation the value evaluates
/// first, the right of `AND_THEN` or `OR_ELSE`, a subscript, an `IF`
/// condition, and the value of a bit assignment, whose place the
/// assignment reads first.
#[test]
fn lower_statement_when_call_with_outputs_anywhere_then_in_place_where_the_source_has_it() {
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
    let f = |x: i32, temporary: u32, output: &str| {
        format!(
            "(call-in-place pou2 i32s (const i32s {x}) (ref v{temporary}) \
             {{assign v0^.{output} (read i32s v{temporary})}})"
        )
    };
    assert_eq!(
        statements,
        [
            format!(
                "assign v0^.f0 (add i32s (read i32s v0^.f3) {})",
                f(1, 4, "f3")
            ),
            format!("assign v0^.f0 (add i32s (const i32s 1) {})", f(1, 5, "f1")),
            format!(
                "assign v0^.f0 (add i32s (mul i32s {} (const i32s 2)) {})",
                f(1, 6, "f1"),
                f(2, 7, "f2")
            ),
            format!(
                "assign v0^.f0 (add i32s (div i32s {} {}) {})",
                f(1, 8, "f1"),
                f(2, 9, "f2"),
                f(3, 10, "f3")
            ),
            format!(
                "assign v0^.f0 (convert i32s (add i64s (convert i64s {}) (convert i64s {})))",
                f(1, 11, "f1"),
                f(2, 12, "f2")
            ),
            format!(
                "assign v0^.f4 (and_then (read bool v0^.f5) (gt {} (const i32s 0)))",
                f(1, 13, "f1")
            ),
            format!(
                "assign v0^.f4 (or_else (gt {} (const i32s 0)) (gt {} (const i32s 0)))",
                f(1, 14, "f1"),
                f(2, 15, "f2")
            ),
            format!("assign v0^.f0 (read i32s v0^.f7[{}])", f(1, 16, "f1")),
            format!("assign v0^.f7[{}] (const i32s 1)", f(1, 17, "f1")),
            format!(
                "assign-bits v0^.f6 3 1 (gt {} (const i32s 0))",
                f(1, 18, "f1")
            ),
            format!(
                "if (gt {} (const i32s 0))\n  assign v0^.f0 (const i32s 1)\nend",
                f(1, 19, "f1")
            ),
        ]
    );
}
