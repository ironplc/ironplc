//! Integration tests for the scan counter that `run_round` and
//! `run_round_debug` advance after each completed scan.
//!
//! An embedder can restore the counter from a previous session with
//! `VmReady::resume`, so it can start anywhere up to `u64::MAX`; the
//! counter saturates there rather than overflowing.

use ironplc_container::opcode;
use ironplc_vm::{NoopDebugHook, Vm};

use crate::common::{single_function_container, VmBuffers};

#[test]
fn run_round_when_resumed_at_max_scan_count_then_saturates() {
    let c = single_function_container(&[opcode::RET_VOID], 0, &[]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new().load(&c, &mut b).unwrap().resume(u64::MAX);

    vm.run_round(0).unwrap();

    assert_eq!(vm.scan_count(), u64::MAX);
}

#[test]
fn run_round_debug_when_resumed_at_max_scan_count_then_saturates() {
    let c = single_function_container(&[opcode::RET_VOID], 0, &[]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new().load(&c, &mut b).unwrap().resume(u64::MAX);

    vm.run_round_debug(0, &mut NoopDebugHook).unwrap();

    assert_eq!(vm.scan_count(), u64::MAX);
}

#[test]
fn run_round_when_resumed_below_max_then_increments() {
    let c = single_function_container(&[opcode::RET_VOID], 0, &[]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new().load(&c, &mut b).unwrap().resume(u64::MAX - 1);

    vm.run_round(0).unwrap();

    assert_eq!(vm.scan_count(), u64::MAX);
}
