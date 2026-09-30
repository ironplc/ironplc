//! End-to-end tests for non-formal (positional) function block calls.
//!
//! A non-formal call, `inst(1, 2)`, binds its arguments to the block's
//! `VAR_INPUT` variables in declaration order. Each program is first run
//! through the full semantic analysis (`check_and_run`): a call the checker
//! refuses must not pass here.

use ironplc_parser::options::CompilerOptions;

use crate::common::{check_and_run, try_parse_and_compile};

fn assert_i32(source: &str, asserts: &[(usize, i32)]) {
    let bufs = check_and_run(source, &CompilerOptions::default());
    for (idx, expected) in asserts {
        assert_eq!(bufs.vars[*idx].as_i32(), *expected, "vars[{idx}] mismatch");
    }
}

// The example from issue #1855.
#[test]
fn end_to_end_when_user_fb_nonformal_call_then_inputs_bound() {
    assert_i32(
        "
FUNCTION_BLOCK f
  VAR_INPUT a, b : INT; END_VAR
  VAR_OUTPUT s : INT; END_VAR
  s := a + b;
END_FUNCTION_BLOCK

PROGRAM main
  VAR i : f; x : INT; END_VAR
  i(1, 2);
  x := i.s;
END_PROGRAM
",
        &[(1, 3)],
    );
}

// Subtraction does not commute, so a swapped binding shows. The inputs are
// declared in two VAR_INPUT sections around the output: declaration order,
// not section, decides the position.
#[test]
fn end_to_end_when_inputs_declared_in_two_sections_then_bound_in_declaration_order() {
    assert_i32(
        "
FUNCTION_BLOCK diff
  VAR_INPUT minuend : DINT; END_VAR
  VAR_OUTPUT d : DINT; END_VAR
  VAR_INPUT subtrahend : DINT; END_VAR
  d := minuend - subtrahend;
END_FUNCTION_BLOCK

PROGRAM main
  VAR i : diff; x : DINT; END_VAR
  i(10, 3);
  x := i.d;
END_PROGRAM
",
        &[(1, 7)],
    );
}

#[test]
fn end_to_end_when_nonformal_arguments_are_expressions_then_evaluated_and_bound() {
    assert_i32(
        "
FUNCTION_BLOCK diff
  VAR_INPUT minuend, subtrahend : DINT; END_VAR
  VAR_OUTPUT d : DINT; END_VAR
  d := minuend - subtrahend;
END_FUNCTION_BLOCK

PROGRAM main
  VAR i : diff; y : DINT := 4; x : DINT; END_VAR
  i(y * 10, y + 1);
  x := i.d;
END_PROGRAM
",
        &[(2, 35)],
    );
}

#[test]
fn end_to_end_when_nonformal_call_with_output_assignment_then_output_stored() {
    assert_i32(
        "
FUNCTION_BLOCK diff
  VAR_INPUT minuend, subtrahend : DINT; END_VAR
  VAR_OUTPUT d : DINT; END_VAR
  d := minuend - subtrahend;
END_FUNCTION_BLOCK

PROGRAM main
  VAR i : diff; x : DINT; END_VAR
  i(10, 3, d => x);
END_PROGRAM
",
        &[(1, 7)],
    );
}

// Each input is stored at its own type: a REAL and a DINT input side by side.
#[test]
fn end_to_end_when_nonformal_inputs_of_different_types_then_each_stored_at_its_type() {
    let bufs = check_and_run(
        "
FUNCTION_BLOCK scale
  VAR_INPUT factor : REAL; count : DINT; END_VAR
  VAR_OUTPUT y : REAL; END_VAR
  y := factor * DINT_TO_REAL(count);
END_FUNCTION_BLOCK

PROGRAM main
  VAR i : scale; x : REAL; END_VAR
  i(2.5, 7);
  x := i.y;
END_PROGRAM
",
        &CompilerOptions::default(),
    );
    assert_eq!(bufs.vars[1].as_f32(), 17.5);
}

// Arguments are stored on every call, so a second call with other values
// rebinds the inputs rather than keeping the first call's.
#[test]
fn end_to_end_when_nonformal_call_repeated_then_inputs_rebound() {
    assert_i32(
        "
FUNCTION_BLOCK diff
  VAR_INPUT minuend, subtrahend : DINT; END_VAR
  VAR_OUTPUT d : DINT; END_VAR
  d := minuend - subtrahend;
END_FUNCTION_BLOCK

PROGRAM main
  VAR i : diff; first : DINT; second : DINT; END_VAR
  i(10, 3);
  first := i.d;
  i(3, 10);
  second := i.d;
END_PROGRAM
",
        &[(1, 7), (2, -7)],
    );
}

// Standard-library blocks bind the same way: CTU takes `CU, R, PV`.
#[test]
fn end_to_end_when_ctu_nonformal_call_then_counts_and_presets() {
    assert_i32(
        "
PROGRAM main
  VAR counter : CTU; cv : INT; q : BOOL; END_VAR
  counter(TRUE, FALSE, 1);
  cv := counter.CV;
  q := counter.Q;
END_PROGRAM
",
        &[(1, 1), (2, 1)],
    );
}

// SR takes `S1, R`: set without reset latches Q1.
#[test]
fn end_to_end_when_sr_nonformal_call_then_sets_output() {
    assert_i32(
        "
PROGRAM main
  VAR latch : SR; q : BOOL; END_VAR
  latch(TRUE, FALSE);
  q := latch.Q1;
END_PROGRAM
",
        &[(1, 1)],
    );
}

// The analyzer refuses a positional count other than the number of inputs
// (P4003). Code generation, which the end-to-end harness reaches without the
// semantic rules, reports reaching one as an internal error rather than
// binding some inputs and dropping the rest.
#[test]
fn compile_when_nonformal_count_differs_from_inputs_then_internal_error() {
    let result = try_parse_and_compile(
        "
PROGRAM main
  VAR timer : TON; END_VAR
  timer(TRUE);
END_PROGRAM
",
        &CompilerOptions::default(),
    );
    assert_eq!(result.unwrap_err().code, "P9998");
}

#[test]
fn compile_when_nonformal_arguments_exceed_inputs_then_internal_error() {
    let result = try_parse_and_compile(
        "
FUNCTION_BLOCK f
  VAR_INPUT a : INT; END_VAR
END_FUNCTION_BLOCK

PROGRAM main
  VAR i : f; END_VAR
  i(1, 2);
END_PROGRAM
",
        &CompilerOptions::default(),
    );
    assert_eq!(result.unwrap_err().code, "P9998");
}
