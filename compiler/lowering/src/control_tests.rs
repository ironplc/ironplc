//! Tests of lowering control flow.
//!
//! Each test that checks a requirement of `specs/design/lowered-program.md`
//! names it.

use crate::test_support::lower;

/// One lowered statement printed on `lines`.
fn lines(lines: &[&str]) -> String {
    lines.join("\n")
}

/// `ELSIF` is an `If` nested in the `ELSE` of the one before.
#[test]
fn lower_statement_when_elsif_then_nested_if() {
    let outcome = lower(
        "PROGRAM main VAR a : DINT; END_VAR
            IF a > 1 THEN a := 1; ELSIF a > 0 THEN a := 2; ELSE a := 3; END_IF;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [lines(&[
            "if (gt (read i32s v0^.f0) (const i32s 1))",
            "  assign v0^.f0 (const i32s 1)",
            "else",
            "  if (gt (read i32s v0^.f0) (const i32s 0))",
            "    assign v0^.f0 (const i32s 2)",
            "  else",
            "    assign v0^.f0 (const i32s 3)",
            "  end",
            "end",
        ])]
    );
}

/// REQ-LOW-lowering-064: the labels of a `Case` are constants and ranges of
/// the selector's type; an unsigned label keeps its value.
#[test]
fn lower_statement_when_case_then_labels_of_selector_type() {
    let outcome = lower(
        "PROGRAM main VAR a : DINT; u : UDINT; END_VAR
            CASE a OF
              1, 3..5: a := 10;
              -2: a := 20;
            ELSE
              a := 30;
            END_CASE;
            CASE u OF 4294967295: u := 1; END_CASE;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            lines(&[
                "case (read i32s v0^.f0)",
                "of 1, 3..5",
                "  assign v0^.f0 (const i32s 10)",
                "of -2",
                "  assign v0^.f0 (const i32s 20)",
                "else",
                "  assign v0^.f0 (const i32s 30)",
                "end",
            ]),
            lines(&[
                "case (read i32u v0^.f1)",
                "of 4294967295",
                "  assign v0^.f1 (const i32u 1)",
                "end",
            ]),
        ]
    );
}

/// An enumerated value is a constant, its ordinal as the analyzer decided
/// it, and so is a `CASE` label naming one.
#[test]
fn lower_statement_when_enumerated_value_then_ordinal_constant() {
    let outcome = lower(
        "TYPE COLOR : (RED, GREEN := 5, BLUE); END_TYPE
         PROGRAM main VAR c : COLOR; END_VAR
            c := BLUE;
            CASE c OF GREEN: c := RED; END_CASE;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            "assign v0^.f0 (const i32s 6)".to_string(),
            lines(&[
                "case (read i32s v0^.f0)",
                "of 5",
                "  assign v0^.f0 (const i32s 0)",
                "end",
            ]),
        ]
    );
}

/// REQ-LOW-lowering-059: `WHILE` and `REPEAT` are each a `Loop`, leaving at
/// its head and at its `continuing`.
#[test]
fn lower_statement_when_while_and_repeat_then_loops() {
    let outcome = lower(
        "PROGRAM main VAR a : DINT; END_VAR
            WHILE a < 10 DO a := a + 1; END_WHILE;
            REPEAT a := a - 1; UNTIL a = 0 END_REPEAT;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            lines(&[
                "loop loop0",
                "  if (lt (read i32s v0^.f0) (const i32s 10))",
                "  else",
                "    exit loop0",
                "  end",
                "  assign v0^.f0 (add i32s (read i32s v0^.f0) (const i32s 1))",
                "end",
            ]),
            lines(&[
                "loop loop1",
                "  assign v0^.f0 (sub i32s (read i32s v0^.f0) (const i32s 1))",
                "continuing",
                "  if (eq (read i32s v0^.f0) (const i32s 0))",
                "    exit loop1",
                "  end",
                "end",
            ]),
        ]
    );
}

/// REQ-LOW-lowering-066, REQ-LOW-lowering-067: a `For` keeps its control,
/// bounds and constant step; the step is 1 when none is written, and the
/// first value is narrowed to the control's storage.
#[test]
fn lower_statement_when_for_then_bounds_and_constant_step() {
    let outcome = lower(
        "PROGRAM main VAR i : INT; n : INT; d : DINT; END_VAR
            FOR i := n + 1 TO 10 DO d := d + 1; END_FOR;
            FOR d := 10 TO 0 BY -2 DO i := 1; END_FOR;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            lines(&[
                "for loop0 v0^.f0 from (truncate 16 (add i32s (read i32s v0^.f1) (const i32s 1))) \
                 to (const i32s 10) by 1",
                "  assign v0^.f2 (add i32s (read i32s v0^.f2) (const i32s 1))",
                "end",
            ]),
            lines(&[
                "for loop1 v0^.f2 from (const i32s 10) to (const i32s 0) by -2",
                "  assign v0^.f0 (const i32s 1)",
                "end",
            ]),
        ]
    );
}

/// REQ-LOW-lowering-062: `EXIT` and `CONTINUE` name the innermost loop that
/// encloses them.
#[test]
fn lower_statement_when_exit_and_continue_then_name_innermost_loop() {
    let outcome = lower(
        "PROGRAM main VAR i : DINT; j : DINT; END_VAR
            FOR i := 1 TO 3 DO
              IF i = 2 THEN CONTINUE; END_IF;
              WHILE j < i DO
                EXIT;
              END_WHILE;
              EXIT;
            END_FOR;
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [lines(&[
            "for loop0 v0^.f0 from (const i32s 1) to (const i32s 3) by 1",
            "  if (eq (read i32s v0^.f0) (const i32s 2))",
            "    continue loop0",
            "  end",
            "  loop loop1",
            "    if (lt (read i32s v0^.f1) (read i32s v0^.f0))",
            "    else",
            "      exit loop1",
            "    end",
            "    exit loop1",
            "  end",
            "  exit loop0",
            "end",
        ])]
    );
}

#[test]
fn lower_statement_when_return_then_return() {
    let outcome = lower(
        "FUNCTION f : DINT VAR_INPUT a : DINT; END_VAR
            f := a;
            RETURN;
         END_FUNCTION",
        "f",
    );
    assert_eq!(outcome.statements, ["assign v0 (read i32s v1)", "return"]);
}

/// A function block's `VAR_IN_OUT` is a field its call binds, which lowering
/// does not express yet.
#[test]
fn lower_statement_when_block_var_in_out_then_unsupported() {
    let outcome = lower(
        "FUNCTION_BLOCK fb VAR_IN_OUT a : DINT; END_VAR
            a := 1;
         END_FUNCTION_BLOCK",
        "fb",
    );
    assert_eq!(outcome.statements, ["unsupported"]);
}
