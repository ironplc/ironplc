//! End-to-end tests for function block invocation (TP pulse timer).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! a TP function block instance, compile to bytecode, and execute on the VM.
//!
//! TIME values are 32-bit signed integers in milliseconds.
//! The VM cycle_time is in microseconds; timer intrinsics convert to ms internally.

mod common;

use common::{parse_and_compile, VmBuffers};
use ironplc_vm::test_support::load_and_start;

#[test]
fn end_to_end_when_tp_triggered_then_q_is_true() {
    let source = "
PROGRAM main
  VAR
    timer : TP;
    result : BOOL;
  END_VAR
  timer(IN := TRUE, PT := T#5s, Q => result);
END_PROGRAM
";
    let container = parse_and_compile(source);
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1 at t=0: IN goes TRUE, pulse starts, Q should be TRUE
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(1).unwrap(),
            1,
            "Q should be TRUE when pulse starts"
        );
    }
}

#[test]
fn end_to_end_when_tp_before_pt_then_q_is_true() {
    let source = "
PROGRAM main
  VAR
    timer : TP;
    result : BOOL;
  END_VAR
  timer(IN := TRUE, PT := T#5s, Q => result);
END_PROGRAM
";
    let container = parse_and_compile(source);
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1 at t=0: pulse starts
        vm.run_round(0).unwrap();

        // Round 2 at t=2s: still before PT (5s)
        vm.run_round(2_000_000).unwrap();
        assert_eq!(vm.read_variable(1).unwrap(), 1, "Q should be TRUE at t=2s");
    }
}

#[test]
fn end_to_end_when_tp_after_pt_then_q_is_false() {
    let source = "
PROGRAM main
  VAR
    timer : TP;
    result : BOOL;
  END_VAR
  timer(IN := TRUE, PT := T#5s, Q => result);
END_PROGRAM
";
    let container = parse_and_compile(source);
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1 at t=0: pulse starts
        vm.run_round(0).unwrap();

        // Round 2 at t=6s: past PT (5s), pulse expired
        vm.run_round(6_000_000).unwrap();
        assert_eq!(
            vm.read_variable(1).unwrap(),
            0,
            "Q should be FALSE after PT elapsed"
        );
    }
}

#[test]
fn end_to_end_when_tp_reads_et_then_elapsed_time_correct() {
    let source = "
PROGRAM main
  VAR
    timer : TP;
    elapsed : TIME;
  END_VAR
  timer(IN := TRUE, PT := T#10s, ET => elapsed);
END_PROGRAM
";
    let container = parse_and_compile(source);
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1 at t=0: pulse starts
        vm.run_round(0).unwrap();

        // Round 2 at t=3s: ET should be 3000 ms (3 seconds)
        vm.run_round(3_000_000).unwrap();
        assert_eq!(
            vm.read_variable(1).unwrap(),
            3000,
            "ET should be 3000 ms (3 seconds)"
        );
    }
}

#[test]
fn end_to_end_when_tp_in_falls_during_pulse_then_q_stays_true() {
    // TP-specific: IN goes FALSE during pulse, but Q stays TRUE until PT expires.
    let source = "
PROGRAM main
  VAR
    timer : TP;
    enable : BOOL;
    result : BOOL;
    elapsed : TIME;
  END_VAR
  timer(IN := enable, PT := T#5s, Q => result, ET => elapsed);
END_PROGRAM
";
    let container = parse_and_compile(source);
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1 at t=0: enable=TRUE, pulse starts
        vm.write_variable(1, 1).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(2).unwrap(), 1, "Q should be TRUE");

        // Round 2 at t=2s: enable=FALSE, but pulse continues
        vm.write_variable(1, 0).unwrap();
        vm.run_round(2_000_000).unwrap();
        assert_eq!(
            vm.read_variable(2).unwrap(),
            1,
            "Q should still be TRUE (pulse ignores IN)"
        );

        // Round 3 at t=6s: pulse expired
        vm.run_round(6_000_000).unwrap();
        assert_eq!(
            vm.read_variable(2).unwrap(),
            0,
            "Q should be FALSE after pulse expired"
        );
        assert_eq!(
            vm.read_variable(3).unwrap(),
            5000,
            "ET should be clamped to PT (5000 ms)"
        );
    }
}

#[test]
fn end_to_end_when_tp_at_exact_pt_then_q_is_false() {
    let source = "
PROGRAM main
  VAR
    timer : TP;
    result : BOOL;
  END_VAR
  timer(IN := TRUE, PT := T#5s, Q => result);
END_PROGRAM
";
    let container = parse_and_compile(source);
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1 at t=0: pulse starts
        vm.run_round(0).unwrap();

        // Round 2 at exactly t=5s: ET == PT, pulse should end
        vm.run_round(5_000_000).unwrap();
        assert_eq!(
            vm.read_variable(1).unwrap(),
            0,
            "Q should be FALSE when ET equals PT exactly"
        );
    }
}

#[test]
fn end_to_end_when_two_tp_timers_then_independent() {
    let source = "
PROGRAM main
  VAR
    timer1 : TP;
    timer2 : TP;
    enable : BOOL;
    q1 : BOOL;
    q2 : BOOL;
  END_VAR
  timer1(IN := enable, PT := T#3s, Q => q1);
  timer2(IN := enable, PT := T#7s, Q => q2);
END_PROGRAM
";
    let container = parse_and_compile(source);
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1 at t=0: enable=TRUE, both start pulsing
        // enable is var index 2 (after timer1=0, timer2=1)
        vm.write_variable(2, 1).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(3).unwrap(), 1, "q1 should be TRUE");
        assert_eq!(vm.read_variable(4).unwrap(), 1, "q2 should be TRUE");

        // Drop enable so pulses don't retrigger after expiry
        vm.write_variable(2, 0).unwrap();

        // Round 2 at t=4s: timer1 (3s) pulse expired, timer2 (7s) still pulsing
        vm.run_round(4_000_000).unwrap();
        assert_eq!(
            vm.read_variable(3).unwrap(),
            0,
            "q1 should be FALSE at t=4s"
        );
        assert_eq!(vm.read_variable(4).unwrap(), 1, "q2 should be TRUE at t=4s");

        // Round 3 at t=8s: both pulses expired
        vm.run_round(8_000_000).unwrap();
        assert_eq!(
            vm.read_variable(3).unwrap(),
            0,
            "q1 should be FALSE at t=8s"
        );
        assert_eq!(
            vm.read_variable(4).unwrap(),
            0,
            "q2 should be FALSE at t=8s"
        );
    }
}
