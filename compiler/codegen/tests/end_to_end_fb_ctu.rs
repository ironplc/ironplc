//! End-to-end tests for function block invocation (CTU count up counter).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! a CTU function block instance, compile to bytecode, and execute on the VM.

mod common;

use common::{parse, VmBuffers};
use ironplc_codegen::compile;
use ironplc_vm::test_support::load_and_start;

#[test]
fn end_to_end_when_ctu_not_triggered_then_q_is_false() {
    let source = "
PROGRAM main
  VAR
    counter : CTU;
    result : BOOL;
  END_VAR
  counter(CU := FALSE, R := FALSE, PV := 3, Q => result);
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
        0,
        "Q should be FALSE when CU is FALSE"
    );
}

#[test]
fn end_to_end_when_ctu_counts_to_pv_then_q_is_true() {
    let source = "
PROGRAM main
  VAR
    counter : CTU;
    pulse : BOOL;
    result : BOOL;
    count : INT;
  END_VAR
  counter(CU := pulse, R := FALSE, PV := 3, Q => result, CV => count);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // 3 rising edges: pulse FALSE->TRUE for each
        for i in 0..3 {
            // pulse = TRUE
            vm.write_variable(1, 1).unwrap();
            vm.run_round(i * 2).unwrap();
            // pulse = FALSE
            vm.write_variable(1, 0).unwrap();
            vm.run_round(i * 2 + 1).unwrap();
        }

        assert_eq!(
            vm.read_variable(2).unwrap(),
            1,
            "Q should be TRUE after 3 counts"
        );
        assert_eq!(vm.read_variable(3).unwrap(), 3, "CV should be 3");
    }
}

#[test]
fn end_to_end_when_ctu_reset_then_cv_is_zero() {
    let source = "
PROGRAM main
  VAR
    counter : CTU;
    pulse : BOOL;
    reset : BOOL;
    result : BOOL;
    count : INT;
  END_VAR
  counter(CU := pulse, R := reset, PV := 5, Q => result, CV => count);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Count up twice
        vm.write_variable(1, 1).unwrap();
        vm.run_round(0).unwrap();
        vm.write_variable(1, 0).unwrap();
        vm.run_round(1).unwrap();
        vm.write_variable(1, 1).unwrap();
        vm.run_round(2).unwrap();
        vm.write_variable(1, 0).unwrap();
        vm.run_round(3).unwrap();
        assert_eq!(
            vm.read_variable(4).unwrap(),
            2,
            "CV should be 2 after 2 pulses"
        );

        // Reset
        vm.write_variable(2, 1).unwrap();
        vm.run_round(4).unwrap();
        assert_eq!(
            vm.read_variable(4).unwrap(),
            0,
            "CV should be 0 after reset"
        );
        assert_eq!(
            vm.read_variable(3).unwrap(),
            0,
            "Q should be FALSE after reset"
        );
    }
}

#[test]
fn end_to_end_when_ctu_below_pv_then_q_is_false() {
    let source = "
PROGRAM main
  VAR
    counter : CTU;
    pulse : BOOL;
    result : BOOL;
  END_VAR
  counter(CU := pulse, R := FALSE, PV := 5, Q => result);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // Count up twice (below PV=5)
        vm.write_variable(1, 1).unwrap();
        vm.run_round(0).unwrap();
        vm.write_variable(1, 0).unwrap();
        vm.run_round(1).unwrap();
        vm.write_variable(1, 1).unwrap();
        vm.run_round(2).unwrap();

        assert_eq!(
            vm.read_variable(2).unwrap(),
            0,
            "Q should be FALSE when CV < PV"
        );
    }
}

#[test]
fn end_to_end_when_ctu_dint_variant_then_compiles_and_runs() {
    let source = "
PROGRAM main
  VAR
    counter : CTU_DINT;
    result : BOOL;
    count : DINT;
  END_VAR
  counter(CU := TRUE, R := FALSE, PV := 1, Q => result, CV => count);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(1).unwrap(), 1, "Q should be TRUE");
        assert_eq!(vm.read_variable(2).unwrap(), 1, "CV should be 1");
    }
}
