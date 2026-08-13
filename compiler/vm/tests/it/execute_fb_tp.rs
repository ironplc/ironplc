//! VM-specific edge case tests for the TP pulse timer intrinsic.
//!
//! The nominal TP matrix — pulse start, before/after/at PT, ET readout, IN
//! falling mid-pulse (with ET clamped to PT), and two independent timers — is
//! covered by end_to_end_fb_tp.rs, which drives the same `intrinsic.rs` state
//! machine through compiled ST. These tests keep only what that file does not
//! assert: the cold-start case where IN was never TRUE, and retriggering a new
//! pulse after the previous one completed.

use crate::common::VmBuffers;
use ironplc_container::opcode;
use ironplc_container::VarIndex;

#[test]
fn tp_when_in_false_then_q_false_et_zero() {
    let c = crate::common::timer_test_container(5000, opcode::fb_type::TP);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    // var[1] = IN = 0 (FALSE) — default
    vm.run_round(1_000_000).unwrap(); // t = 1s

    assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 0); // Q = FALSE
    assert_eq!(vm.read_variable(VarIndex::new(3)).unwrap(), 0); // ET = 0
}

#[test]
fn tp_when_retrigger_after_pulse_then_new_pulse() {
    let c = crate::common::timer_test_container(5000, opcode::fb_type::TP);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();

    // First pulse
    vm.write_variable(VarIndex::new(1), 1).unwrap(); // IN = TRUE
    vm.run_round(1_000_000).unwrap(); // t=1s: rising edge
    vm.run_round(7_000_000).unwrap(); // t=7s: pulse expired

    assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 0); // Q = FALSE

    // IN must go FALSE then TRUE for new rising edge
    vm.write_variable(VarIndex::new(1), 0).unwrap();
    vm.run_round(8_000_000).unwrap(); // t=8s: IN = FALSE

    vm.write_variable(VarIndex::new(1), 1).unwrap();
    vm.run_round(9_000_000).unwrap(); // t=9s: new rising edge

    assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 1); // Q = TRUE (new pulse)
    assert_eq!(vm.read_variable(VarIndex::new(3)).unwrap(), 0); // ET = 0 (just started)

    // New pulse timing
    vm.run_round(12_000_000).unwrap(); // t=12s: 3s into new pulse
    assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 1); // Q = TRUE
    assert_eq!(vm.read_variable(VarIndex::new(3)).unwrap(), 3000); // ET = 3s = 3000 ms
}
