//! End-to-end tests for function block invocation (CTU count up counter).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! a CTU function block instance, compile to bytecode, and execute on the VM.

mod common;
use ironplc_container::VarIndex;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;
use common::parse_and_run_rounds;

const CTU_PROGRAM: &str = "
PROGRAM main
  VAR
    counter : CTU;
    pulse : BOOL;
    reset : BOOL;
    result : BOOL;
    count : INT;
  END_VAR
  counter(CU := pulse, R := reset, PV := 3, Q => result, CV => count);
END_PROGRAM
";

/// Generates `n` rising edges on variable `var_idx` (pulse TRUE then FALSE).
fn pulse_n(vm: &mut ironplc_vm::VmRunning<'_>, var_idx: u16, n: u64) {
    for i in 0..n {
        vm.write_variable(VarIndex::new(var_idx), 1).unwrap();
        vm.run_round(i * 2).unwrap();
        vm.write_variable(VarIndex::new(var_idx), 0).unwrap();
        vm.run_round(i * 2 + 1).unwrap();
    }
}

#[test]
fn end_to_end_when_ctu_not_triggered_then_q_is_false() {
    let (_container, bufs) = parse_and_run(CTU_PROGRAM, &CompilerOptions::default());
    assert_eq!(
        bufs.vars[3].as_i32(),
        0,
        "Q should be FALSE when CU is FALSE"
    );
}

#[test]
fn end_to_end_when_ctu_counts_to_pv_then_q_is_true() {
    parse_and_run_rounds(CTU_PROGRAM, &CompilerOptions::default(), |vm| {
        pulse_n(vm, 1, 3);
        assert_eq!(
            vm.read_variable(VarIndex::new(3)).unwrap(),
            1,
            "Q should be TRUE after 3 counts"
        );
        assert_eq!(
            vm.read_variable(VarIndex::new(4)).unwrap(),
            3,
            "CV should be 3"
        );
    });
}

#[test]
fn end_to_end_when_ctu_reset_then_cv_is_zero() {
    parse_and_run_rounds(CTU_PROGRAM, &CompilerOptions::default(), |vm| {
        pulse_n(vm, 1, 2);
        assert_eq!(
            vm.read_variable(VarIndex::new(4)).unwrap(),
            2,
            "CV should be 2 after 2 pulses"
        );

        // Reset
        vm.write_variable(VarIndex::new(2), 1).unwrap();
        vm.run_round(4).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(4)).unwrap(),
            0,
            "CV should be 0 after reset"
        );
        assert_eq!(
            vm.read_variable(VarIndex::new(3)).unwrap(),
            0,
            "Q should be FALSE after reset"
        );
    });
}

#[test]
fn end_to_end_when_ctu_below_pv_then_q_is_false() {
    parse_and_run_rounds(CTU_PROGRAM, &CompilerOptions::default(), |vm| {
        pulse_n(vm, 1, 2);
        // PV=3, CV=2 => Q should be FALSE
        assert_eq!(
            vm.read_variable(VarIndex::new(3)).unwrap(),
            0,
            "Q should be FALSE when CV < PV"
        );
    });
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
    parse_and_run_rounds(source, &CompilerOptions::default(), |vm| {
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(1)).unwrap(),
            1,
            "Q should be TRUE"
        );
        assert_eq!(
            vm.read_variable(VarIndex::new(2)).unwrap(),
            1,
            "CV should be 1"
        );
    });
}
