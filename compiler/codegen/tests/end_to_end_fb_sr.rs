//! End-to-end tests for function block invocation (SR set-reset bistable).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! an SR function block instance, compile to bytecode, and execute on the VM.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;
use common::parse_and_run_rounds;
use ironplc_container::VarIndex;

const SR_PROGRAM: &str = "
PROGRAM main
  VAR
    latch : SR;
    set_in : BOOL;
    reset_in : BOOL;
    result : BOOL;
  END_VAR
  latch(S1 := set_in, R := reset_in, Q1 => result);
END_PROGRAM
";

#[test]
fn end_to_end_when_sr_both_false_then_q1_stays_false() {
    let (_container, bufs) = parse_and_run(SR_PROGRAM, &CompilerOptions::default());
    assert_eq!(bufs.vars[3].as_i32(), 0, "Q1 should be FALSE");
}

#[test]
fn end_to_end_when_sr_set_then_q1_latches() {
    parse_and_run_rounds(SR_PROGRAM, &CompilerOptions::default(), |vm| {
        // Set S1 = TRUE
        vm.write_variable(VarIndex::new(1), 1).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(3)).unwrap(),
            1,
            "Q1 should be TRUE after set"
        );

        // Remove S1, Q1 should latch (stay TRUE)
        vm.write_variable(VarIndex::new(1), 0).unwrap();
        vm.run_round(1).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(3)).unwrap(),
            1,
            "Q1 should stay TRUE (latched)"
        );
    });
}

#[test]
fn end_to_end_when_sr_reset_after_set_then_q1_is_false() {
    parse_and_run_rounds(SR_PROGRAM, &CompilerOptions::default(), |vm| {
        // Set
        vm.write_variable(VarIndex::new(1), 1).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(VarIndex::new(3)).unwrap(), 1, "Q1 should be TRUE");

        // Remove set, apply reset
        vm.write_variable(VarIndex::new(1), 0).unwrap();
        vm.write_variable(VarIndex::new(2), 1).unwrap();
        vm.run_round(1).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(3)).unwrap(),
            0,
            "Q1 should be FALSE after reset"
        );
    });
}

#[test]
fn end_to_end_when_sr_both_true_then_set_dominates() {
    parse_and_run_rounds(SR_PROGRAM, &CompilerOptions::default(), |vm| {
        vm.write_variable(VarIndex::new(1), 1).unwrap();
        vm.write_variable(VarIndex::new(2), 1).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(3)).unwrap(),
            1,
            "Q1 should be TRUE (set dominates)"
        );
    });
}
