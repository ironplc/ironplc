//! End-to-end tests for the order in which a statement evaluates its parts,
//! and for how many times it evaluates each.
//!
//! Neither is visible unless a part has a side effect, so every program here
//! calls `trace(log, id)`, which appends the digit `id` to the variable `log`
//! and returns `id`. A log of 123 says three calls ran in that order; a log
//! of 3333 says one call ran four times.

use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

use crate::common::{assert_run_with, drive_fb, expect, run, write, FbStep};

/// The declarations every program here can use: `trace`, a function of three
/// inputs, a function block of two inputs, and one with a method of two
/// inputs.
const DECLARATIONS: &str = "
FUNCTION trace : DINT
  VAR_IN_OUT log : DINT; END_VAR
  VAR_INPUT id : DINT; END_VAR
  log := log * 10 + id;
  trace := id;
END_FUNCTION

FUNCTION digits : DINT
  VAR_INPUT hundreds : DINT; tens : DINT; ones : DINT; END_VAR
  digits := hundreds * 100 + tens * 10 + ones;
END_FUNCTION

FUNCTION_BLOCK Pair
  VAR_INPUT tens : DINT; ones : DINT; END_VAR
  VAR_OUTPUT joined : DINT; END_VAR
  joined := tens * 10 + ones;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Joiner
  METHOD join : DINT
    VAR_INPUT tens : DINT; ones : DINT; END_VAR
    join := tens * 10 + ones;
  END_METHOD
END_FUNCTION_BLOCK
";

/// Methods are an extension; nothing else here needs one.
fn options() -> CompilerOptions {
    CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    }
}

/// Runs a program of `variables` and `body` for one scan and asserts each
/// `(name, value)` of `expected`.
fn assert_main(variables: &str, body: &str, expected: &[(&str, i64)]) {
    let source =
        format!("{DECLARATIONS}\nPROGRAM main\n  VAR {variables} END_VAR\n{body}\nEND_PROGRAM\n");
    assert_run_with(&source, &options(), expected);
}

#[rstest]
#[case::operands_left_to_right(
    "log : DINT; r : DINT;",
    "r := trace(log, 1) + trace(log, 2) * trace(log, 3);",
    &[("log", 123), ("r", 7)]
)]
#[case::arguments_left_to_right(
    "log : DINT; r : DINT;",
    "r := digits(trace(log, 1), trace(log, 2), trace(log, 3));",
    &[("log", 123), ("r", 123)]
)]
// Named arguments of a function are evaluated in the order of its
// parameters, whatever order the call writes them in.
#[case::named_arguments_in_parameter_order(
    "log : DINT; r : DINT;",
    "r := digits(ones := trace(log, 3), hundreds := trace(log, 1), tens := trace(log, 2));",
    &[("log", 123), ("r", 123)]
)]
#[case::method_arguments_left_to_right(
    "log : DINT; r : DINT; joiner : Joiner;",
    "r := joiner.join(trace(log, 1), trace(log, 2));",
    &[("log", 12), ("r", 12)]
)]
#[case::named_method_arguments_in_parameter_order(
    "log : DINT; r : DINT; joiner : Joiner;",
    "r := joiner.join(ones := trace(log, 2), tens := trace(log, 1));",
    &[("log", 12), ("r", 12)]
)]
// The inputs of a function block call are evaluated in the order the block
// declares them, as the named arguments of a function are, whatever order the
// call writes them in (specs/design/call-binding.md).
#[case::function_block_inputs_in_declaration_order(
    "log : DINT; r : DINT; pair : Pair;",
    "pair(ones := trace(log, 2), tens := trace(log, 1), joined => r);",
    &[("log", 12), ("r", 12)]
)]
#[case::subscripts_left_to_right(
    "log : DINT; r : DINT; grid : ARRAY[0..3, 0..3] OF DINT;",
    "grid[trace(log, 1), trace(log, 2)] := 7; r := grid[1, 2];",
    &[("log", 12), ("r", 7)]
)]
// An assignment evaluates its value before the subscripts of its target.
#[case::assigned_value_before_target_subscripts(
    "log : DINT; r : DINT; cells : ARRAY[0..3] OF DINT;",
    "cells[trace(log, 1)] := trace(log, 2); r := cells[1];",
    &[("log", 21), ("r", 2)]
)]
fn end_to_end_when_parts_have_side_effects_then_evaluated_in_order(
    #[case] variables: &str,
    #[case] body: &str,
    #[case] expected: &[(&str, i64)],
) {
    assert_main(variables, body, expected);
}

#[rstest]
#[case::subscript_of_a_stored_element_once(
    "log : DINT; r : DINT; cells : ARRAY[0..3] OF DINT;",
    "cells[trace(log, 2)] := 7; r := cells[2];",
    &[("log", 2), ("r", 7)]
)]
// A FOR evaluates its start once and its end before every iteration, the
// one that leaves the loop included.
#[case::for_start_once_and_end_every_iteration(
    "start : DINT; stop : DINT; i : DINT; n : DINT;",
    "FOR i := trace(start, 1) TO trace(stop, 3) DO n := n + 1; END_FOR;",
    &[("start", 1), ("stop", 3333), ("n", 3), ("i", 4)]
)]
// WHILE tests before every iteration and once more; REPEAT after each.
#[case::while_condition_once_more_than_iterations(
    "log : DINT; i : DINT;",
    "WHILE trace(log, 1) + i < 4 DO i := i + 1; END_WHILE;",
    &[("log", 1111), ("i", 3)]
)]
#[case::repeat_condition_once_per_iteration(
    "log : DINT; i : DINT;",
    "REPEAT i := i + 1; UNTIL trace(log, 1) + i >= 4 END_REPEAT;",
    &[("log", 111), ("i", 3)]
)]
#[case::elsif_condition_after_taken_branch_not_at_all(
    "first : DINT; second : DINT; r : DINT;",
    "IF trace(first, 1) = 1 THEN r := 1; ELSIF trace(second, 1) = 1 THEN r := 2; ELSE r := 3; END_IF;",
    &[("first", 1), ("second", 0), ("r", 1)]
)]
#[case::elsif_condition_after_skipped_branch_once(
    "first : DINT; second : DINT; r : DINT;",
    "IF trace(first, 1) = 2 THEN r := 1; ELSIF trace(second, 1) = 1 THEN r := 2; ELSE r := 3; END_IF;",
    &[("first", 1), ("second", 1), ("r", 2)]
)]
// AND and OR evaluate both operands whatever the first one is.
#[case::and_evaluates_right_operand_when_left_is_false(
    "left : DINT; right : DINT; r : BOOL;",
    "r := (trace(left, 1) = 2) AND (trace(right, 1) = 1);",
    &[("left", 1), ("right", 1), ("r", 0)]
)]
#[case::or_evaluates_right_operand_when_left_is_true(
    "left : DINT; right : DINT; r : BOOL;",
    "r := (trace(left, 1) = 1) OR (trace(right, 1) = 1);",
    &[("left", 1), ("right", 1), ("r", 1)]
)]
fn end_to_end_when_part_has_side_effect_then_evaluated_as_often_as_its_statement_says(
    #[case] variables: &str,
    #[case] body: &str,
    #[case] expected: &[(&str, i64)],
) {
    assert_main(variables, body, expected);
}

#[rstest]
// The end is read again before every iteration, so lowering it in the body
// ends the loop early.
#[case::end_changed_in_body_takes_effect(
    "i : DINT; limit : DINT := 10; n : DINT;",
    "FOR i := 1 TO limit DO n := n + 1; IF i = 3 THEN limit := 5; END_IF; END_FOR;",
    &[("n", 5), ("i", 6)]
)]
#[case::control_variable_assigned_in_body_takes_effect(
    "i : DINT; n : DINT;",
    "FOR i := 1 TO 10 DO n := n + 1; i := i + 1; END_FOR;",
    &[("n", 5), ("i", 11)]
)]
// After the loop the control variable is one step past the last value it
// ran with, or its start if the loop did not run.
#[case::control_variable_after_loop(
    "up : DINT; down : DINT; unused : DINT := 77;",
    "FOR up := 1 TO 5 DO ; END_FOR;
     FOR down := 10 TO 1 BY -3 DO ; END_FOR;
     FOR unused := 5 TO 1 DO ; END_FOR;",
    &[("up", 6), ("down", -2), ("unused", 5)]
)]
// The control variable wraps at its storage width, so a loop to the largest
// value of its type does not end by itself.
#[case::signed_control_variable_wraps(
    "i : SINT; n : DINT;",
    "FOR i := 120 TO 127 DO n := n + 1; IF n >= 20 THEN EXIT; END_IF; END_FOR;",
    &[("n", 20), ("i", -117)]
)]
#[case::unsigned_control_variable_wraps(
    "i : USINT; n : DINT;",
    "FOR i := 250 TO 255 DO n := n + 1; IF n >= 20 THEN EXIT; END_IF; END_FOR;",
    &[("n", 20), ("i", 13)]
)]
#[case::control_variable_beyond_32_bits(
    "i : LINT; sum : LINT;",
    "FOR i := 4294967296 TO 4294967298 DO sum := sum + i; END_FOR;",
    &[("sum", 12_884_901_891), ("i", 4_294_967_299)]
)]
// EXIT leaves only the innermost loop, whatever kinds of loop are nested.
#[case::exit_leaves_innermost_of_mixed_loops(
    "i : DINT; j : DINT; n : DINT; w : DINT;",
    "FOR i := 1 TO 3 DO
       j := 0;
       WHILE TRUE DO
         j := j + 1;
         REPEAT
           w := w + 1;
           IF w MOD 2 = 0 THEN EXIT; END_IF;
         UNTIL FALSE END_REPEAT;
         IF j >= i THEN EXIT; END_IF;
       END_WHILE;
       n := n + j;
     END_FOR;",
    &[("i", 4), ("j", 3), ("n", 6), ("w", 12)]
)]
#[case::exit_in_case_arm_leaves_enclosing_loop(
    "i : DINT; n : DINT;",
    "FOR i := 1 TO 10 DO CASE i OF 3: EXIT; ELSE n := n + 1; END_CASE; END_FOR;",
    &[("i", 3), ("n", 2)]
)]
fn end_to_end_when_loop_runs_then_control_variable_and_count_follow_for_rules(
    #[case] variables: &str,
    #[case] body: &str,
    #[case] expected: &[(&str, i64)],
) {
    assert_main(variables, body, expected);
}

/// The control variable of a FOR may be a global or, in a function block, a
/// field of the instance: each is left one step past the end, like a local.
#[rstest]
#[case::global(
    "
CONFIGURATION config
  VAR_GLOBAL shared : DINT; END_VAR
  RESOURCE resource1 ON PLC
    PROGRAM instance : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL shared : DINT; END_VAR
  VAR sum : DINT; END_VAR
  FOR shared := 1 TO 3 DO sum := sum + shared; END_FOR;
END_PROGRAM
",
    &[("shared", 4), ("sum", 6)]
)]
#[case::field_of_function_block_instance(
    "
FUNCTION_BLOCK Summing
  VAR_OUTPUT sum : DINT; last : DINT; END_VAR
  VAR i : DINT; END_VAR
  sum := 0;
  FOR i := 1 TO 4 DO sum := sum + i; END_FOR;
  last := i;
END_FUNCTION_BLOCK

PROGRAM main
  VAR summing : Summing; sum : DINT; last : DINT; END_VAR
  summing(sum => sum, last => last);
END_PROGRAM
",
    &[("sum", 10), ("last", 5)]
)]
fn end_to_end_when_for_control_variable_is_not_a_local_then_loop_runs_the_same(
    #[case] source: &str,
    #[case] expected: &[(&str, i64)],
) {
    assert_run_with(source, &options(), expected);
}

#[rstest]
// The arms are tried in order, so the first one that matches runs.
#[case::overlapping_labels_first_arm_wins(
    "x : DINT := 5; r : DINT;",
    "CASE x OF 1..10: r := 1; 5: r := 2; ELSE r := 3; END_CASE;",
    &[("r", 1)]
)]
#[case::selector_is_expression_with_negative_labels(
    "x : DINT := -4; r : DINT;",
    "CASE x + 1 OF -10..-5: r := 1; -3: r := 2; 0: r := 3; END_CASE;",
    &[("r", 2)]
)]
#[case::label_is_least_value_of_narrow_selector(
    "x : SINT := -128; r : DINT;",
    "CASE x OF -128: r := 1; 127: r := 2; END_CASE;",
    &[("r", 1)]
)]
#[case::empty_arm_and_nested_case(
    "x : DINT := 2; y : DINT := 1; r : DINT;",
    "CASE x OF 1: ; 2: CASE y OF 1: r := 21; 2: r := 22; END_CASE; END_CASE;",
    &[("r", 21)]
)]
fn end_to_end_when_case_selects_then_first_matching_arm_runs(
    #[case] variables: &str,
    #[case] body: &str,
    #[case] expected: &[(&str, i64)],
) {
    assert_main(variables, body, expected);
}

#[rstest]
// The inputs are assigned, the body runs, then the outputs are assigned, so
// an output may be stored in the variable an input was read from.
#[case::output_stored_in_variable_an_input_read(
    "pair : Pair; x : DINT := 4;",
    "pair(tens := x, ones := 0, joined => x);",
    &[("x", 40)]
)]
#[case::input_reads_own_output_of_previous_call(
    "pair : Pair; r : DINT;",
    "pair(tens := 4, ones := 0); pair(tens := pair.joined, ones := 1); r := pair.joined;",
    &[("r", 401)]
)]
// An input a call does not name keeps the value an earlier call gave it.
#[case::input_not_named_keeps_its_value(
    "pair : Pair; all : DINT; one : DINT; none : DINT;",
    "pair(tens := 1, ones := 2); all := pair.joined;
     pair(ones := 5); one := pair.joined;
     pair(); none := pair.joined;",
    &[("all", 12), ("one", 15), ("none", 15)]
)]
fn end_to_end_when_function_block_called_then_inputs_body_outputs_in_that_order(
    #[case] variables: &str,
    #[case] body: &str,
    #[case] expected: &[(&str, i64)],
) {
    assert_main(variables, body, expected);
}

/// One instance called more than once in a scan keeps its state from call to
/// call, not from scan to scan: the second call of an edge detector sees no
/// edge, a counter counts every edge it is shown, and a timer called twice
/// in one round sees the same time both times.
#[rstest]
#[case::clock_rises(&[
    write("clock", 1), run(0),
    expect("first_edge", 1), expect("second_edge", 0), expect("count", 2), expect("timed_out", 1),
])]
#[case::clock_stays_high(&[
    write("clock", 1), run(0), run(1),
    expect("first_edge", 0), expect("second_edge", 0), expect("count", 3), expect("timed_out", 1),
])]
fn end_to_end_when_instance_called_twice_in_scan_then_state_carries_between_calls(
    #[case] steps: &[FbStep],
) {
    let source = "
PROGRAM main
  VAR
    clock : BOOL;
    edge : R_TRIG; first_edge : BOOL; second_edge : BOOL;
    counter : CTU; count : INT;
    timer : TON; timed_out : BOOL;
  END_VAR
  edge(CLK := clock, Q => first_edge);
  edge(CLK := clock, Q => second_edge);
  counter(CU := clock, PV := 5);
  counter(CU := FALSE);
  counter(CU := clock);
  count := counter.CV;
  timer(IN := TRUE, PT := T#0ms);
  timer();
  timed_out := timer.Q;
END_PROGRAM
";
    drive_fb(source, &CompilerOptions::default(), steps);
}
