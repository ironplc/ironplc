//! End-to-end tests for function block invocation (CTUD count up/down counter).
//!
//! These tests verify the complete pipeline: parse IEC 61131-3 source with
//! a CTUD function block instance, compile to bytecode, and execute on the VM.

mod common;

use common::{parse, VmBuffers};
use ironplc_codegen::compile;
use ironplc_vm::test_support::load_and_start;

#[test]
fn end_to_end_when_ctud_not_triggered_then_outputs_at_defaults() {
    let source = "
PROGRAM main
  VAR
    counter : CTUD;
    qu_out : BOOL;
    qd_out : BOOL;
  END_VAR
  counter(CU := FALSE, CD := FALSE, R := FALSE, LD := FALSE, PV := 5, QU => qu_out, QD => qd_out);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();
        vm.run_round(0).unwrap();
    }
    // CV=0, PV=5: QU = (0 >= 5) = FALSE, QD = (0 <= 0) = TRUE
    assert_eq!(bufs.vars[1].as_i32(), 0, "QU should be FALSE");
    assert_eq!(bufs.vars[2].as_i32(), 1, "QD should be TRUE (CV=0 <= 0)");
}

#[test]
fn end_to_end_when_ctud_counts_up_to_pv_then_qu_is_true() {
    let source = "
PROGRAM main
  VAR
    counter : CTUD;
    cu_in : BOOL;
    qu_out : BOOL;
    cv_out : INT;
  END_VAR
  counter(CU := cu_in, CD := FALSE, R := FALSE, LD := FALSE, PV := 3, QU => qu_out, CV => cv_out);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // 3 rising edges on CU
        for i in 0..3 {
            vm.write_variable(1, 1).unwrap();
            vm.run_round(i * 2).unwrap();
            vm.write_variable(1, 0).unwrap();
            vm.run_round(i * 2 + 1).unwrap();
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
fn end_to_end_when_ctud_counts_down_then_qd_is_true() {
    let source = "
PROGRAM main
  VAR
    counter : CTUD;
    cd_in : BOOL;
    qd_out : BOOL;
    cv_out : INT;
  END_VAR
  counter(CU := FALSE, CD := cd_in, R := FALSE, LD := FALSE, PV := 5, QD => qd_out, CV => cv_out);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        // CV starts at 0, counting down goes to -1
        vm.write_variable(1, 1).unwrap();
        vm.run_round(0).unwrap();

        assert_eq!(vm.read_variable(3).unwrap(), -1, "CV should be -1");
        assert_eq!(
            vm.read_variable(2).unwrap(),
            1,
            "QD should be TRUE (CV <= 0)"
        );
    }
}

#[test]
fn end_to_end_when_ctud_reset_then_cv_is_zero() {
    let source = "
PROGRAM main
  VAR
    counter : CTUD;
    cu_in : BOOL;
    reset : BOOL;
    cv_out : INT;
  END_VAR
  counter(CU := cu_in, CD := FALSE, R := reset, LD := FALSE, PV := 5, CV => cv_out);
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
            vm.read_variable(3).unwrap(),
            2,
            "CV should be 2 after 2 counts"
        );

        // Reset
        vm.write_variable(2, 1).unwrap();
        vm.run_round(4).unwrap();
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
    load : BOOL;
    cv_out : INT;
  END_VAR
  counter(CU := FALSE, CD := FALSE, R := FALSE, LD := load, PV := 7, CV => cv_out);
END_PROGRAM
";
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();

        vm.write_variable(1, 1).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(
            vm.read_variable(2).unwrap(),
            7,
            "CV should be PV (7) after load"
        );
    }
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
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs).unwrap();
        vm.run_round(0).unwrap();
        assert_eq!(vm.read_variable(1).unwrap(), 1, "QU should be TRUE");
        assert_eq!(vm.read_variable(2).unwrap(), 1, "CV should be 1");
    }
}
