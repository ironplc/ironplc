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

use ironplc_container::FunctionId;

use crate::debug_hook::{DebugHook, HookAction};

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

/// One breakpoint: a `(function_id, offset)` location plus an enabled flag.
#[derive(Clone, Copy, Debug)]
struct BreakpointEntry {
    id: BreakpointId,
    function_id: FunctionId,
    offset: usize,
    enabled: bool,
}

impl BreakpointEntry {
    /// Sort/search key: function first (by raw id), then bytecode offset.
    fn key(&self) -> (u16, usize) {
        (self.function_id.raw(), self.offset)
    }
}

/// Set of pause-only breakpoints, keyed by `(function_id, bytecode_offset)`.
///
/// Entries are kept sorted so a per-instruction lookup is a binary search.
/// This is deliberately a plain `Vec` with no atomics or `ArcSwap`: the
/// single-threaded debug loop owns and mutates it directly.
#[derive(Debug, Default)]
pub struct BreakpointTable {
    entries: Vec<BreakpointEntry>,
    next_id: u32,
}

impl BreakpointTable {
    /// Create an empty table.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 0,
        }
    }

    /// Number of breakpoints (enabled or not).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the table holds no breakpoints.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Add an enabled breakpoint at `(function_id, offset)`, returning its id.
    ///
    /// Duplicate locations are allowed; [`lookup`](Self::lookup) reports the
    /// first enabled breakpoint at a location.
    pub fn add(&mut self, function_id: FunctionId, offset: usize) -> BreakpointId {
        let id = BreakpointId(self.next_id);
        self.next_id += 1;
        let entry = BreakpointEntry {
            id,
            function_id,
            offset,
            enabled: true,
        };
        let pos = self.entries.partition_point(|e| e.key() < entry.key());
        self.entries.insert(pos, entry);
        id
    }

    /// Remove the breakpoint with `id`. Returns whether it existed.
    pub fn remove(&mut self, id: BreakpointId) -> bool {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            self.entries.remove(pos);
            true
        } else {
            false
        }
    }

    /// Enable or disable the breakpoint with `id`. Returns whether it existed.
    pub fn set_enabled(&mut self, id: BreakpointId, enabled: bool) -> bool {
        if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
            e.enabled = enabled;
            true
        } else {
            false
        }
    }

    /// Remove every breakpoint (ids are not reused).
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// The id of the first enabled breakpoint at `(function_id, offset)`, or
    /// `None`.
    pub fn lookup(&self, function_id: FunctionId, offset: usize) -> Option<BreakpointId> {
        let key = (function_id.raw(), offset);
        // Binary search to any entry at this key, then scan the equal run for
        // an enabled one (duplicates at a location are permitted).
        let mut idx = self.entries.partition_point(|e| e.key() < key);
        while idx < self.entries.len() && self.entries[idx].key() == key {
            if self.entries[idx].enabled {
                return Some(self.entries[idx].id);
            }
            idx += 1;
        }
        None
    }
}

/// Single-step mode requested of the [`DebuggerHook`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepMode {
    /// Not stepping.
    None,
    /// Step to the next instruction at the same (or a shallower) call depth,
    /// running any called functions to completion.
    Over,
    /// Step to the very next instruction executed, descending into calls.
    In,
    /// Run until the current function returns, then stop in the caller.
    Out,
}

/// Remembers where a step started so the hook can tell when it has "landed".
///
/// Depth is the debugger's own mirror of the call depth (relative to scan
/// entry), tracked via `before_call` / `after_return` — it never reaches
/// into the VM's frame stack.
#[derive(Clone, Copy, Debug)]
struct StepController {
    mode: StepMode,
    origin_depth: usize,
    origin_offset: usize,
}

impl StepController {
    fn idle() -> Self {
        Self {
            mode: StepMode::None,
            origin_depth: 0,
            origin_offset: 0,
        }
    }

    /// Whether a step in progress has landed at `(depth, offset)`.
    fn landed(&self, depth: usize, offset: usize) -> bool {
        // The origin instruction itself is never a landing — a step must
        // make forward progress.
        let at_origin = depth == self.origin_depth && offset == self.origin_offset;
        match self.mode {
            StepMode::None => false,
            // Same or shallower depth (calls stepped over), but not the
            // origin instruction.
            StepMode::Over => depth <= self.origin_depth && !at_origin,
            // Any next instruction, including the first of a callee.
            StepMode::In => !at_origin,
            // Only once control has unwound past the origin frame.
            StepMode::Out => depth < self.origin_depth,
        }
    }
}

/// The debugger's [`DebugHook`]: pauses at enabled breakpoints, single-steps
/// (over / in / out), and leaves the frame stack intact for inspection.
///
/// Borrows the [`BreakpointTable`] so the owning (single-threaded) loop can
/// consult and mutate it between rounds. After the hook reports a pause it
/// suppresses that exact breakpoint for the immediately-following
/// instruction, so a `continue`/resume or the first step off the current
/// location does not re-trigger the same breakpoint in place.
pub struct DebuggerHook<'a> {
    breakpoints: &'a BreakpointTable,
    /// When set, the next instruction skips the breakpoint check exactly
    /// once. Set on every pause so resume makes forward progress.
    skip_breakpoint_once: bool,
    /// Call depth relative to scan entry: `+1` per call, `-1` per return.
    /// Self-heals to 0 at each scan boundary (the entry-frame return uses a
    /// saturating decrement).
    depth: usize,
    /// Location observed at the most recent `before_instruction`, used as a
    /// step's origin when one is armed while paused.
    last_offset: usize,
    step: StepController,
}

impl<'a> DebuggerHook<'a> {
    /// Create a debugger hook over `breakpoints` for a fresh debug session.
    pub fn new(breakpoints: &'a BreakpointTable) -> Self {
        Self {
            breakpoints,
            skip_breakpoint_once: false,
            depth: 0,
            last_offset: 0,
            step: StepController::idle(),
        }
    }

    /// Arm a step-over from the current (paused) location: run to the next
    /// instruction at the same or a shallower call depth.
    pub fn step_over(&mut self) {
        self.arm(StepMode::Over);
    }

    /// Arm a step-in from the current (paused) location: stop at the very
    /// next instruction, descending into any call.
    pub fn step_in(&mut self) {
        self.arm(StepMode::In);
    }

    /// Arm a step-out from the current (paused) location: run until the
    /// current function returns, then stop in the caller.
    pub fn step_out(&mut self) {
        self.arm(StepMode::Out);
    }

    fn arm(&mut self, mode: StepMode) {
        self.step = StepController {
            mode,
            origin_depth: self.depth,
            origin_offset: self.last_offset,
        };
    }
}

impl DebugHook for DebuggerHook<'_> {
    fn before_instruction(&mut self, function_id: FunctionId, pc: usize, _op: u8) -> HookAction {
        self.last_offset = pc;
        let skip = self.skip_breakpoint_once;
        self.skip_breakpoint_once = false;
        if !skip {
            if let Some(id) = self.breakpoints.lookup(function_id, pc) {
                // Suppress this breakpoint for the resume instruction.
                self.skip_breakpoint_once = true;
                return HookAction::Pause(PauseReason::Breakpoint(id));
            }
        }
        if self.step.landed(self.depth, pc) {
            // A step lands only once; disarm and suppress a co-located
            // breakpoint on the resume instruction.
            self.step.mode = StepMode::None;
            self.skip_breakpoint_once = true;
            return HookAction::Pause(PauseReason::Step);
        }
        HookAction::Continue
    }

    fn before_call(&mut self, _callee: FunctionId) {
        self.depth += 1;
    }

    fn after_return(&mut self, _returning_to: Option<FunctionId>) {
        self.depth = self.depth.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakpoint_table_when_empty_then_lookup_misses() {
        let table = BreakpointTable::new();
        assert!(table.is_empty());
        assert_eq!(table.lookup(FunctionId::SCAN, 0), None);
    }

    #[test]
    fn breakpoint_table_when_added_then_lookup_hits_exact_location() {
        let mut table = BreakpointTable::new();
        let id = table.add(FunctionId::SCAN, 6);
        assert_eq!(table.lookup(FunctionId::SCAN, 6), Some(id));
        // Different offset / function does not match.
        assert_eq!(table.lookup(FunctionId::SCAN, 5), None);
        assert_eq!(table.lookup(FunctionId::new(2), 6), None);
    }

    #[test]
    fn breakpoint_table_when_disabled_then_lookup_misses() {
        let mut table = BreakpointTable::new();
        let id = table.add(FunctionId::SCAN, 3);
        assert!(table.set_enabled(id, false));
        assert_eq!(table.lookup(FunctionId::SCAN, 3), None);
        assert!(table.set_enabled(id, true));
        assert_eq!(table.lookup(FunctionId::SCAN, 3), Some(id));
    }

    #[test]
    fn breakpoint_table_when_removed_then_lookup_misses() {
        let mut table = BreakpointTable::new();
        let id = table.add(FunctionId::new(2), 9);
        assert!(table.remove(id));
        assert!(!table.remove(id));
        assert_eq!(table.lookup(FunctionId::new(2), 9), None);
    }

    #[test]
    fn breakpoint_table_when_many_functions_then_sorted_lookup_works() {
        let mut table = BreakpointTable::new();
        // Insert out of order across functions and offsets.
        let a = table.add(FunctionId::new(5), 10);
        let b = table.add(FunctionId::SCAN, 2);
        let c = table.add(FunctionId::new(2), 100);
        let d = table.add(FunctionId::SCAN, 0);
        assert_eq!(table.lookup(FunctionId::new(5), 10), Some(a));
        assert_eq!(table.lookup(FunctionId::SCAN, 2), Some(b));
        assert_eq!(table.lookup(FunctionId::new(2), 100), Some(c));
        assert_eq!(table.lookup(FunctionId::SCAN, 0), Some(d));
        assert_eq!(table.len(), 4);
    }
}
