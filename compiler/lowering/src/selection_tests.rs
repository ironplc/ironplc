//! Tests of lowering `SEL` and `MUX` of strings, arrays and structures,
//! stored at a place (`selection.rs`).
//!
//! Each test that checks a requirement of `specs/design/lowered-program.md`
//! names it.

use crate::test_support::lower;

/// One lowered statement printed on `lines`.
fn lines(lines: &[&str]) -> String {
    lines.join("\n")
}

const POINT: &str = "TYPE Point : STRUCT x : DINT; y : DINT; END_STRUCT; END_TYPE";

/// REQ-LOW-lowering-204: a selection of strings whose inputs are variables
/// is the `If` that stores the input its selector selects.
#[test]
fn lower_statement_when_sel_of_string_variables_then_if_storing_selected_input() {
    let outcome = lower(
        "PROGRAM main VAR g : BOOL; s : STRING; t : STRING; END_VAR
            s := SEL(g, s, t);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [lines(&[
            "if (read bool v0^.f0)",
            "  assign-str v0^.f1 (read string[254] v0^.f2)",
            "else",
            "  assign-str v0^.f1 (read string[254] v0^.f1)",
            "end",
        ])]
    );
}

/// REQ-LOW-lowering-204: a selection of structures by `MUX` is the `Case`
/// that copies the input its selector selects, the selector clamped: up to
/// zero the first input, and from the last input's position on the last.
#[test]
fn lower_statement_when_mux_of_structures_then_case_clamped_to_inputs() {
    let outcome = lower(
        &format!(
            "{POINT}
             PROGRAM main VAR k : DINT; p : Point; q : Point; r : Point; END_VAR
                p := MUX(k, p, q, r);
             END_PROGRAM"
        ),
        "main",
    );
    assert_eq!(
        outcome.statements,
        [lines(&[
            "case (read i32s v0^.f0)",
            "of -2147483648..0",
            "  copy v0^.f1 v0^.f1",
            "of 1",
            "  copy v0^.f1 v0^.f2",
            "else",
            "  copy v0^.f1 v0^.f3",
            "end",
        ])]
    );
}

/// REQ-LOW-lowering-204: when an input is reached through a computed
/// subscript, the selector and then each input are stored in temporaries,
/// in order, before the `If` stores the selected temporary; an input that is
/// a literal is stored from the literal.
#[test]
fn lower_statement_when_sel_input_has_computed_subscript_then_inputs_stored_in_order_first() {
    let outcome = lower(
        "PROGRAM main VAR g : BOOL; i : DINT; names : ARRAY[1..2] OF STRING; s : STRING;
             END_VAR
            s := SEL(g, names[i], 'none');
         END_PROGRAM",
        "main",
    );
    let statement = &outcome.statements[0];
    let stored: Vec<&str> = statement.lines().collect();
    assert_eq!(stored.len(), 7, "{statement}");
    assert!(
        stored[0].starts_with("assign v1 (read bool v0^.f0)"),
        "{statement}"
    );
    assert!(
        stored[1].starts_with("assign-str v2 (read string[254] v0^.f2["),
        "{statement}"
    );
    assert_eq!(
        stored[2..].join("\n"),
        lines(&[
            "if (read bool v1)",
            "  assign-str v0^.f3 (literal string[4] \"none\")",
            "else",
            "  assign-str v0^.f3 (read string[254] v2)",
            "end",
        ]),
    );
}

/// REQ-LOW-lowering-204: a constant selector is not stored in a temporary,
/// and a selection that is an input of another is stored in the outer
/// selection's temporary by its own `If`.
#[test]
fn lower_statement_when_nested_sel_with_constant_selector_then_inner_selection_stored_first() {
    let outcome = lower(
        "PROGRAM main VAR h : BOOL; s : STRING; t : STRING; END_VAR
            s := SEL(TRUE, SEL(h, s, t), t);
         END_PROGRAM",
        "main",
    );
    assert_eq!(
        outcome.statements,
        [lines(&[
            "if (read bool v0^.f0)",
            "  assign-str v1 (read string[254] v0^.f2)",
            "else",
            "  assign-str v1 (read string[254] v0^.f1)",
            "end",
            "assign-str v2 (read string[254] v0^.f2)",
            "if (const bool true)",
            "  assign-str v0^.f1 (read string[254] v2)",
            "else",
            "  assign-str v0^.f1 (read string[254] v1)",
            "end",
        ])]
    );
}

/// REQ-LOW-lowering-204: a selection stored in a function block input is
/// lowered where the input is stored, before the call.
#[test]
fn lower_statement_when_sel_of_structures_is_block_input_then_if_before_call() {
    let outcome = lower(
        &format!(
            "{POINT}
             FUNCTION_BLOCK Keep VAR_INPUT v : Point; END_VAR END_FUNCTION_BLOCK
             PROGRAM main VAR g : BOOL; p : Point; q : Point; keep : Keep; END_VAR
                keep(v := SEL(g, p, q));
             END_PROGRAM"
        ),
        "main",
    );
    let statement = &outcome.statements[0];
    assert!(
        statement.starts_with(&lines(&[
            "if (read bool v0^.f0)",
            "  copy v0^.f3.f0 v0^.f2",
            "else",
            "  copy v0^.f3.f0 v0^.f1",
            "end",
            "fb-call v0^.f3",
        ])),
        "{statement}"
    );
}

/// A selection of strings or aggregates inside an expression, rather than
/// the whole value stored, is not lowered yet.
#[test]
fn lower_statement_when_selection_of_string_or_aggregate_inside_expression_then_unsupported() {
    let outcome = lower(
        &format!(
            "{POINT}
             FUNCTION width : DINT VAR_INPUT v : Point; END_VAR width := v.x; END_FUNCTION
             PROGRAM main VAR g : BOOL; s : STRING; t : STRING; p : Point; q : Point; n : DINT;
                 x : BOOL; END_VAR
                s := CONCAT(SEL(g, s, t), t);
                n := width(SEL(g, p, q));
                x := SEL(g, s, t) = t;
             END_PROGRAM"
        ),
        "main",
    );
    assert_eq!(outcome.statements, ["unsupported"; 3]);
}
