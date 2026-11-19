//! Tests of lowering `SEL` and `MUX` of strings, arrays and structures, and
//! `MIN`, `MAX` and `LIMIT` of strings, stored at a place (`selection.rs`).
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

/// The comparison `op` of the strings `left` and `right`, as a comparison of
/// strings of `encoding` lowers (REQ-LOW-lowering-169).
fn compared(op: &str, encoding: &str, left: &str, right: &str) -> String {
    format!("({op} (call compare_{encoding} i32s {left} {right}) (const i32s 0))")
}

const STRINGS: &str = "PROGRAM main VAR a : STRING; b : STRING; c : STRING; s : STRING; END_VAR";
const A: &str = "(read string[254] v0^.f0)";
const B: &str = "(read string[254] v0^.f1)";
const C: &str = "(read string[254] v0^.f2)";

/// REQ-LOW-lowering-208: `MIN` of two strings is the `If` that stores the
/// first when it is at most the second, and the second otherwise. Each
/// condition is evaluated before an arm stores, so the target may be an
/// input.
#[test]
fn lower_statement_when_min_of_two_strings_then_if_storing_lesser_input() {
    let outcome = lower(
        &format!(
            "{STRINGS}
                s := MIN(a, b);
                a := MIN(b, a);
             END_PROGRAM"
        ),
        "main",
    );
    assert_eq!(
        outcome.statements,
        [
            lines(&[
                &format!("if {}", compared("le", "string", A, B)),
                &format!("  assign-str v0^.f3 {A}"),
                "else",
                &format!("  assign-str v0^.f3 {B}"),
                "end",
            ]),
            lines(&[
                &format!("if {}", compared("le", "string", B, A)),
                &format!("  assign-str v0^.f0 {B}"),
                "else",
                &format!("  assign-str v0^.f0 {A}"),
                "end",
            ]),
        ]
    );
}

/// REQ-LOW-lowering-208: `MAX` of three strings stores the first input that
/// is at least every later one, else the last: the `If` on the first being
/// at least the second and the third, whose `else` is the `If` on the second
/// being at least the third.
#[test]
fn lower_statement_when_max_of_three_strings_then_if_chain_over_later_inputs() {
    let outcome = lower(
        &format!(
            "{STRINGS}
                s := MAX(a, b, c);
             END_PROGRAM"
        ),
        "main",
    );
    let a_greatest = format!(
        "(bool_and bool {} {})",
        compared("ge", "string", A, B),
        compared("ge", "string", A, C)
    );
    assert_eq!(
        outcome.statements,
        [lines(&[
            &format!("if {a_greatest}"),
            &format!("  assign-str v0^.f3 {A}"),
            "else",
            &format!("  if {}", compared("ge", "string", B, C)),
            &format!("    assign-str v0^.f3 {B}"),
            "  else",
            &format!("    assign-str v0^.f3 {C}"),
            "  end",
            "end",
        ])]
    );
}

/// REQ-LOW-lowering-208: `LIMIT(MN, IN, MX)` of strings stores `MX` when it
/// is less than `IN` or than `MN`, else `MN` when `IN` is less than it, else
/// `IN`, as `MIN(MAX(IN, MN), MX)` selects.
#[test]
fn lower_statement_when_limit_of_strings_then_if_chain_storing_limited_input() {
    let outcome = lower(
        &format!(
            "{STRINGS}
                s := LIMIT(a, b, c);
             END_PROGRAM"
        ),
        "main",
    );
    let mx_below = format!(
        "(bool_or bool {} {})",
        compared("lt", "string", C, B),
        compared("lt", "string", C, A)
    );
    assert_eq!(
        outcome.statements,
        [lines(&[
            &format!("if {mx_below}"),
            &format!("  assign-str v0^.f3 {C}"),
            "else",
            &format!("  if {}", compared("lt", "string", B, A)),
            &format!("    assign-str v0^.f3 {A}"),
            "  else",
            &format!("    assign-str v0^.f3 {B}"),
            "  end",
            "end",
        ])]
    );
}

/// REQ-LOW-lowering-208: an input reached through a computed subscript is
/// stored in a temporary first, and the conditions compare, and the arms
/// store, the temporary; a literal is compared and stored as it is. Wide
/// strings are compared by the intrinsic of their encoding.
#[test]
fn lower_statement_when_min_of_wide_strings_with_computed_subscript_then_temporary_compared() {
    let outcome = lower(
        "PROGRAM main VAR i : DINT; names : ARRAY[1..2] OF WSTRING; w : WSTRING; END_VAR
            w := MIN(names[i], \"none\");
         END_PROGRAM",
        "main",
    );
    let statement = &outcome.statements[0];
    let stored: Vec<&str> = statement.lines().collect();
    assert_eq!(stored.len(), 6, "{statement}");
    assert!(
        stored[0].starts_with("assign-str v1 (read wstring[254] v0^.f1["),
        "{statement}"
    );
    let (temporary, none) = ("(read wstring[254] v1)", "(literal wstring[4] \"none\")");
    assert_eq!(
        stored[1..].join("\n"),
        lines(&[
            &format!("if {}", compared("le", "wstring", temporary, none)),
            &format!("  assign-str v0^.f2 {temporary}"),
            "else",
            &format!("  assign-str v0^.f2 {none}"),
            "end",
        ]),
    );
}

/// REQ-LOW-lowering-208: `MAX` of strings stored in a function block input
/// is lowered where the input is stored, before the call.
#[test]
fn lower_statement_when_max_of_strings_is_block_input_then_if_before_call() {
    let outcome = lower(
        "FUNCTION_BLOCK Keep VAR_INPUT v : STRING; END_VAR END_FUNCTION_BLOCK
         PROGRAM main VAR a : STRING; b : STRING; keep : Keep; END_VAR
            keep(v := MAX(a, b));
         END_PROGRAM",
        "main",
    );
    let statement = &outcome.statements[0];
    assert!(
        statement.starts_with(&lines(&[
            &format!("if {}", compared("ge", "string", A, B)),
            &format!("  assign-str v0^.f2.f0 {A}"),
            "else",
            &format!("  assign-str v0^.f2.f0 {B}"),
            "end",
            "fb-call v0^.f2",
        ])),
        "{statement}"
    );
}

/// `MIN`, `MAX` and `LIMIT` of strings inside an expression, rather than the
/// whole value stored, are not lowered yet, as a selection is not.
#[test]
fn lower_statement_when_min_or_max_of_strings_inside_expression_then_unsupported() {
    let outcome = lower(
        &format!(
            "{STRINGS}
                s := CONCAT(MIN(a, b), c);
                s := CONCAT(LIMIT(a, b, c), c);
             END_PROGRAM"
        ),
        "main",
    );
    assert_eq!(outcome.statements, ["unsupported"; 2]);
}
