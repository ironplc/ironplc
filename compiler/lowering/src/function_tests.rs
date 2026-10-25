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
/// the function's body writes through. The call is in place: after it, each
/// output is stored from its temporary, converted or negated as the
/// analyzer recorded, and the call's value is narrowed or converted where
/// the assignment stores it. An output the call does not assign still has
/// its temporary.
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
            "assign v0^.f0 (truncate 16 (call-in-place pou1 i32s (read i32s v0^.f1) (ref v6) \
             (ref v7) (ref v8) {assign v0^.f1 (read i32s v6)} \
             {assign v0^.f2 (bool_not bool (read bool v7))} \
             {assign v0^.f3 (convert f64 (read f32 v8))}))"
                .to_string(),
            "assign v0^.f3 (convert f64 (call-in-place pou1 i32s (const i32s 1) (ref v9) \
             (ref v10) (ref v11)))"
                .to_string(),
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
        "assign v0^.f0 (call-in-place pou1 bool (ref v4) (ref v5) \
         {assign-str v0^.f1 (read string[8] v4)} {assign-str v0^.f2 (read wstring[254] v5)})"
    );
    assert_eq!(
        outcome.statements[1],
        "assign-str v1^ (literal string[3] \"abc\")"
    );
}

/// A call of a function with outputs inside an `IF` is in place in the
/// statement of its branch.
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
          \x20 assign v0^.f0 (truncate 16 (call-in-place pou1 i32s (read i32s v0^.f1) (ref v6) \
          (ref v7) (ref v8) {assign v0^.f1 (read i32s v6)}))\n\
          end"]
    );
}

/// REQ-LOW-lowering-172: a call of a function with outputs in a condition
/// is in place in the condition, which runs it where the `IF` evaluates it.
#[test]
fn lower_statement_when_call_with_outputs_in_condition_then_in_place_in_condition() {
    let outcome = lower(
        &format!(
            "{OUTPUTS}
             PROGRAM main VAR d : DINT; END_VAR
               IF F(a := 1) > 0 THEN d := 1; END_IF;
             END_PROGRAM"
        ),
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "if (gt (call-in-place pou1 i32s (const i32s 1) (ref v6) (ref v7) (ref v8)) \
          (const i32s 0))\n\
          \x20 assign v0^.f0 (const i32s 1)\n\
          end"
        ]
    );
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
        [
            "assign v0^.f0 (call-in-place pou1 i32s (literal string[3] \"abc\") (ref v4) \
          {assign v0^.f1 (read i32s v4)})"
        ]
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

/// REQ-LOW-lowering-067: what a function's `VAR_OUTPUT` parameter refers to
/// is a `FOR` loop's control variable.
#[test]
fn lower_statement_when_output_is_loop_control_then_for() {
    let outcome = lower(
        "FUNCTION COUNT : DINT VAR_OUTPUT n : DINT; END_VAR
           FOR n := 1 TO 3 DO
             COUNT := COUNT + n;
           END_FOR;
         END_FUNCTION",
        "COUNT",
    );
    assert_eq!(
        outcome.statements,
        ["for loop0 v0^ from (const i32s 1) to (const i32s 3) by 1\n  assign v1 (add i32s (read i32s v1) (read i32s v0^))\nend"]
    );
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

/// REQ-LOW-lowering-075: an argument for a function's or method's input that
/// is a structure or an array is a `Copy` of the place its argument names,
/// and the input is a variable of the callee of the aggregate's type.
#[test]
fn lower_statement_when_aggregate_input_then_copy_of_argument() {
    let outcome = lower_bodies(
        "TYPE P : STRUCT x : DINT; END_STRUCT; END_TYPE
         TYPE A3 : ARRAY[1..3] OF DINT; END_TYPE
         FUNCTION SUMP : DINT VAR_INPUT q : P; n : DINT; END_VAR SUMP := n; END_FUNCTION
         FUNCTION SUMA : DINT VAR_INPUT q : A3; END_VAR SUMA := 8; END_FUNCTION
         FUNCTION_BLOCK BP METHOD M : DINT VAR_INPUT q : P; END_VAR M := 5; END_METHOD
         END_FUNCTION_BLOCK
         PROGRAM main VAR pt : P; a3 : A3; r : DINT; f : BP; END_VAR
            r := SUMP(q := pt, n := 2);
            r := SUMA(q := a3);
            r := f.M(q := pt);
         END_PROGRAM",
        &["main"],
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f2 (call pou2 i32s (copy v0^.f0) (const i32s 2))",
            "assign v0^.f2 (call pou3 i32s (copy v0^.f1))",
            "assign v0^.f2 (call pou4 i32s (ref v0^.f3) (copy v0^.f0))",
        ]
    );
}

/// REQ-LOW-lowering-072: a `VAR_IN_OUT` of a `DINT` takes a `Ref` to a
/// variable of a type declared as `DINT`, and one of that type takes a `Ref`
/// to a `DINT`.
#[test]
fn lower_statement_when_in_out_argument_type_is_alias_then_ref() {
    let outcome = lower_bodies(
        "TYPE MyInt : DINT; END_TYPE
         FUNCTION F : DINT VAR_IN_OUT x : DINT; END_VAR x := x + 1; F := x; END_FUNCTION
         FUNCTION G : DINT VAR_IN_OUT x : MyInt; END_VAR x := x + 1; G := x; END_FUNCTION
         PROGRAM main VAR a : MyInt := 4; d : DINT; r : DINT; END_VAR
           r := F(x := a);
           r := G(x := d);
         END_PROGRAM",
        &["main"],
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f2 (call pou1 i32s (ref v0^.f0))",
            "assign v0^.f2 (call pou2 i32s (ref v0^.f1))",
        ]
    );
}

/// REQ-LOW-lowering-075: an argument for a method's input of a function
/// block type, user-defined or standard, is a `Copy` of the instance its
/// argument names, or of the field of another instance it names.
#[test]
fn lower_statement_when_instance_input_then_copy_of_argument() {
    let outcome = lower_bodies(
        "FUNCTION_BLOCK INNER VAR_INPUT v : DINT; END_VAR END_FUNCTION_BLOCK
         FUNCTION_BLOCK OUTER VAR_INPUT inner : INNER; END_VAR END_FUNCTION_BLOCK
         FUNCTION_BLOCK BP
           METHOD Go : DINT VAR_INPUT i : INNER; END_VAR Go := 2; END_METHOD
           METHOD Tm : DINT VAR_INPUT t : TON; END_VAR Tm := 3; END_METHOD
         END_FUNCTION_BLOCK
         PROGRAM main VAR f : INNER; g : OUTER; b : BP; t : TON; r : DINT; END_VAR
            r := b.Go(i := f);
            r := b.Go(i := g.inner);
            r := b.Tm(t := t);
         END_PROGRAM",
        &["main"],
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f4 (call pou5 i32s (ref v0^.f2) (copy v0^.f0))",
            "assign v0^.f4 (call pou5 i32s (ref v0^.f2) (copy v0^.f1.f0))",
            "assign v0^.f4 (call pou6 i32s (ref v0^.f2) (copy v0^.f3))",
        ]
    );
}
