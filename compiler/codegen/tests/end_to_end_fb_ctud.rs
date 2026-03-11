//! End-to-end tests for function block invocation (CTUD count up/down counter).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! a CTUD function block instance, compile to bytecode, and execute on the VM.

mod common;

use common::{parse, VmBuffers};
use ironplc_codegen::compile;
use ironplc_vm::test_support::load_and_start;

#[test]
fn end_to_end_when_ctud_cu_then_cv_increments() {
    let source = "
PROGRAM main
  VAR
    counter : CTUD;
    up_pulse : BOOL;
    qu_out : BOOL;
    qd_out : BOOL;
    cv_out : INT;
  END_VAR
  counter(CU := up_pulse, CD := FALSE, R := FALSE, LD := FALSE, PV := 5, QU => qu_out, QD => qd_out, CV => cv_out);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // CU rising edge
        vm.write_variable(1, 1).unwrap(); // up_pulse = TRUE
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(4).unwrap(),
            1,
            "CV should be 1 after first CU edge"
        );
        assert_eq!(
            vm.read_variable(2).unwrap(),
            0,
            "QU should be FALSE (1 < 5)"
        );
        assert_eq!(
            vm.read_variable(3).unwrap(),
            0,
            "QD should be FALSE (1 > 0)"
        );
    }
}

#[test]
fn end_to_end_when_ctud_reaches_pv_then_qu_true() {
    let source = "
PROGRAM main
  VAR
    counter : CTUD;
    up_pulse : BOOL;
    qu_out : BOOL;
    cv_out : INT;
  END_VAR
  counter(CU := up_pulse, CD := FALSE, R := FALSE, LD := FALSE, PV := 3, QU => qu_out, CV => cv_out);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Count up 3 times (3 rising edges)
        for _ in 0..3 {
            vm.write_variable(1, 1).unwrap();
            vm.run_round(0).unwrap();
            vm.write_variable(1, 0).unwrap();
            vm.run_round(0).unwrap();
        }
        assert_eq!(vm.read_variable(3).unwrap(), 3, "CV should be 3");
        assert_eq!(
            vm.read_variable(2).unwrap(),
            1,
            "QU should be TRUE (CV >= PV)"
        );
    }
}

#[test]
fn end_to_end_when_ctud_cd_then_cv_decrements() {
    let source = "
PROGRAM main
  VAR
    counter : CTUD;
    up_pulse : BOOL;
    down_pulse : BOOL;
    qd_out : BOOL;
    cv_out : INT;
  END_VAR
  counter(CU := up_pulse, CD := down_pulse, R := FALSE, LD := FALSE, PV := 10, QD => qd_out, CV => cv_out);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Count up twice
        for _ in 0..2 {
            vm.write_variable(1, 1).unwrap();
            vm.run_round(0).unwrap();
            vm.write_variable(1, 0).unwrap();
            vm.run_round(0).unwrap();
        }
        assert_eq!(vm.read_variable(4).unwrap(), 2, "CV should be 2");

        // Count down once
        vm.write_variable(2, 1).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(4).unwrap(), 1, "CV should be 1 after CD");
    }
}

#[test]
fn end_to_end_when_ctud_reset_then_cv_zero() {
    let source = "
PROGRAM main
  VAR
    counter : CTUD;
    up_pulse : BOOL;
    reset : BOOL;
    cv_out : INT;
  END_VAR
  counter(CU := up_pulse, CD := FALSE, R := reset, LD := FALSE, PV := 10, CV => cv_out);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Count up
        vm.write_variable(1, 1).unwrap();
        vm.run_round(0).unwrap();
        vm.write_variable(1, 0).unwrap();
        vm.run_round(0).unwrap();
        vm.write_variable(1, 1).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(3).unwrap(), 2, "CV should be 2");

        // Reset
        vm.write_variable(1, 0).unwrap();
        vm.write_variable(2, 1).unwrap(); // reset = TRUE
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(3).unwrap(),
            0,
            "CV should be 0 after reset"
        );
    }
}

#[test]
fn end_to_end_when_ctud_load_then_cv_equals_pv() {
    let source = "
PROGRAM main
  VAR
    counter : CTUD;
    load_input : BOOL;
    qu_out : BOOL;
    cv_out : INT;
  END_VAR
  counter(CU := FALSE, CD := FALSE, R := FALSE, LD := load_input, PV := 7, QU => qu_out, CV => cv_out);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Load PV into CV
        vm.write_variable(1, 1).unwrap(); // load_input = TRUE
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(3).unwrap(), 7, "CV should be PV (7)");
        assert_eq!(
            vm.read_variable(2).unwrap(),
            1,
            "QU should be TRUE (CV >= PV)"
        );
    }
}

#[test]
fn end_to_end_when_two_ctud_counters_then_independent() {
    let source = "
PROGRAM main
  VAR
    c1 : CTUD;
    c2 : CTUD;
    pulse : BOOL;
    qu1 : BOOL;
    qu2 : BOOL;
    cv1 : INT;
    cv2 : INT;
  END_VAR
  c1(CU := pulse, CD := FALSE, R := FALSE, LD := FALSE, PV := 2, QU => qu1, CV => cv1);
  c2(CU := pulse, CD := FALSE, R := FALSE, LD := FALSE, PV := 5, QU => qu2, CV => cv2);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Two rising edges
        for _ in 0..2 {
            vm.write_variable(2, 1).unwrap(); // pulse = TRUE
            vm.run_round(0).unwrap();
            vm.write_variable(2, 0).unwrap();
            vm.run_round(0).unwrap();
        }

        // c1 has PV=2, c2 has PV=5, both have CV=2
        assert_eq!(vm.read_variable(5).unwrap(), 2, "cv1 should be 2");
        assert_eq!(vm.read_variable(6).unwrap(), 2, "cv2 should be 2");
        assert_eq!(
            vm.read_variable(3).unwrap(),
            1,
            "qu1 should be TRUE (2 >= 2)"
        );
        assert_eq!(
            vm.read_variable(4).unwrap(),
            0,
            "qu2 should be FALSE (2 < 5)"
        );
    }
}
