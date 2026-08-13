//! VM-specific edge case tests for the TOF off-delay timer intrinsic.
//!
//! The nominal TOF matrix — IN true, before/after/at PT following the falling
//! edge, ET readout, reset on a rising edge, and two independent timers — is
//! covered by end_to_end_fb_tof.rs, which drives the same `intrinsic.rs`
//! state machine through compiled ST. These tests keep only what that file
//! does not assert: ET clamping at PT, and the cold-start case where IN was
//! never TRUE (every codegen case drives IN TRUE first).

use crate::common::VmBuffers;
use ironplc_container::opcode;
use ironplc_container::VarIndex;

#[test]
fn tof_when_in_false_after_pt_then_q_false_et_clamped() {
    let c = crate::common::timer_test_container(5000, opcode::fb_type::TOF);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();

    // IN = TRUE first
    vm.write_variable(VarIndex::new(1), 1).unwrap();
    vm.run_round(1_000_000).unwrap(); // t=1s

    // IN falls
    vm.write_variable(VarIndex::new(1), 0).unwrap();
    vm.run_round(2_000_000).unwrap(); // t=2s: falling edge

    // After PT
    vm.run_round(8_000_000).unwrap(); // t=8s: 6s elapsed > 5s PT

    assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 0); // Q = FALSE
    assert_eq!(vm.read_variable(VarIndex::new(3)).unwrap(), 5000); // ET clamped to PT (5000 ms)
}

#[test]
fn tof_when_in_never_true_then_q_false() {
    let c = crate::common::timer_test_container(5000, opcode::fb_type::TOF);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();

    // IN = FALSE (default), no prior TRUE state
    vm.run_round(1_000_000).unwrap(); // t = 1s

    // First scan with IN=FALSE starts timing from "falling edge"
    // Q starts TRUE (off-delay holds Q=TRUE during timing)
    // After PT expires, Q goes FALSE
    vm.run_round(7_000_000).unwrap(); // t=7s: > 5s elapsed

    assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 0); // Q = FALSE after PT
}
