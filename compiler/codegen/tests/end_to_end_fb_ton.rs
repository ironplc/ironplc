//! End-to-end tests for function block invocation (TON on-delay timer).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! a TON function block instance, compile to bytecode, and execute on the VM.
//!
//! TIME values are 32-bit signed integers in milliseconds.
//! The VM cycle_time is in microseconds; timer intrinsics convert to ms internally.

mod common;

use common::{parse, VmBuffers};
use ironplc_codegen::compile;
use ironplc_vm::test_support::load_and_start;

#[test]
fn end_to_end_when_time_variable_then_debug_type_name_is_time() {
    let source = "
PROGRAM main
  VAR
    elapsed : TIME;
  END_VAR
  elapsed := T#5s;
END_PROGRAM
";
    let (library, context) = parse(source);
    let container = compile(&library, context.functions(), context.types()).unwrap();
    let debug = container.debug_section.as_ref().unwrap();
    let elapsed_entry = debug
        .var_names
        .iter()
        .find(|e| e.name == "elapsed")
        .unwrap();
    assert_eq!(
        elapsed_entry.type_name, "TIME",
        "TIME variable should have type_name TIME, got {}",
        elapsed_entry.type_name
    );
}

#[test]
fn end_to_end_when_time_variable_then_value_is_i32_milliseconds() {
    let source = "
PROGRAM main
  VAR
    elapsed : TIME;
  END_VAR
  elapsed := T#5s;
END_PROGRAM
";
    let (library, context) = parse(source);
    let container = compile(&library, context.functions(), context.types()).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();
        vm.run_round(0).unwrap();
        // T#5s = 5000 milliseconds as i32
        assert_eq!(
            vm.read_variable(0).unwrap(),
            5000,
            "TIME value should be 5000 ms"
        );
    }
}

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
    let (library, context) = parse(source);
    let container = compile(&library, context.functions(), context.types()).unwrap();
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
    let (library, context) = parse(source);
    let container = compile(&library, context.functions(), context.types()).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1 at t=0us: IN goes TRUE, starts timing
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(1).unwrap(),
            0,
            "Q should be FALSE before PT elapsed"
        );

        // Round 2 at t=2s (2_000_000 us): still before PT (5s = 5000 ms)
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
    let (library, context) = parse(source);
    let container = compile(&library, context.functions(), context.types()).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1 at t=0: IN goes TRUE, starts timing
        vm.run_round(0).unwrap();

        // Round 2 at t=6s: past PT (5s = 5000 ms), Q should be TRUE
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
    let (library, context) = parse(source);
    let container = compile(&library, context.functions(), context.types()).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1 at t=0: IN goes TRUE, starts timing
        vm.run_round(0).unwrap();

        // Round 2 at t=3s: ET should be 3000 ms (i32)
        vm.run_round(3_000_000).unwrap();
        assert_eq!(
            vm.read_variable(1).unwrap(),
            3000,
            "ET should be 3000 ms (3 seconds)"
        );
    }
}

#[test]
fn end_to_end_when_ton_in_reset_then_timer_restarts() {
    let source = "
PROGRAM main
  VAR
    timer : TON;
    enable : BOOL;
    result : BOOL;
    elapsed : TIME;
  END_VAR
  timer(IN := enable, PT := T#5s, Q => result, ET => elapsed);
END_PROGRAM
";
    let (library, context) = parse(source);
    let container = compile(&library, context.functions(), context.types()).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1 at t=0: enable=TRUE, timer starts
        vm.write_variable(1, 1).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(2).unwrap(), 0, "Q should be FALSE");

        // Round 2 at t=3s: still timing, not yet at PT
        vm.run_round(3_000_000).unwrap();
        assert_eq!(vm.read_variable(2).unwrap(), 0, "Q should be FALSE at t=3s");

        // Round 3 at t=4s: enable=FALSE, reset timer
        vm.write_variable(1, 0).unwrap();
        vm.run_round(4_000_000).unwrap();
        assert_eq!(
            vm.read_variable(2).unwrap(),
            0,
            "Q should be FALSE after reset"
        );
        assert_eq!(
            vm.read_variable(3).unwrap(),
            0,
            "ET should be 0 after reset"
        );

        // Round 4 at t=6s: enable=TRUE again, timer restarts from here
        vm.write_variable(1, 1).unwrap();
        vm.run_round(6_000_000).unwrap();
        assert_eq!(
            vm.read_variable(2).unwrap(),
            0,
            "Q should be FALSE, timer just restarted"
        );

        // Round 5 at t=10s: only 4s since restart, not yet at PT (5s)
        vm.run_round(10_000_000).unwrap();
        assert_eq!(
            vm.read_variable(2).unwrap(),
            0,
            "Q should be FALSE, only 4s since restart"
        );

        // Round 6 at t=12s: 6s since restart, past PT (5s)
        vm.run_round(12_000_000).unwrap();
        assert_eq!(
            vm.read_variable(2).unwrap(),
            1,
            "Q should be TRUE, 6s since restart > PT"
        );
    }
}

#[test]
fn end_to_end_when_ton_at_exact_pt_then_q_is_true() {
    let source = "
PROGRAM main
  VAR
    timer : TON;
    result : BOOL;
  END_VAR
  timer(IN := TRUE, PT := T#5s, Q => result);
END_PROGRAM
";
    let (library, context) = parse(source);
    let container = compile(&library, context.functions(), context.types()).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1 at t=0: IN goes TRUE, starts timing
        vm.run_round(0).unwrap();

        // Round 2 at exactly t=5s: ET == PT (5000 ms), Q should be TRUE
        vm.run_round(5_000_000).unwrap();
        assert_eq!(
            vm.read_variable(1).unwrap(),
            1,
            "Q should be TRUE when ET equals PT exactly"
        );
    }
}

#[test]
fn end_to_end_when_two_ton_timers_then_independent() {
    let source = "
PROGRAM main
  VAR
    timer1 : TON;
    timer2 : TON;
    q1 : BOOL;
    q2 : BOOL;
  END_VAR
  timer1(IN := TRUE, PT := T#3s, Q => q1);
  timer2(IN := TRUE, PT := T#7s, Q => q2);
END_PROGRAM
";
    let (library, context) = parse(source);
    let container = compile(&library, context.functions(), context.types()).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Round 1 at t=0: both start
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(2).unwrap(), 0, "q1 should be FALSE");
        assert_eq!(vm.read_variable(3).unwrap(), 0, "q2 should be FALSE");

        // Round 2 at t=4s: timer1 (3s) done, timer2 (7s) still running
        vm.run_round(4_000_000).unwrap();
        assert_eq!(vm.read_variable(2).unwrap(), 1, "q1 should be TRUE at t=4s");
        assert_eq!(
            vm.read_variable(3).unwrap(),
            0,
            "q2 should be FALSE at t=4s"
        );

        // Round 3 at t=8s: both done
        vm.run_round(8_000_000).unwrap();
        assert_eq!(vm.read_variable(2).unwrap(), 1, "q1 should be TRUE at t=8s");
        assert_eq!(vm.read_variable(3).unwrap(), 1, "q2 should be TRUE at t=8s");
    }
}
