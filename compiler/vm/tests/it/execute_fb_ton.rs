//! VM-specific edge case tests for the TON on-delay timer intrinsic.
//!
//! The nominal TON matrix — IN false, before/after/at PT, ET readout, reset
//! and restart, two independent timers, and every dot-access form — is
//! covered by end_to_end_fb_ton.rs, which drives the same `intrinsic.rs`
//! state machine through compiled ST. These tests keep only what that file
//! does not assert: ET clamping at PT, and resetting out of the expired
//! (Q=TRUE) state rather than out of the still-timing state.

use crate::common::VmBuffers;
use ironplc_container::opcode;
use ironplc_container::VarIndex;

#[test]
fn ton_when_in_true_after_pt_then_q_true_et_clamped() {
    let c = crate::common::timer_test_container(5000, opcode::fb_type::TON);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();

    vm.write_variable(VarIndex::new(1), 1).unwrap(); // IN = TRUE

    vm.run_round(1_000_000).unwrap(); // t=1s: rising edge
    vm.run_round(7_000_000).unwrap(); // t=7s: 6s elapsed > 5s PT

    assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 1); // Q = TRUE
    assert_eq!(vm.read_variable(VarIndex::new(3)).unwrap(), 5000); // ET clamped to PT (5000 ms)
}

#[test]
fn ton_when_in_falls_after_timer_expires_then_resets() {
    let c = crate::common::timer_test_container(5000, opcode::fb_type::TON);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();

    vm.write_variable(VarIndex::new(1), 1).unwrap(); // IN = TRUE
    vm.run_round(1_000_000).unwrap(); // t=1s: rising edge
    vm.run_round(7_000_000).unwrap(); // t=7s: timer expired

    assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 1); // Q = TRUE

    // IN goes FALSE
    vm.write_variable(VarIndex::new(1), 0).unwrap();
    vm.run_round(8_000_000).unwrap();

    assert_eq!(vm.read_variable(VarIndex::new(2)).unwrap(), 0); // Q = FALSE
    assert_eq!(vm.read_variable(VarIndex::new(3)).unwrap(), 0); // ET = 0
}
