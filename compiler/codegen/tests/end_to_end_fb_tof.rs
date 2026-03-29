//! End-to-end tests for function block invocation (TOF off-delay timer).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! a TOF function block instance, compile to bytecode, and execute on the VM.
//!
//! TIME values are 32-bit signed integers in milliseconds.
//! The VM cycle_time is in microseconds; timer intrinsics convert to ms internally.

mod common;
use ironplc_container::VarIndex;
use ironplc_parser::options::CompilerOptions;

use common::{parse_and_compile, VmBuffers};
use ironplc_vm::test_support::load_and_start;

#[test]
fn end_to_end_when_tof_in_true_then_q_is_true() {
    let source = "
PROGRAM main
  VAR
    timer : TOF;
    result : BOOL;
  END_VAR
  timer(IN := TRUE, PT := T#5s, Q => result);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();
        vm.run_round(0).unwrap();
    }

    // Q should be TRUE when IN is TRUE
    assert_eq!(bufs.vars[1].as_i32(), 1);
}

#[test]
fn end_to_end_when_tof_in_false_before_pt_then_q_is_true() {
    let source = "
PROGRAM main
  VAR
    timer : TOF;
    enable : BOOL;
    result : BOOL;
  END_VAR
  timer(IN := enable, PT := T#5s, Q => result);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1 at t=0: enable=TRUE
        vm.write_variable(VarIndex::new(1), 1).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(2)).unwrap(),
            1,
            "Q should be TRUE when IN is TRUE"
        );

        // Round 2 at t=1s: enable=FALSE, falling edge starts timing
        vm.write_variable(VarIndex::new(1), 0).unwrap();
        vm.run_round(1_000_000).unwrap();

        // Round 3 at t=3s: 2s elapsed, still before PT (5s)
        vm.run_round(3_000_000).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(2)).unwrap(),
            1,
            "Q should still be TRUE during off-delay"
        );
    }
}

#[test]
fn end_to_end_when_tof_in_false_after_pt_then_q_is_false() {
    let source = "
PROGRAM main
  VAR
    timer : TOF;
    enable : BOOL;
    result : BOOL;
  END_VAR
  timer(IN := enable, PT := T#5s, Q => result);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1: enable=TRUE
        vm.write_variable(VarIndex::new(1), 1).unwrap();
        vm.run_round(0).unwrap();

        // Round 2: enable=FALSE, falling edge
        vm.write_variable(VarIndex::new(1), 0).unwrap();
        vm.run_round(1_000_000).unwrap();

        // Round 3 at t=7s: 6s elapsed > 5s PT, Q should be FALSE
        vm.run_round(7_000_000).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(2)).unwrap(),
            0,
            "Q should be FALSE after PT elapsed"
        );
    }
}

#[test]
fn end_to_end_when_tof_reads_et_then_elapsed_time_correct() {
    let source = "
PROGRAM main
  VAR
    timer : TOF;
    enable : BOOL;
    elapsed : TIME;
  END_VAR
  timer(IN := enable, PT := T#10s, ET => elapsed);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1: enable=TRUE
        vm.write_variable(VarIndex::new(1), 1).unwrap();
        vm.run_round(0).unwrap();

        // Round 2: enable=FALSE, falling edge starts timing
        vm.write_variable(VarIndex::new(1), 0).unwrap();
        vm.run_round(1_000_000).unwrap();

        // Round 3 at t=4s: ET should be 3000 ms (3 seconds)
        // elapsed is var[2] (timer=var[0], enable=var[1], elapsed=var[2])
        vm.run_round(4_000_000).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(2)).unwrap(),
            3000,
            "ET should be 3000 ms (3 seconds)"
        );
    }
}

#[test]
fn end_to_end_when_tof_in_rises_during_timing_then_resets() {
    let source = "
PROGRAM main
  VAR
    timer : TOF;
    enable : BOOL;
    result : BOOL;
    elapsed : TIME;
  END_VAR
  timer(IN := enable, PT := T#5s, Q => result, ET => elapsed);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1 at t=0: enable=TRUE
        vm.write_variable(VarIndex::new(1), 1).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(2)).unwrap(),
            1,
            "Q should be TRUE"
        );

        // Round 2 at t=1s: enable=FALSE, falling edge
        vm.write_variable(VarIndex::new(1), 0).unwrap();
        vm.run_round(1_000_000).unwrap();

        // Round 3 at t=3s: 2s elapsed, still timing
        vm.run_round(3_000_000).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(2)).unwrap(),
            1,
            "Q should be TRUE during timing"
        );

        // Round 4 at t=4s: enable=TRUE again, reset
        vm.write_variable(VarIndex::new(1), 1).unwrap();
        vm.run_round(4_000_000).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(2)).unwrap(),
            1,
            "Q should be TRUE"
        );
        assert_eq!(
            vm.read_variable(VarIndex::new(3)).unwrap(),
            0,
            "ET should be 0 after reset"
        );

        // Round 5 at t=5s: enable=FALSE again, new falling edge
        vm.write_variable(VarIndex::new(1), 0).unwrap();
        vm.run_round(5_000_000).unwrap();

        // Round 6 at t=8s: 3s since new falling edge, still before PT
        vm.run_round(8_000_000).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(2)).unwrap(),
            1,
            "Q should be TRUE, only 3s since new falling edge"
        );

        // Round 7 at t=11s: 6s since new falling edge, past PT
        vm.run_round(11_000_000).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(2)).unwrap(),
            0,
            "Q should be FALSE, past PT since new falling edge"
        );
    }
}

#[test]
fn end_to_end_when_tof_at_exact_pt_then_q_is_false() {
    let source = "
PROGRAM main
  VAR
    timer : TOF;
    enable : BOOL;
    result : BOOL;
  END_VAR
  timer(IN := enable, PT := T#5s, Q => result);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1: enable=TRUE
        vm.write_variable(VarIndex::new(1), 1).unwrap();
        vm.run_round(0).unwrap();

        // Round 2 at t=1s: enable=FALSE, falling edge
        vm.write_variable(VarIndex::new(1), 0).unwrap();
        vm.run_round(1_000_000).unwrap();

        // Round 3 at exactly t=6s: ET == PT (5s), Q should be FALSE
        vm.run_round(6_000_000).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(2)).unwrap(),
            0,
            "Q should be FALSE when ET equals PT exactly"
        );
    }
}

#[test]
fn end_to_end_when_two_tof_timers_then_independent() {
    let source = "
PROGRAM main
  VAR
    timer1 : TOF;
    timer2 : TOF;
    enable : BOOL;
    q1 : BOOL;
    q2 : BOOL;
  END_VAR
  timer1(IN := enable, PT := T#3s, Q => q1);
  timer2(IN := enable, PT := T#7s, Q => q2);
END_PROGRAM
";
    let container = parse_and_compile(source, &CompilerOptions::default());
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1: enable=TRUE
        vm.write_variable(VarIndex::new(2), 1).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(3)).unwrap(),
            1,
            "q1 should be TRUE"
        );
        assert_eq!(
            vm.read_variable(VarIndex::new(4)).unwrap(),
            1,
            "q2 should be TRUE"
        );

        // Round 2 at t=1s: enable=FALSE, both start timing
        vm.write_variable(VarIndex::new(2), 0).unwrap();
        vm.run_round(1_000_000).unwrap();

        // Round 3 at t=5s: timer1 (3s) done, timer2 (7s) still running
        vm.run_round(5_000_000).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(3)).unwrap(),
            0,
            "q1 should be FALSE at t=5s"
        );
        assert_eq!(
            vm.read_variable(VarIndex::new(4)).unwrap(),
            1,
            "q2 should be TRUE at t=5s"
        );

        // Round 4 at t=9s: both done
        vm.run_round(9_000_000).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(3)).unwrap(),
            0,
            "q1 should be FALSE at t=9s"
        );
        assert_eq!(
            vm.read_variable(VarIndex::new(4)).unwrap(),
            0,
            "q2 should be FALSE at t=9s"
        );
    }
}
