//! Debugger engine: breakpoints, stepping, and the pause/resume driver.
//!
//! This module turns the VM's instruction-level [`DebugHook`] into a
//! debugger-grade engine that can pause at breakpoints, single-step, and
//! leave the frame stack intact for inspection — all in `(FunctionId,
//! bytecode_offset)` space, with no dependency on source-line debug info.
//!
//! It is deliberately single-threaded: the [`BreakpointTable`] is a plain
//! sorted `Vec` owned and mutated directly by the caller (the Phase 4 DAP
//! loop). There are no atomics, no `ArcSwap`, and no cross-thread pause.
//!
//! [`DebugHook`]: crate::debug_hook::DebugHook

/// Stable identifier for a breakpoint, handed out by [`BreakpointTable`].
///
/// The value is opaque; callers use it to disable or remove a specific
/// breakpoint and to recognise which breakpoint a [`PauseReason::Breakpoint`]
/// refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BreakpointId(pub u32);

/// Why the VM stopped before executing the next instruction.
///
/// A trap is *not* a pause reason: traps continue to surface through the
/// existing fault path ([`FaultContext`](crate::FaultContext)), not here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseReason {
    /// Stopped because execution reached an enabled breakpoint.
    Breakpoint(BreakpointId),
    /// Stopped because a single-step (`over` / `in` / `out`) landed.
    Step,
    /// Stopped on entry, before executing the first instruction.
    Entry,
}
