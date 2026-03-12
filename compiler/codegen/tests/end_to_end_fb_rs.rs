//! End-to-end tests for function block invocation (RS reset-set bistable).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! an RS function block instance, compile to bytecode, and execute on the VM.

mod common;

use common::parse_and_run;
use common::parse_and_run_rounds;

const RS_PROGRAM: &str = "
PROGRAM main
  VAR
    latch : RS;
    set_in : BOOL;
    reset_in : BOOL;
    result : BOOL;
  END_VAR
  latch(S := set_in, R1 := reset_in, Q1 => result);
END_PROGRAM
";

#[test]
fn end_to_end_when_rs_both_false_then_q1_stays_false() {
    let (_container, bufs) = parse_and_run(RS_PROGRAM);
    assert_eq!(bufs.vars[3].as_i32(), 0, "Q1 should be FALSE");
}

#[test]
fn end_to_end_when_rs_set_then_q1_latches() {
    parse_and_run_rounds(RS_PROGRAM, |vm| {
        // Set S = TRUE
        vm.write_variable(1, 1).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(3).unwrap(),
            1,
            "Q1 should be TRUE after set"
        );

        // Remove S, Q1 should latch (stay TRUE)
        vm.write_variable(1, 0).unwrap();
        vm.run_round(1).unwrap();
        assert_eq!(
            vm.read_variable(3).unwrap(),
            1,
            "Q1 should stay TRUE (latched)"
        );
    });
}

#[test]
fn end_to_end_when_rs_reset_after_set_then_q1_is_false() {
    parse_and_run_rounds(RS_PROGRAM, |vm| {
        // Set
        vm.write_variable(1, 1).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(3).unwrap(), 1, "Q1 should be TRUE");

        // Remove set, apply reset
        vm.write_variable(1, 0).unwrap();
        vm.write_variable(2, 1).unwrap();
        vm.run_round(1).unwrap();
        assert_eq!(
            vm.read_variable(3).unwrap(),
            0,
            "Q1 should be FALSE after reset"
        );
    });
}

#[test]
fn end_to_end_when_rs_both_true_then_reset_dominates() {
    parse_and_run_rounds(RS_PROGRAM, |vm| {
        vm.write_variable(1, 1).unwrap();
        vm.write_variable(2, 1).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(3).unwrap(),
            0,
            "Q1 should be FALSE (reset dominates)"
        );
    });
}
