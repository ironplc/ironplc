//! End-to-end tests for function block invocation (SR set-reset bistable).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! an SR function block instance, compile to bytecode, and execute on the VM.

mod common;

use common::{parse, VmBuffers};
use ironplc_codegen::compile;
use ironplc_vm::test_support::load_and_start;

#[test]
fn end_to_end_when_sr_both_false_then_q1_stays_false() {
    let source = "
PROGRAM main
  VAR
    latch : SR;
    result : BOOL;
  END_VAR
  latch(S1 := FALSE, R := FALSE, Q1 => result);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();
        vm.run_round(0).unwrap();
    }
    assert_eq!(bufs.vars[1].as_i32(), 0, "Q1 should be FALSE");
}

#[test]
fn end_to_end_when_sr_set_then_q1_is_true() {
    let source = "
PROGRAM main
  VAR
    latch : SR;
    set_in : BOOL;
    result : BOOL;
  END_VAR
  latch(S1 := set_in, R := FALSE, Q1 => result);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Set S1 = TRUE
        vm.write_variable(1, 1).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(2).unwrap(),
            1,
            "Q1 should be TRUE after set"
        );

        // Remove S1, Q1 should latch (stay TRUE)
        vm.write_variable(1, 0).unwrap();
        vm.run_round(1).unwrap();
        assert_eq!(
            vm.read_variable(2).unwrap(),
            1,
            "Q1 should stay TRUE (latched)"
        );
    }
}

#[test]
fn end_to_end_when_sr_reset_then_q1_is_false() {
    let source = "
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
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

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
    }
}

#[test]
fn end_to_end_when_sr_both_true_then_set_dominates() {
    let source = "
PROGRAM main
  VAR
    latch : SR;
    result : BOOL;
  END_VAR
  latch(S1 := TRUE, R := TRUE, Q1 => result);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();
        vm.run_round(0).unwrap();
    }
    assert_eq!(
        bufs.vars[1].as_i32(),
        1,
        "Q1 should be TRUE (set dominates in SR)"
    );
}
