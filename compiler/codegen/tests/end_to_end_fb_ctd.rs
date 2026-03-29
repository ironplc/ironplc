//! End-to-end tests for function block invocation (CTD count down counter).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! a CTD function block instance, compile to bytecode, and execute on the VM.

mod common;
use ironplc_container::VarIndex;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run_rounds;

const CTD_PROGRAM: &str = "
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

/// Generates `n` rising edges on variable `var_idx` starting at `time_base`.
fn pulse_n(vm: &mut ironplc_vm::VmRunning<'_>, var_idx: u16, n: u64, time_base: u64) {
    for i in 0..n {
        vm.write_variable(VarIndex::new(var_idx), 1).unwrap();
        vm.run_round(time_base + i * 2).unwrap();
        vm.write_variable(VarIndex::new(var_idx), 0).unwrap();
        vm.run_round(time_base + i * 2 + 1).unwrap();
    }
}

#[test]
fn end_to_end_when_ctd_not_loaded_then_q_is_true() {
    // CV starts at 0, Q = (CV <= 0) should be TRUE
    parse_and_run_rounds(CTD_PROGRAM, &CompilerOptions::default(), |vm| {
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(3)).unwrap(),
            1,
            "Q should be TRUE when CV <= 0"
        );
    });
}

#[test]
fn end_to_end_when_ctd_loaded_then_cv_equals_pv() {
    parse_and_run_rounds(CTD_PROGRAM, &CompilerOptions::default(), |vm| {
        vm.write_variable(VarIndex::new(2), 1).unwrap(); // LD = TRUE
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(4)).unwrap(),
            3,
            "CV should be PV (3) after load"
        );
        assert_eq!(
            vm.read_variable(VarIndex::new(3)).unwrap(),
            0,
            "Q should be FALSE when CV > 0"
        );
    });
}

#[test]
fn end_to_end_when_ctd_counts_to_zero_then_q_is_true() {
    parse_and_run_rounds(CTD_PROGRAM, &CompilerOptions::default(), |vm| {
        // Load PV
        vm.write_variable(VarIndex::new(2), 1).unwrap();
        vm.run_round(0).unwrap();
        vm.write_variable(VarIndex::new(2), 0).unwrap();
        vm.run_round(1).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(4)).unwrap(),
            3,
            "CV should be 3 after load"
        );

        // Count down 3 times
        pulse_n(vm, 1, 3, 2);

        assert_eq!(
            vm.read_variable(VarIndex::new(4)).unwrap(),
            0,
            "CV should be 0 after 3 counts"
        );
        assert_eq!(
            vm.read_variable(VarIndex::new(3)).unwrap(),
            1,
            "Q should be TRUE when CV <= 0"
        );
    });
}

#[test]
fn end_to_end_when_ctd_above_zero_then_q_is_false() {
    parse_and_run_rounds(CTD_PROGRAM, &CompilerOptions::default(), |vm| {
        // Load PV=3
        vm.write_variable(VarIndex::new(2), 1).unwrap();
        vm.run_round(0).unwrap();
        vm.write_variable(VarIndex::new(2), 0).unwrap();
        vm.run_round(1).unwrap();

        // Count down once (CV=2)
        vm.write_variable(VarIndex::new(1), 1).unwrap();
        vm.run_round(2).unwrap();

        assert_eq!(
            vm.read_variable(VarIndex::new(3)).unwrap(),
            0,
            "Q should be FALSE when CV > 0"
        );
        assert_eq!(
            vm.read_variable(VarIndex::new(4)).unwrap(),
            2,
            "CV should be 2"
        );
    });
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
    parse_and_run_rounds(source, &CompilerOptions::default(), |vm| {
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(VarIndex::new(2)).unwrap(),
            10,
            "CV should be 10 after load"
        );
    });
}
