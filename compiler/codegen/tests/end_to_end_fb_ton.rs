//! End-to-end tests for function block invocation (TON on-delay timer).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! a TON function block instance, compile to bytecode, and execute on the VM.

mod common;

use common::{parse, VmBuffers};
use ironplc_codegen::compile;
use ironplc_vm::test_support::load_and_start;

#[test]
fn end_to_end_when_ton_not_triggered_then_q_is_false() {
    let source = "
PROGRAM main
  VAR
    timer : TON;
    result : BOOL;
  END_VAR
  timer(IN := FALSE, PT := T#5s, Q => result);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();
        vm.run_round(0).unwrap();
    }

    // Q should be FALSE when IN is FALSE
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn end_to_end_when_ton_triggered_before_pt_then_q_is_false() {
    let source = "
PROGRAM main
  VAR
    timer : TON;
    result : BOOL;
  END_VAR
  timer(IN := TRUE, PT := T#5s, Q => result);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1 at t=0: IN goes TRUE, starts timing
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(1).unwrap(),
            0,
            "Q should be FALSE before PT elapsed"
        );

        // Round 2 at t=2s: still before PT (5s)
        vm.run_round(2_000_000).unwrap();
        assert_eq!(
            vm.read_variable(1).unwrap(),
            0,
            "Q should still be FALSE at t=2s"
        );
    }
}

#[test]
fn end_to_end_when_ton_triggered_after_pt_then_q_is_true() {
    let source = "
PROGRAM main
  VAR
    timer : TON;
    result : BOOL;
  END_VAR
  timer(IN := TRUE, PT := T#5s, Q => result);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1 at t=0: IN goes TRUE, starts timing
        vm.run_round(0).unwrap();

        // Round 2 at t=6s: past PT (5s), Q should be TRUE
        vm.run_round(6_000_000).unwrap();
        assert_eq!(
            vm.read_variable(1).unwrap(),
            1,
            "Q should be TRUE after PT elapsed"
        );
    }
}

#[test]
fn end_to_end_when_ton_reads_et_then_elapsed_time_correct() {
    let source = "
PROGRAM main
  VAR
    timer : TON;
    elapsed : TIME;
  END_VAR
  timer(IN := TRUE, PT := T#10s, ET => elapsed);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1 at t=0: IN goes TRUE, starts timing
        vm.run_round(0).unwrap();

        // Round 2 at t=3s: ET should be 3s = 3_000_000 us
        vm.run_round(3_000_000).unwrap();
        assert_eq!(
            vm.read_variable_i64(1).unwrap(),
            3_000_000,
            "ET should be 3s in microseconds"
        );
    }
}
