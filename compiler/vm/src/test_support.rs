//! Shared test helpers for VM integration tests.
//!
//! This module is only available when the `test-support` feature is enabled
//! or during `cargo test`. It provides VM loading and common assertion helpers
//! used by both `ironplc-vm` and `ironplc-codegen` test suites.

use crate::error::Trap;
use crate::{FaultContext, Vm, VmBuffers, VmRunning};
use ironplc_container::Container;

/// Loads a container into the VM using the given buffers and starts execution.
///
/// This centralizes the `.load()` call so that adding new buffer parameters
/// only requires updating this one function instead of every test file.
pub fn load_and_start<'a>(
    container: &'a Container,
    bufs: &'a mut VmBuffers,
) -> Result<VmRunning<'a>, FaultContext> {
    Vm::new().load(container, bufs).start()
}

/// Asserts that a run_round produces a specific trap.
pub fn assert_trap(vm: &mut VmRunning, expected: Trap) {
    let result = vm.run_round(0);
    assert!(
        result.is_err(),
        "expected trap {expected} but run_round succeeded"
    );
    assert_eq!(result.unwrap_err().trap, expected);
}
