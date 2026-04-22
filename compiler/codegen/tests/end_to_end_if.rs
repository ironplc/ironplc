//! End-to-end integration tests for IF/ELSIF/ELSE statements.

#[macro_use]
mod common;

use common::parse_and_run;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

// Simple IF (optionally with ELSE): `VAR x, y : DINT; x := <x>; IF x > 0 THEN y := 1; [ELSE y := 2;] END_IF;`
// y defaults to 0 when the THEN branch is skipped and no ELSE is present.
#[rstest]
#[case::if_true(5, false, 1)]
#[case::if_false(-5, false, 0)]
#[case::if_else_true(5, true, 1)]
#[case::if_else_false(-5, true, 2)]
fn end_to_end_if_simple(#[case] x: i32, #[case] has_else: bool, #[case] expected_y: i32) {
    let else_clause = if has_else { "ELSE y := 2;" } else { "" };
    let source = format!(
        "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := {x}; IF x > 0 THEN y := 1; {else_clause} END_IF; END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), x);
    assert_eq!(bufs.vars[1].as_i32(), expected_y);
}

// IF / ELSIF / ELSE chain: `VAR x, y : DINT; x := <x>; IF x > 5 THEN y := 1; ELSIF x > 0 THEN y := 2; ELSE y := 3; END_IF;`
#[rstest]
#[case::first_branch(10, 1)]
#[case::second_branch(3, 2)]
#[case::else_branch(-5, 3)]
fn end_to_end_if_elsif_else(#[case] x: i32, #[case] expected_y: i32) {
    let source = format!(
        "PROGRAM main VAR x : DINT; y : DINT; END_VAR x := {x}; IF x > 5 THEN y := 1; ELSIF x > 0 THEN y := 2; ELSE y := 3; END_IF; END_PROGRAM"
    );
    let (_c, bufs) = parse_and_run(&source, &CompilerOptions::default());
    assert_eq!(bufs.vars[0].as_i32(), x);
    assert_eq!(bufs.vars[1].as_i32(), expected_y);
}

// `IF <literal> > <var>`: exercises the literal-on-LHS compare codegen path.
e2e_i32!(
    end_to_end_when_if_literal_gt_var_true_then_executes_body,
    "PROGRAM main VAR n : DINT; y : DINT; END_VAR IF 2 > n THEN y := 1; END_IF; END_PROGRAM",
    &[(1, 1)],
);

e2e_i32!(
    end_to_end_when_if_literal_gt_var_false_then_skips_body,
    "PROGRAM main VAR n : DINT; y : DINT; END_VAR n := 5; IF 2 > n THEN y := 1; END_IF; END_PROGRAM",
    &[(0, 5), (1, 0)],
);

// Purely-literal IF condition: constant-folded expression on the LHS.
e2e_i32!(
    end_to_end_when_if_literal_expr_gt_literal_false_then_skips_body,
    "PROGRAM main VAR y : DINT; END_VAR IF 2 * 4 > 8 THEN y := 1; END_IF; END_PROGRAM",
    &[(0, 0)],
);
