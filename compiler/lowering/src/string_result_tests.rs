//! Tests of lowering calls of functions whose result is a string.
//!
//! Each test that checks a requirement of `specs/design/lowered-program.md`
//! names it.

use crate::test_support::{lower, lower_bodies};

/// Functions whose results are strings: one of a `STRING[20]` input,
/// doubled, and one of a `DINT`, that many `x`s up to three; a `DINT`
/// function to call beside them; and a block with a method of a string
/// input and one whose result is a string.
const FUNCTIONS: &str = "
FUNCTION TWICE : STRING[20]
  VAR_INPUT text : STRING[20]; END_VAR
  TWICE := CONCAT(text, text);
END_FUNCTION
FUNCTION XS : STRING[3]
  VAR_INPUT n : DINT; END_VAR
  XS := LEFT('xxx', n);
  XS := CONCAT(XS, '');
END_FUNCTION
FUNCTION ONE : DINT
  ONE := 1;
END_FUNCTION
FUNCTION BACK : STRING[2]
  VAR_INPUT x : DINT; END_VAR
  VAR_OUTPUT n : DINT; END_VAR
  n := x;
  BACK := 'ab';
END_FUNCTION
FUNCTION_BLOCK Named
  VAR_OUTPUT last : DINT; END_VAR
  METHOD Keep
    VAR_INPUT v : STRING[4]; END_VAR
    last := LEN(v);
  END_METHOD
  METHOD Name : STRING[5]
    Name := 'n';
  END_METHOD
END_FUNCTION_BLOCK";

/// Lowers `body`, the statements of a program of the variables `s`, a
/// `STRING[20]`, `n` and `k`, `DINT`s, `b`, a `BOOL`, and `f`, a `Named`.
fn main(body: &str) -> Vec<String> {
    lower(
        &format!(
            "{FUNCTIONS}
             PROGRAM main
               VAR s : STRING[20]; n : DINT; k : DINT; b : BOOL; f : Named; END_VAR
               {body}
             END_PROGRAM"
        ),
        "main",
    )
    .statements
}

/// REQ-LOW-lowering-079, REQ-LOW-lowering-179: a call of a function whose
/// result is a string is a string in-place call that passes a reference to
/// a new temporary of the result's type for the result parameter, and is
/// the string the function writes there. The function's body reaches its
/// result through the result parameter.
#[test]
fn lower_statement_when_string_result_assigned_then_in_place_call_assigned() {
    let outcome = lower_bodies(
        &format!(
            "{FUNCTIONS}
             PROGRAM main VAR s : STRING[20]; END_VAR
               s := TWICE(s);
             END_PROGRAM"
        ),
        &["main", "TWICE", "XS"],
    );
    assert_eq!(
        outcome.statements,
        [
            "assign-str v0^.f0 (call-in-place pou1 string[20] (ref v3) (read string[20] v0^.f0))",
            "assign-str v2^ (call concat_string string[40] (read string[20] v1) \
             (read string[20] v1))",
            "assign-str v4^ (call left_string string[3] (literal string[3] \"xxx\") \
             (read i32s v5))",
            "assign-str v4^ (call concat_string string[3] (read string[3] v4^) \
             (literal string[0] \"\"))",
        ]
    );
}

/// REQ-LOW-lowering-179: a call whose value is an operand of a string
/// operation, or an argument of another call, is in place where the
/// operation or the call uses it.
#[test]
fn lower_statement_when_string_result_is_operand_then_in_place_where_used() {
    assert_eq!(
        main(
            "n := LEN(XS(2));
             s := TWICE(TWICE('x'));
             s := CONCAT(XS(1), XS(k));
             n := 1 + LEN(XS(k));"
        ),
        [
            "assign v0^.f1 (call len_string i32s (call-in-place pou2 string[3] (ref v3) \
             (const i32s 2)))",
            "assign-str v0^.f0 (call-in-place pou3 string[20] (ref v6) \
             (call-in-place pou3 string[20] (ref v7) (literal string[1] \"x\")))",
            "assign-str v0^.f0 (call concat_string string[6] \
             (call-in-place pou2 string[3] (ref v8) (const i32s 1)) \
             (call-in-place pou2 string[3] (ref v9) (read i32s v0^.f2)))",
            "assign v0^.f1 (add i32s (const i32s 1) (call len_string i32s \
             (call-in-place pou2 string[3] (ref v10) (read i32s v0^.f2))))",
        ]
    );
}

/// REQ-LOW-lowering-179: a call in the condition of an `IF`, in the `from`
/// of a `FOR`, in a statement of a branch, or in the argument of a method
/// call statement is in place there.
#[test]
fn lower_statement_when_string_result_in_condition_or_branch_then_in_place_there() {
    assert_eq!(
        main(
            "IF XS(n) = 'xx' THEN s := XS(1); END_IF;
             FOR n := LEN(XS(3)) TO 5 DO k := k + 1; END_FOR;
             f.Keep(v := XS(2));"
        ),
        [
            "if (eq (call compare_string i32s (call-in-place pou2 string[3] (ref v3) \
             (read i32s v0^.f1)) (literal string[2] \"xx\")) (const i32s 0))\n\
             \x20 assign-str v0^.f0 (call-in-place pou2 string[3] (ref v4) (const i32s 1))\n\
             end",
            "for loop0 v0^.f1 from (call len_string i32s (call-in-place pou2 string[3] (ref v5) \
             (const i32s 3))) to (const i32s 5) by 1\n\
             \x20 assign v0^.f2 (add i32s (read i32s v0^.f2) (const i32s 1))\n\
             end",
            "call pou3 (ref v0^.f4) (call-in-place pou2 string[3] (ref v8) (const i32s 2))",
        ]
    );
}

/// REQ-LOW-lowering-180: a call is in place wherever the source has it: after
/// a read, a call or an operation the statement evaluates first, in an
/// `ELSIF`, `WHILE` or `REPEAT` condition, the `TO` of a `FOR`, a `CASE`
/// selector, a bit write and the right of `AND_THEN` or `OR_ELSE`.
#[test]
fn lower_statement_when_string_result_anywhere_then_in_place_where_the_source_has_it() {
    assert_eq!(
        main(
            "n := k + LEN(XS(1));
             n := ONE() + LEN(XS(1));
             n := LEN(XS(1)) + LEN(XS(2));
             IF n > 0 THEN k := 1; ELSIF XS(n) = 'x' THEN k := 2; END_IF;
             WHILE XS(n) = 'x' DO n := n + 1; END_WHILE;
             REPEAT n := n + 1; UNTIL XS(n) = 'xx' END_REPEAT;
             FOR n := 1 TO LEN(XS(3)) DO k := k + 1; END_FOR;
             CASE LEN(XS(n)) OF 1: k := 1; END_CASE;
             k.0 := XS(n) = 'x';
             b := n > 0 AND_THEN XS(n) = 'x';
             b := n > 0 OR_ELSE XS(n) = 'x';"
        ),
        [
            "assign v0^.f1 (add i32s (read i32s v0^.f2) (call len_string i32s \
             (call-in-place pou2 string[3] (ref v3) (const i32s 1))))",
            "assign v0^.f1 (add i32s (call pou3 i32s) (call len_string i32s \
             (call-in-place pou2 string[3] (ref v5) (const i32s 1))))",
            "assign v0^.f1 (add i32s (call len_string i32s (call-in-place pou2 string[3] (ref v6) \
             (const i32s 1))) (call len_string i32s (call-in-place pou2 string[3] (ref v7) \
             (const i32s 2))))",
            "if (gt (read i32s v0^.f1) (const i32s 0))\n\
             \x20 assign v0^.f2 (const i32s 1)\n\
             else\n\
             \x20 if (eq (call compare_string i32s (call-in-place pou2 string[3] (ref v8) \
             (read i32s v0^.f1)) (literal string[1] \"x\")) (const i32s 0))\n\
             \x20   assign v0^.f2 (const i32s 2)\n\
             \x20 end\n\
             end",
            "loop loop0\n\
             \x20 if (eq (call compare_string i32s (call-in-place pou2 string[3] (ref v9) \
             (read i32s v0^.f1)) (literal string[1] \"x\")) (const i32s 0))\n\
             \x20 else\n\
             \x20   exit loop0\n\
             \x20 end\n\
             \x20 assign v0^.f1 (add i32s (read i32s v0^.f1) (const i32s 1))\n\
             end",
            "loop loop1\n\
             \x20 assign v0^.f1 (add i32s (read i32s v0^.f1) (const i32s 1))\n\
             continuing\n\
             \x20 if (eq (call compare_string i32s (call-in-place pou2 string[3] (ref v10) \
             (read i32s v0^.f1)) (literal string[2] \"xx\")) (const i32s 0))\n\
             \x20   exit loop1\n\
             \x20 end\n\
             end",
            "for loop2 v0^.f1 from (const i32s 1) to (call len_string i32s \
             (call-in-place pou2 string[3] (ref v11) (const i32s 3))) by 1\n\
             \x20 assign v0^.f2 (add i32s (read i32s v0^.f2) (const i32s 1))\n\
             end",
            "case (call len_string i32s (call-in-place pou2 string[3] (ref v12) \
             (read i32s v0^.f1)))\n\
             of 1\n\
             \x20 assign v0^.f2 (const i32s 1)\n\
             end",
            "assign-bits v0^.f2 0 1 (eq (call compare_string i32s (call-in-place pou2 string[3] \
             (ref v13) (read i32s v0^.f1)) (literal string[1] \"x\")) (const i32s 0))",
            "assign v0^.f3 (and_then (gt (read i32s v0^.f1) (const i32s 0)) \
             (eq (call compare_string i32s (call-in-place pou2 string[3] (ref v14) \
             (read i32s v0^.f1)) (literal string[1] \"x\")) (const i32s 0)))",
            "assign v0^.f3 (or_else (gt (read i32s v0^.f1) (const i32s 0)) \
             (eq (call compare_string i32s (call-in-place pou2 string[3] (ref v15) \
             (read i32s v0^.f1)) (literal string[1] \"x\")) (const i32s 0)))",
        ]
    );
}

/// REQ-LOW-lowering-092, REQ-LOW-lowering-179: a call of a function with
/// outputs whose result is a string is in place with the stores of its
/// outputs.
#[test]
fn lower_statement_when_string_result_with_outputs_then_in_place_with_stores() {
    assert_eq!(
        main(
            "s := BACK(x := 1, n => k);
             n := LEN(BACK(x := 2, n => k));"
        ),
        [
            "assign-str v0^.f0 (call-in-place pou2 string[2] (ref v4) (const i32s 1) (ref v5) \
             {assign v0^.f2 (read i32s v5)})",
            "assign v0^.f1 (call len_string i32s (call-in-place pou2 string[2] (ref v6) \
             (const i32s 2) (ref v7) {assign v0^.f2 (read i32s v7)}))",
        ]
    );
}

/// A method whose result is a string is not lowered: the route that reads
/// the AST does not compile one.
#[test]
fn lower_statement_when_method_result_is_string_then_unsupported() {
    assert_eq!(main("s := f.Name();"), ["unsupported"]);
}
