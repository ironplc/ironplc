//! `S=` / `R=` set/reset assignment operators (TwinCAT/CODESYS).
//!
//! Unlike `REF=` (see `reference_to.rs`), these are not gated by any
//! dialect flag — see the plan discussion at the time this was added:
//! `REF=` itself has no flag check either, and `S`/`R` are too common as
//! ordinary variable names to safely turn into demoted keywords.

use super::common::*;

#[test]
fn parse_when_set_bind_then_ok() {
    let lib = parse_text(
        "PROGRAM main
VAR
    bOut : BOOL;
    bCondition : BOOL;
END_VAR
    bOut S= bCondition;
END_PROGRAM",
    );
    let assignment = cast!(only_statement(&lib), StmtKind::Assignment);
    assert!(assignment.set_bind);
    assert!(!assignment.reset_bind);
    assert!(!assignment.ref_bind);
}

#[test]
fn parse_when_reset_bind_then_ok() {
    let lib = parse_text(
        "PROGRAM main
VAR
    bOut : BOOL;
    bCondition : BOOL;
END_VAR
    bOut R= bCondition;
END_PROGRAM",
    );
    let assignment = cast!(only_statement(&lib), StmtKind::Assignment);
    assert!(assignment.reset_bind);
    assert!(!assignment.set_bind);
    assert!(!assignment.ref_bind);
}

#[test]
fn parse_when_lowercase_set_bind_then_ok() {
    let lib = parse_text(
        "PROGRAM main
VAR
    bOut : BOOL;
END_VAR
    bOut s= TRUE;
END_PROGRAM",
    );
    let assignment = cast!(only_statement(&lib), StmtKind::Assignment);
    assert!(assignment.set_bind);
}

/// A variable *named* `S` assigned with `:=` is a normal assignment, not a
/// set-bind — the rule only fires on `S` immediately followed by `=`.
#[test]
fn parse_when_variable_named_s_assigned_then_normal_assignment() {
    let lib = parse_text(
        "PROGRAM main
VAR
    S : INT;
END_VAR
    S := 5;
END_PROGRAM",
    );
    let assignment = cast!(only_statement(&lib), StmtKind::Assignment);
    assert!(!assignment.set_bind);
    assert_eq!(assignment.target.to_string(), "S");
}

/// A variable named `S` used as an ordinary expression operand elsewhere is
/// unaffected.
#[test]
fn parse_when_variable_named_s_used_in_expression_then_normal() {
    let lib = parse_text(
        "PROGRAM main
VAR
    S : INT;
    total : INT;
END_VAR
    total := S + 1;
END_PROGRAM",
    );
    let assignment = cast!(only_statement(&lib), StmtKind::Assignment);
    assert!(!assignment.set_bind);
}

/// A space between `S` and `=` is not the operator: `S = x` is a syntax
/// error in statement position (assignment requires `:=`), not a set-bind.
#[test]
fn parse_when_space_between_s_and_equals_then_error() {
    let source = "PROGRAM main
VAR
    x : BOOL;
    y : BOOL;
END_VAR
    x S = y;
END_PROGRAM";
    let result = parse_program(source, &FileId::default(), &CompilerOptions::default());
    assert!(result.is_err(), "S = with a space must be rejected");
}

/// `=` equality comparison in a condition still parses (the set-bind rule
/// only fires on `S` immediately followed by `=`, and only in
/// assignment-statement position).
#[test]
fn parse_when_equality_in_condition_then_parses_as_comparison() {
    let lib = parse_text(
        "PROGRAM main
VAR
    a : INT;
    b : INT;
    c : INT;
END_VAR
    IF a = b THEN
        c := 1;
    END_IF;
END_PROGRAM",
    );
    cast!(only_statement(&lib), StmtKind::If);
}

#[test]
fn parse_when_set_and_reset_bind_mixed_with_plain_assignment_then_ok() {
    let lib = parse_text(
        "PROGRAM main
VAR
    bRunning : BOOL;
    bStart : BOOL;
    bStop : BOOL;
    n : INT;
END_VAR
    bRunning S= bStart;
    bRunning R= bStop;
    n := 1;
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let stmts = cast!(&prog.body, FunctionBlockBodyKind::Statements);
    assert_eq!(stmts.body.len(), 3);
    let a0 = cast!(&stmts.body[0], StmtKind::Assignment);
    assert!(a0.set_bind);
    let a1 = cast!(&stmts.body[1], StmtKind::Assignment);
    assert!(a1.reset_bind);
    let a2 = cast!(&stmts.body[2], StmtKind::Assignment);
    assert!(!a2.set_bind && !a2.reset_bind && !a2.ref_bind);
}
