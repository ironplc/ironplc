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

/// `REF(v)` of a function's `VAR_IN_OUT` is a reference to what `v`
/// refers to, the caller's variable, not to the parameter.
#[test]
fn lower_statement_when_ref_of_in_out_then_ref_to_what_it_refers_to() {
    let outcome = lower_bodies(
        "FUNCTION F : BOOL VAR_IN_OUT v : DINT; END_VAR VAR p : REF_TO DINT; END_VAR
            p := REF(v);
            F := p <> NULL;
         END_FUNCTION
         PROGRAM main VAR x : DINT; END_VAR
            x := 1;
         END_PROGRAM",
        &["F"],
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0 (ref_to v1^)",
            "assign v2 (ne (read ref v0) (null))",
        ]
    );
}

/// A function with outputs, of several types.
const OUTPUTS: &str = "
FUNCTION F : INT
VAR_INPUT a : DINT; END_VAR
VAR_OUTPUT q : DINT; done : BOOL; r : REAL; END_VAR
  q := a + 1;
  done := q > 0;
  r := 1.5;
  F := 2;
END_FUNCTION";

/// REQ-LOW-lowering-092, REQ-LOW-lowering-172: a call of a function with
/// outputs passes a reference to a temporary of each output's type, which
/// the function's body writes through. The call's result is stored in a
/// temporary of its own, then each output is assigned from its temporary,
/// converted or negated as the analyzer recorded, then the assignment the
/// call is the value of is made from the result's temporary, converted too.
/// An output the call does not assign still has its temporary.
#[test]
fn lower_statement_when_function_with_outputs_called_then_call_outputs_then_result() {
    let outcome = lower_bodies(
        &format!(
            "{OUTPUTS}
             PROGRAM main VAR i : INT; d : DINT; b : BOOL; l : LREAL; END_VAR
               i := F(a := d, q => d, NOT done => b, r => l);
               l := F(a := 1);
             END_PROGRAM"
        ),
        &["main", "F"],
    );
    assert_eq!(
        outcome.statements,
        [
            [
                "assign v9 (truncate 16 (call pou1 i32s (read i32s v0^.f1) (ref v6) (ref v7) \
                 (ref v8)))",
                "assign v0^.f1 (read i32s v6)",
                "assign v0^.f2 (bool_not bool (read bool v7))",
                "assign v0^.f3 (convert f64 (read f32 v8))",
                "assign v0^.f0 (read i32s v9)",
            ]
            .join("\n"),
            [
                "assign v13 (truncate 16 (call pou1 i32s (const i32s 1) (ref v10) (ref v11) \
                 (ref v12)))",
                "assign v0^.f3 (convert f64 (read i32s v13))",
            ]
            .join("\n"),
            "assign v2^ (add i32s (read i32s v1) (const i32s 1))".to_string(),
            "assign v3^ (gt (read i32s v2^) (const i32s 0))".to_string(),
            "assign v4^ (const f32 1.5)".to_string(),
            "assign v5 (const i32s 2)".to_string(),
        ]
    );
}

/// REQ-LOW-lowering-092: a string output's temporary is a string, assigned
/// to its target as a string is.
#[test]
fn lower_statement_when_function_has_string_output_then_assigned_as_string() {
    let outcome = lower_bodies(
        "FUNCTION NAMED : BOOL VAR_OUTPUT s : STRING[8]; w : WSTRING; END_VAR
           s := 'abc';
           w := \"de\";
           NAMED := TRUE;
         END_FUNCTION
         PROGRAM main VAR ok : BOOL; t : STRING; u : WSTRING; END_VAR
           ok := NAMED(s => t, w => u);
         END_PROGRAM",
        &["main", "NAMED"],
    );
    assert_eq!(
        outcome.statements[0],
        [
            "assign v6 (call pou1 bool (ref v4) (ref v5))",
            "assign-str v0^.f1 (read string[8] v4)",
            "assign-str v0^.f2 (read wstring[254] v5)",
            "assign v0^.f0 (read bool v6)",
        ]
        .join("\n")
    );
    assert_eq!(
        outcome.statements[1],
        "assign-str v1^ (literal string[3] \"abc\")"
    );
}

/// A call of a function with outputs inside an `IF` is among the
/// statements of its branch.
#[test]
fn lower_statement_when_function_with_outputs_called_inside_if_then_among_its_statements() {
    let outcome = lower(
        &format!(
            "{OUTPUTS}
             PROGRAM main VAR i : INT; d : DINT; END_VAR
               IF d > 0 THEN i := F(a := d, q => d); END_IF;
             END_PROGRAM"
        ),
        "main",
    );
    assert_eq!(
        outcome.statements,
        ["if (gt (read i32s v0^.f1) (const i32s 0))\n\
          \x20 assign v9 (truncate 16 (call pou1 i32s (read i32s v0^.f1) (ref v6) (ref v7) \
          (ref v8)))\n\
          \x20 assign v0^.f1 (read i32s v6)\n\
          \x20 assign v0^.f0 (read i32s v9)\n\
          end"]
    );
}

/// A call of a function with outputs in a condition is not lowered yet.
#[test]
fn lower_statement_when_call_with_outputs_in_condition_then_unsupported() {
    let outcome = lower(
        &format!(
            "{OUTPUTS}
             PROGRAM main VAR d : DINT; END_VAR
               IF F(a := 1) > 0 THEN d := 1; END_IF;
             END_PROGRAM"
        ),
        "main",
    );
    assert_eq!(outcome.statements, ["unsupported"]);
}

/// REQ-LOW-lowering-071, REQ-LOW-lowering-072: a function's `STRING` and
/// `WSTRING` inputs are variables of their string types, held by value,
/// and a call passes each a `Str` of its encoding, of any capacity: a
/// variable or a literal. The body reads the variables the call passes.
#[test]
fn lower_statement_when_function_has_string_inputs_then_strs_of_their_encodings() {
    let outcome = lower_bodies(
        "FUNCTION SAME_LEN : BOOL VAR_INPUT s : STRING[4]; w : WSTRING[8]; END_VAR
           SAME_LEN := LEN(s) = LEN(w);
         END_FUNCTION
         PROGRAM main VAR ok : BOOL; t : STRING[10]; u : WSTRING; END_VAR
           ok := SAME_LEN(t, u);
           ok := SAME_LEN(w := \"x\", s := 'abcdef');
         END_PROGRAM",
        &["main", "SAME_LEN"],
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0 (call pou1 bool (read string[10] v0^.f1) (read wstring[254] v0^.f2))",
            "assign v0^.f0 (call pou1 bool (literal string[6] \"abcdef\") \
             (literal wstring[1] \"x\"))",
            "assign v3 (eq (call len_string i32s (read string[4] v1)) \
             (call len_wstring i32s (read wstring[8] v2)))",
        ]
    );
}

/// REQ-LOW-lowering-072, REQ-LOW-lowering-092: a string input of a call
/// with outputs is a `Str`, before the temporaries of its outputs.
#[test]
fn lower_statement_when_call_with_outputs_has_string_input_then_str_before_temporaries() {
    let outcome = lower(
        "FUNCTION PAD : DINT VAR_INPUT s : STRING; END_VAR VAR_OUTPUT n : DINT; END_VAR
           n := LEN(s);
           PAD := 1;
         END_FUNCTION
         PROGRAM main VAR r : DINT; k : DINT; END_VAR
           r := PAD('abc', n => k);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [[
            "assign v5 (call pou1 i32s (literal string[3] \"abc\") (ref v4))",
            "assign v0^.f1 (read i32s v4)",
            "assign v0^.f0 (read i32s v5)",
        ]
        .join("\n")]
    );
}

/// A `VAR_IN_OUT` of a string type is not lowered yet: the route that reads
/// the AST passes no string by reference.
#[test]
fn lower_statement_when_function_has_string_in_out_then_unsupported() {
    let outcome = lower(
        "FUNCTION SIZE : DINT VAR_IN_OUT s : STRING; END_VAR SIZE := LEN(s); END_FUNCTION
         PROGRAM main VAR r : DINT; t : STRING; END_VAR
           r := SIZE(t);
         END_PROGRAM",
        "main",
    );
    assert_eq!(outcome.statements, ["unsupported"]);
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
