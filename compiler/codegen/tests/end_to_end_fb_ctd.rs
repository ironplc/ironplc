//! End-to-end tests for function block invocation (CTD count down counter).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! a CTD function block instance, compile to bytecode, and execute on the VM.

mod common;

use common::{parse, VmBuffers};
use ironplc_codegen::compile;
use ironplc_vm::test_support::load_and_start;

#[test]
fn end_to_end_when_ctd_not_loaded_then_q_is_true() {
    // CV starts at 0, Q = (CV <= 0) should be TRUE
    let source = "
PROGRAM main
  VAR
    counter : CTD;
    result : BOOL;
  END_VAR
  counter(CD := FALSE, LD := FALSE, PV := 5, Q => result);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();
        vm.run_round(0).unwrap();
    }
    assert_eq!(bufs.vars[1].as_i32(), 1, "Q should be TRUE when CV <= 0");
}

#[test]
fn end_to_end_when_ctd_loaded_then_cv_equals_pv() {
    let source = "
PROGRAM main
  VAR
    counter : CTD;
    result : BOOL;
    count : INT;
  END_VAR
  counter(CD := FALSE, LD := TRUE, PV := 5, Q => result, CV => count);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(2).unwrap(),
            5,
            "CV should be PV (5) after load"
        );
        assert_eq!(
            vm.read_variable(1).unwrap(),
            0,
            "Q should be FALSE when CV > 0"
        );
    }
}

#[test]
fn end_to_end_when_ctd_counts_to_zero_then_q_is_true() {
    let source = "
PROGRAM main
  VAR
    counter : CTD;
    pulse : BOOL;
    load : BOOL;
    result : BOOL;
    count : INT;
  END_VAR
  counter(CD := pulse, LD := load, PV := 3, Q => result, CV => count);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Load PV
        vm.write_variable(2, 1).unwrap();
        vm.run_round(0).unwrap();
        vm.write_variable(2, 0).unwrap();
        vm.run_round(1).unwrap();
        assert_eq!(vm.read_variable(4).unwrap(), 3, "CV should be 3 after load");

        // Count down 3 times
        for i in 0..3 {
            vm.write_variable(1, 1).unwrap();
            vm.run_round((i + 1) * 2).unwrap();
            vm.write_variable(1, 0).unwrap();
            vm.run_round((i + 1) * 2 + 1).unwrap();
        }

        assert_eq!(
            vm.read_variable(4).unwrap(),
            0,
            "CV should be 0 after 3 counts"
        );
        assert_eq!(
            vm.read_variable(3).unwrap(),
            1,
            "Q should be TRUE when CV <= 0"
        );
    }
}

#[test]
fn end_to_end_when_ctd_above_zero_then_q_is_false() {
    let source = "
PROGRAM main
  VAR
    counter : CTD;
    pulse : BOOL;
    load : BOOL;
    result : BOOL;
    count : INT;
  END_VAR
  counter(CD := pulse, LD := load, PV := 5, Q => result, CV => count);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Load PV=5
        vm.write_variable(2, 1).unwrap();
        vm.run_round(0).unwrap();
        vm.write_variable(2, 0).unwrap();
        vm.run_round(1).unwrap();

        // Count down once (CV=4)
        vm.write_variable(1, 1).unwrap();
        vm.run_round(2).unwrap();

        assert_eq!(
            vm.read_variable(3).unwrap(),
            0,
            "Q should be FALSE when CV > 0"
        );
        assert_eq!(vm.read_variable(4).unwrap(), 4, "CV should be 4");
    }
}

#[test]
fn end_to_end_when_ctd_dint_variant_then_compiles_and_runs() {
    let source = "
PROGRAM main
  VAR
    counter : CTD_DINT;
    result : BOOL;
    count : DINT;
  END_VAR
  counter(CD := FALSE, LD := TRUE, PV := 10, Q => result, CV => count);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(2).unwrap(),
            10,
            "CV should be 10 after load"
        );
    }
}
