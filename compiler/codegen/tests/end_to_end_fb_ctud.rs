//! End-to-end tests for function block invocation (CTUD count up/down counter).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! a CTUD function block instance, compile to bytecode, and execute on the VM.

mod common;
use ironplc_parser::options::CompilerOptions;

use common::parse_and_run;
use common::parse_and_run_rounds;

const CTUD_PROGRAM: &str = "
PROGRAM main
  VAR
    counter : CTUD;
    cu_in : BOOL;
    cd_in : BOOL;
    reset : BOOL;
    load : BOOL;
    qu_out : BOOL;
    qd_out : BOOL;
    cv_out : INT;
  END_VAR
  counter(CU := cu_in, CD := cd_in, R := reset, LD := load, PV := 3,
          QU => qu_out, QD => qd_out, CV => cv_out);
END_PROGRAM
";

/// Generates `n` rising edges on variable `var_idx` starting at `time_base`.
fn pulse_n(vm: &mut ironplc_vm::VmRunning<'_>, var_idx: u16, n: u64, time_base: u64) {
    for i in 0..n {
        vm.write_variable(var_idx, 1).unwrap();
        vm.run_round(time_base + i * 2).unwrap();
        vm.write_variable(var_idx, 0).unwrap();
        vm.run_round(time_base + i * 2 + 1).unwrap();
    }
}

#[test]
fn end_to_end_when_ctud_not_triggered_then_outputs_at_defaults() {
    // CV=0, PV=3: QU = (0 >= 3) = FALSE, QD = (0 <= 0) = TRUE
    let (_container, bufs) = parse_and_run(CTUD_PROGRAM, &CompilerOptions::default());
    assert_eq!(bufs.vars[5].as_i32(), 0, "QU should be FALSE");
    assert_eq!(bufs.vars[6].as_i32(), 1, "QD should be TRUE (CV=0 <= 0)");
}

#[test]
fn end_to_end_when_ctud_counts_up_to_pv_then_qu_is_true() {
    parse_and_run_rounds(CTUD_PROGRAM, &CompilerOptions::default(), |vm| {
        pulse_n(vm, 1, 3, 0); // 3 rising edges on CU
        assert_eq!(vm.read_variable(7).unwrap(), 3, "CV should be 3");
        assert_eq!(
            vm.read_variable(5).unwrap(),
            1,
            "QU should be TRUE (CV >= PV)"
        );
    });
}

#[test]
fn end_to_end_when_ctud_counts_down_then_qd_is_true() {
    parse_and_run_rounds(CTUD_PROGRAM, &CompilerOptions::default(), |vm| {
        // CV starts at 0, counting down goes to -1
        vm.write_variable(2, 1).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(7).unwrap(), -1, "CV should be -1");
        assert_eq!(
            vm.read_variable(6).unwrap(),
            1,
            "QD should be TRUE (CV <= 0)"
        );
    });
}

#[test]
fn end_to_end_when_ctud_reset_then_cv_is_zero() {
    parse_and_run_rounds(CTUD_PROGRAM, &CompilerOptions::default(), |vm| {
        pulse_n(vm, 1, 2, 0); // Count up twice
        assert_eq!(
            vm.read_variable(7).unwrap(),
            2,
            "CV should be 2 after 2 counts"
        );

        // Reset
        vm.write_variable(3, 1).unwrap();
        vm.run_round(4).unwrap();
        assert_eq!(
            vm.read_variable(7).unwrap(),
            0,
            "CV should be 0 after reset"
        );
    });
}

#[test]
fn end_to_end_when_ctud_load_then_cv_equals_pv() {
    parse_and_run_rounds(CTUD_PROGRAM, &CompilerOptions::default(), |vm| {
        vm.write_variable(4, 1).unwrap(); // LD = TRUE
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(7).unwrap(),
            3,
            "CV should be PV (3) after load"
        );
    });
}

#[test]
fn end_to_end_when_ctud_dint_variant_then_compiles_and_runs() {
    let source = "
PROGRAM main
  VAR
    counter : CTUD_DINT;
    qu_out : BOOL;
    cv_out : DINT;
  END_VAR
  counter(CU := TRUE, CD := FALSE, R := FALSE, LD := FALSE, PV := 1, QU => qu_out, CV => cv_out);
END_PROGRAM
";
    parse_and_run_rounds(source, &CompilerOptions::default(), |vm| {
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(1).unwrap(), 1, "QU should be TRUE");
        assert_eq!(vm.read_variable(2).unwrap(), 1, "CV should be 1");
    });
}
