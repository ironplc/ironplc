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

use core::ops::{Deref, DerefMut};

use crate::error::Trap;
use crate::{Clock, FaultContext, Vm, VmBuffers, VmFaulted, VmRunning, VmStopped};
use ironplc_container::{Container, VarIndex};

pub use ironplc_container::test_support::*;

/// A [`Clock`] driven by the test instead of by wall time.
///
/// Each reading returns the current time and then advances it by a fixed
/// step, so a task timed by two readings measures exactly that step however
/// long the host took. The default step is 0: every task measures as taking
/// no time, so the watchdog fires only in a test that asks for it.
#[derive(Clone, Copy, Debug, Default)]
pub struct ManualClock {
    now_us: u64,
    step_us: u64,
}

impl ManualClock {
    /// A clock, starting at 0, that each reading advances by `step_us`.
    pub fn stepping(step_us: u64) -> Self {
        ManualClock { now_us: 0, step_us }
    }
}

impl Clock for ManualClock {
    fn now_us(&mut self) -> u64 {
        let now = self.now_us;
        self.now_us = self.now_us.saturating_add(self.step_us);
        now
    }
}

/// A started VM that owns the clock its rounds are timed with.
///
/// [`VmRunning::run_round`] takes the [`Clock`] that times each task. Most
/// tests are not about that timing, so this wrapper supplies its own
/// [`clock`](Self::clock) and offers `run_round(uptime_us)`; every other
/// `VmRunning` method is reached through `Deref`. A test about timing sets
/// the clock, for example to [`ManualClock::stepping`].
pub struct TestVm<'a> {
    vm: VmRunning<'a>,
    /// The clock the next round is timed with.
    pub clock: ManualClock,
}

impl<'a> TestVm<'a> {
    /// Runs one scheduling round at `uptime_us`, timed by
    /// [`clock`](Self::clock).
    pub fn run_round(&mut self, uptime_us: u64) -> Result<(), FaultContext> {
        self.vm.run_round(uptime_us, &mut self.clock)
    }

    /// [`VmRunning::stop`], which consumes the VM and so cannot be reached
    /// through `Deref`.
    pub fn stop(self) -> VmStopped<'a> {
        self.vm.stop()
    }

    /// [`VmRunning::fault`], which consumes the VM and so cannot be reached
    /// through `Deref`.
    pub fn fault(self, ctx: FaultContext) -> VmFaulted<'a> {
        self.vm.fault(ctx)
    }
}

impl<'a> Deref for TestVm<'a> {
    type Target = VmRunning<'a>;

    fn deref(&self) -> &VmRunning<'a> {
        &self.vm
    }
}

impl<'a> DerefMut for TestVm<'a> {
    fn deref_mut(&mut self) -> &mut VmRunning<'a> {
        &mut self.vm
    }
}

/// Loads a container into the VM using the given buffers and starts execution.
///
/// This centralizes the `.load()` call so that adding new buffer parameters
/// only requires updating this one function instead of every test file. The
/// returned VM is timed by a [`ManualClock`] that measures every task as
/// taking no time.
pub fn load_and_start<'a>(
    container: &'a Container,
    bufs: &'a mut VmBuffers,
) -> Result<TestVm<'a>, FaultContext> {
    let vm = Vm::new()
        .load(container, bufs)
        .expect("container call depth fits the buffer")
        .start()?;
    Ok(TestVm {
        vm,
        clock: ManualClock::default(),
    })
}

/// Asserts that a run_round produces a specific trap.
pub fn assert_trap(vm: &mut TestVm, expected: Trap) {
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
