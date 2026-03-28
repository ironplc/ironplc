//! End-to-end tests for user-defined function block compilation and execution.
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! user-defined FUNCTION_BLOCK declarations, compile to bytecode, and execute
//! on the VM.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;
use common::parse_and_run_rounds;

#[test]
fn end_to_end_when_user_fb_simple_input_output_then_computes_result() {
    let source = "
FUNCTION_BLOCK DOUBLER
  VAR_INPUT x : DINT; END_VAR
  VAR_OUTPUT y : DINT; END_VAR
  y := x * 2;
END_FUNCTION_BLOCK

PROGRAM main
  VAR
    fb : DOUBLER;
    result : DINT;
  END_VAR
  fb(x := 7, y => result);
END_PROGRAM
";
    let (_container, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[1].as_i32(), 14, "result should be 7 * 2 = 14");
}

#[test]
fn end_to_end_when_user_fb_internal_state_then_persists_across_calls() {
    let source = "
FUNCTION_BLOCK ACCUMULATOR
  VAR_INPUT val : DINT; END_VAR
  VAR total : DINT; END_VAR
  VAR_OUTPUT sum : DINT; END_VAR
  total := total + val;
  sum := total;
END_FUNCTION_BLOCK

PROGRAM main
  VAR
    acc : ACCUMULATOR;
    result : DINT;
  END_VAR
  acc(val := 10, sum => result);
END_PROGRAM
";
    parse_and_run_rounds(source, &CompilerOptions::default(), |vm| {
        // Round 1: total = 0 + 10 = 10
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(1).unwrap(),
            10,
            "round 1: sum should be 10"
        );

        // Round 2: total = 10 + 10 = 20 (state persists in data region)
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(1).unwrap(),
            20,
            "round 2: sum should be 20"
        );
    });
}

#[test]
fn end_to_end_when_two_user_fb_instances_then_independent_state() {
    let source = "
FUNCTION_BLOCK COUNTER
  VAR_INPUT inc : DINT; END_VAR
  VAR count : DINT; END_VAR
  VAR_OUTPUT value : DINT; END_VAR
  count := count + inc;
  value := count;
END_FUNCTION_BLOCK

PROGRAM main
  VAR
    c1 : COUNTER;
    c2 : COUNTER;
    r1 : DINT;
    r2 : DINT;
  END_VAR
  c1(inc := 1, value => r1);
  c2(inc := 100, value => r2);
END_PROGRAM
";
    parse_and_run_rounds(source, &CompilerOptions::default(), |vm| {
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(2).unwrap(), 1, "c1 should be 1");
        assert_eq!(vm.read_variable(3).unwrap(), 100, "c2 should be 100");

        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(2).unwrap(), 2, "c1 should be 2");
        assert_eq!(vm.read_variable(3).unwrap(), 200, "c2 should be 200");
    });
}
