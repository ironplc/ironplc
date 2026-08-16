//! Shared test helpers for VM tests.
//!
//! This module is only available when the `test-support` feature is enabled
//! or during `cargo test`. It provides VM loading, execution shorthands and
//! common assertion helpers used by `ironplc-vm`, `ironplc-codegen` and the
//! benchmark crate.
//!
//! The container fixtures themselves live in `ironplc_container::test_support`
//! — `container` owns `ContainerBuilder`, and homing them here would force a
//! `container` → `vm` dev-dependency cycle. They are re-exported below so
//! VM-side callers have a single import surface.

use crate::error::Trap;
use crate::{FaultContext, Vm, VmBuffers, VmRunning};
use ironplc_container::{Container, VarIndex};

pub use ironplc_container::test_support::*;

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

/// Runs bytecode with i32 constants and returns var[0] as i32.
///
/// Shorthand for the common pattern: build container, allocate buffers,
/// load VM, execute one round, read variable 0.
pub fn run_and_read_i32(bytecode: &[u8], num_vars: u16, constants: &[i32]) -> i32 {
    let c = single_function_container(bytecode, num_vars, constants);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();
    vm.read_variable(VarIndex::new(0)).unwrap()
}

/// Runs bytecode with i64 constants and returns var[0] as i64.
pub fn run_and_read_i64(bytecode: &[u8], num_vars: u16, constants: &[i64]) -> i64 {
    let c = single_function_container_i64(bytecode, num_vars, constants);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    b.vars[0].as_i64()
}

/// Runs bytecode with f32 constants and returns var[0] as f32.
pub fn run_and_read_f32(bytecode: &[u8], num_vars: u16, constants: &[f32]) -> f32 {
    let c = single_function_container_f32(bytecode, num_vars, constants);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    b.vars[0].as_f32()
}

/// Runs bytecode with f64 constants and returns var[0] as f64.
pub fn run_and_read_f64(bytecode: &[u8], num_vars: u16, constants: &[f64]) -> f64 {
    let c = single_function_container_f64(bytecode, num_vars, constants);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    b.vars[0].as_f64()
}

/// Runs bytecode with i32 constants expecting a trap, returns the trap.
pub fn run_and_expect_trap_i32(bytecode: &[u8], num_vars: u16, constants: &[i32]) -> Trap {
    let c = single_function_container(bytecode, num_vars, constants);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap_err().trap
}
