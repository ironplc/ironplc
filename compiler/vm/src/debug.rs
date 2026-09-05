//! Debugger engine: breakpoints, stepping, and the pause/resume driver.
//!
//! This module turns the VM's instruction-level [`DebugHook`] into a
//! debugger-grade engine that can pause at breakpoints, single-step, and
//! leave the frame stack intact for inspection — all in `(FunctionId,
//! bytecode_offset)` space, with no dependency on source-line debug info.
//!
//! It is deliberately single-threaded: the [`BreakpointTable`] is a plain
//! sorted `Vec` owned and mutated directly by the caller (the DAP server
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
    /// Run to the end of the current scan cycle, then stop at the start of the
    /// next one.
    ///
    /// Unlike the other modes this one has no *intra*-scan landing: the stop is
    /// a round boundary, not an instruction, so the driver decides it. The
    /// [`DebuggerHook`] only reports that a scan step is in flight (see
    /// [`DebugHook::stepping_scan`](crate::debug_hook::DebugHook::stepping_scan))
    /// and `run_round_debug` turns a completed scan into
    /// [`RoundOutcome::PausedAfterScan`](crate::RoundOutcome::PausedAfterScan).
    Scan,
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
            // A scan step never lands inside the scan: it runs to the scan
            // boundary, which only the round driver can observe.
            StepMode::Scan => false,
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
    /// When set, the next instruction pauses with [`PauseReason::Entry`]
    /// exactly once, before any breakpoint or step check. The single-threaded
    /// driver arms this only on the first round of a `stopOnEntry` launch.
    stop_on_entry: bool,
    /// When set, the next instruction pauses with [`PauseReason::Step`] exactly
    /// once — the landing half of a scan step, armed by the driver on the round
    /// that follows the scan the step ran out. See [`land_scan_step`].
    ///
    /// [`land_scan_step`]: DebuggerHook::land_scan_step
    scan_landing: bool,
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
            stop_on_entry: false,
            scan_landing: false,
            depth: 0,
            last_offset: 0,
            step: StepController::idle(),
        }
    }

    /// Arm a one-shot entry pause: the next instruction pauses with
    /// [`PauseReason::Entry`] before executing, ahead of any breakpoint or
    /// step check.
    ///
    /// Honors the DAP `stopOnEntry` launch option. The single-threaded driver
    /// rebuilds the hook each round, so it arms this only on the first round;
    /// scan 2+ never re-arms and runs normally.
    pub fn stop_on_entry(&mut self) {
        self.stop_on_entry = true;
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

    /// Arm the *run* half of a scan step: finish the current scan cycle without
    /// stopping at any step landing, so the driver reports
    /// [`RoundOutcome::PausedAfterScan`](crate::RoundOutcome::PausedAfterScan)
    /// at the scan boundary.
    ///
    /// Breakpoints still fire during that scan, and abandon the step where they
    /// stop — the same way a breakpoint reached mid-`step_over` abandons that
    /// step. Because a scan step's stop is the *next* scan's first instruction
    /// (the frame stack has drained at the boundary itself, leaving nothing to
    /// inspect), the driver completes it with [`land_scan_step`] on the
    /// following round.
    ///
    /// [`land_scan_step`]: Self::land_scan_step
    pub fn step_scan(&mut self) {
        self.arm(StepMode::Scan);
    }

    /// Arm the *landing* half of a scan step: pause before this round's first
    /// instruction, reported as [`PauseReason::Step`].
    ///
    /// The driver arms this on the round that follows a
    /// [`RoundOutcome::PausedAfterScan`](crate::RoundOutcome::PausedAfterScan),
    /// so the user lands at the start of the new scan with the entry frame
    /// live: a call stack to show, a line to highlight, and freshly flushed
    /// outputs to read. A breakpoint on that same instruction takes precedence
    /// and is reported instead.
    pub fn land_scan_step(&mut self) {
        self.scan_landing = true;
    }

    fn arm(&mut self, mode: StepMode) {
        self.step = StepController {
            mode,
            origin_depth: self.depth,
            origin_offset: self.last_offset,
        };
    }

    /// Seed the depth / last-offset mirror to a resumed pause position.
    ///
    /// A fresh hook starts at depth 0 / offset 0, but the single-threaded driver
    /// rebuilds the hook each round; when it resumes a mid-scan paused frame
    /// stack it seeds the live call depth (`frames.len() - 1`, so the entry
    /// frame is depth 0, matching the `before_call`/`after_return` mirror) and
    /// the current offset here, so a step armed immediately after
    /// ([`step_over`](Self::step_over) etc.) measures from where the VM actually
    /// paused rather than from scan entry.
    pub fn seed_resume_position(&mut self, depth: usize, offset: usize) {
        self.depth = depth;
        self.last_offset = offset;
    }

    /// Suppress the breakpoint check for the very next instruction.
    ///
    /// A single-threaded driver that constructs a fresh hook per resume (so it
    /// can mutate the [`BreakpointTable`] between rounds) uses this to avoid
    /// re-triggering, in place, the breakpoint it just paused on: after a
    /// [`PauseReason::Breakpoint`] pause it builds the next round's hook and
    /// calls this before resuming, matching the in-hook `skip_breakpoint_once`
    /// a long-lived hook would have carried across the pause.
    pub fn suppress_next_breakpoint(&mut self) {
        self.skip_breakpoint_once = true;
    }
}

impl DebugHook for DebuggerHook<'_> {
    fn before_instruction(&mut self, function_id: FunctionId, pc: usize, _op: u8) -> HookAction {
        self.last_offset = pc;
        if self.stop_on_entry {
            // Fires once, before the first instruction of the session.
            self.stop_on_entry = false;
            return HookAction::Pause(PauseReason::Entry);
        }
        let skip = self.skip_breakpoint_once;
        self.skip_breakpoint_once = false;
        if !skip {
            if let Some(id) = self.breakpoints.lookup(function_id, pc) {
                // Suppress this breakpoint for the resume instruction.
                self.skip_breakpoint_once = true;
                return HookAction::Pause(PauseReason::Breakpoint(id));
            }
        }
        if self.scan_landing || self.step.landed(self.depth, pc) {
            // A step lands only once; disarm and suppress a co-located
            // breakpoint on the resume instruction.
            self.scan_landing = false;
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

    fn stepping_scan(&self) -> bool {
        self.step.mode == StepMode::Scan
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
    fn breakpoint_table_when_cleared_then_empty_and_lookups_miss() {
        let mut table = BreakpointTable::new();
        table.add(FunctionId::SCAN, 0);
        table.add(FunctionId::SCAN, 6);
        assert_eq!(table.len(), 2);
        table.clear();
        assert!(table.is_empty());
        assert_eq!(table.lookup(FunctionId::SCAN, 0), None);
    }

    #[test]
    fn breakpoint_table_when_id_absent_then_mutators_report_false() {
        let mut table = BreakpointTable::new();
        let ghost = BreakpointId(999);
        assert!(!table.set_enabled(ghost, false));
        assert!(!table.remove(ghost));
    }

    #[test]
    fn debugger_hook_when_suppress_next_breakpoint_then_skips_one_hit_then_pauses() {
        use crate::debug_hook::{DebugHook, HookAction};
        let mut table = BreakpointTable::new();
        table.add(FunctionId::SCAN, 4);
        let mut hook = DebuggerHook::new(&table);
        hook.suppress_next_breakpoint();
        // The suppressed location is skipped exactly once (the resume step).
        assert!(matches!(
            hook.before_instruction(FunctionId::SCAN, 4, 0),
            HookAction::Continue
        ));
        // A later arrival at the same location pauses as normal.
        assert!(matches!(
            hook.before_instruction(FunctionId::SCAN, 4, 0),
            HookAction::Pause(PauseReason::Breakpoint(_))
        ));
    }

    #[test]
    fn debugger_hook_when_stop_on_entry_then_pauses_once_before_first_instruction() {
        use crate::debug_hook::{DebugHook, HookAction};
        let table = BreakpointTable::new();
        let mut hook = DebuggerHook::new(&table);
        hook.stop_on_entry();
        // The first instruction pauses with Entry, before any breakpoint check.
        assert!(matches!(
            hook.before_instruction(FunctionId::SCAN, 0, 0),
            HookAction::Pause(PauseReason::Entry)
        ));
        // It is one-shot: the next instruction runs normally.
        assert!(matches!(
            hook.before_instruction(FunctionId::SCAN, 1, 0),
            HookAction::Continue
        ));
    }

    #[test]
    fn debugger_hook_when_step_over_then_lands_on_next_instruction_same_depth() {
        use crate::debug_hook::{DebugHook, HookAction};
        let table = BreakpointTable::new();
        let mut hook = DebuggerHook::new(&table);
        // Paused at (depth 0, offset 4); arm step-over from there.
        hook.seed_resume_position(0, 4);
        hook.step_over();
        // The resume re-executes the origin instruction: not a landing.
        assert!(matches!(
            hook.before_instruction(FunctionId::SCAN, 4, 0),
            HookAction::Continue
        ));
        // The next instruction at the same depth is the landing.
        assert!(matches!(
            hook.before_instruction(FunctionId::SCAN, 7, 0),
            HookAction::Pause(PauseReason::Step)
        ));
    }

    #[test]
    fn debugger_hook_when_step_over_call_then_skips_callee_body() {
        use crate::debug_hook::{DebugHook, HookAction};
        let table = BreakpointTable::new();
        let mut hook = DebuggerHook::new(&table);
        hook.seed_resume_position(0, 4);
        hook.step_over();
        // Origin instruction resumes.
        assert!(matches!(
            hook.before_instruction(FunctionId::SCAN, 4, 0),
            HookAction::Continue
        ));
        // A CALL descends into a callee; instructions there are deeper than the
        // origin, so step-over does not land inside the callee body.
        hook.before_call(FunctionId::new(2));
        assert!(matches!(
            hook.before_instruction(FunctionId::new(2), 0, 0),
            HookAction::Continue
        ));
        // The callee returns; the next instruction back at origin depth lands.
        hook.after_return(Some(FunctionId::SCAN));
        assert!(matches!(
            hook.before_instruction(FunctionId::SCAN, 8, 0),
            HookAction::Pause(PauseReason::Step)
        ));
    }

    #[test]
    fn debugger_hook_when_step_in_then_lands_on_first_instruction_of_callee() {
        use crate::debug_hook::{DebugHook, HookAction};
        let table = BreakpointTable::new();
        let mut hook = DebuggerHook::new(&table);
        hook.seed_resume_position(0, 4);
        hook.step_in();
        // Origin instruction (the CALL site) resumes.
        assert!(matches!(
            hook.before_instruction(FunctionId::SCAN, 4, 0),
            HookAction::Continue
        ));
        // Step-in descends: the first instruction of the callee is the landing.
        hook.before_call(FunctionId::new(2));
        assert!(matches!(
            hook.before_instruction(FunctionId::new(2), 0, 0),
            HookAction::Pause(PauseReason::Step)
        ));
    }

    #[test]
    fn debugger_hook_when_step_out_then_lands_only_after_origin_frame_returns() {
        use crate::debug_hook::{DebugHook, HookAction};
        let table = BreakpointTable::new();
        let mut hook = DebuggerHook::new(&table);
        // Paused inside a callee at depth 1; step out of it.
        hook.seed_resume_position(1, 2);
        hook.step_out();
        // Still inside the callee (origin depth): no landing yet.
        assert!(matches!(
            hook.before_instruction(FunctionId::new(2), 2, 0),
            HookAction::Continue
        ));
        assert!(matches!(
            hook.before_instruction(FunctionId::new(2), 5, 0),
            HookAction::Continue
        ));
        // The callee returns to the caller (shallower than origin): landing.
        hook.after_return(Some(FunctionId::SCAN));
        assert!(matches!(
            hook.before_instruction(FunctionId::SCAN, 8, 0),
            HookAction::Pause(PauseReason::Step)
        ));
    }

    #[test]
    fn debugger_hook_when_step_scan_then_never_lands_inside_the_scan() {
        use crate::debug_hook::{DebugHook, HookAction};
        let table = BreakpointTable::new();
        let mut hook = DebuggerHook::new(&table);
        hook.seed_resume_position(0, 0);
        hook.step_scan();
        // A scan step's landing is the scan boundary, which the driver
        // observes -- so no instruction, at any depth, is a landing.
        assert!(hook.stepping_scan());
        for (depth_change, function_id, pc) in [
            (0, FunctionId::SCAN, 3),
            (1, FunctionId::new(2), 0),
            (-1, FunctionId::SCAN, 6),
        ] {
            match depth_change {
                1 => hook.before_call(FunctionId::new(2)),
                -1 => hook.after_return(Some(FunctionId::SCAN)),
                _ => {}
            }
            assert!(matches!(
                hook.before_instruction(function_id, pc, 0),
                HookAction::Continue
            ));
        }
        // Still armed at the end of the scan: that is what the driver reads.
        assert!(hook.stepping_scan());
    }

    #[test]
    fn debugger_hook_when_step_scan_and_breakpoint_then_breakpoint_wins() {
        use crate::debug_hook::{DebugHook, HookAction};
        let mut table = BreakpointTable::new();
        let id = table.add(FunctionId::SCAN, 6);
        let mut hook = DebuggerHook::new(&table);
        hook.step_scan();
        assert!(matches!(
            hook.before_instruction(FunctionId::SCAN, 3, 0),
            HookAction::Continue
        ));
        // A breakpoint inside the stepped-over scan still stops there, the way
        // one inside a step-over does.
        assert!(matches!(
            hook.before_instruction(FunctionId::SCAN, 6, 0),
            HookAction::Pause(PauseReason::Breakpoint(bp)) if bp == id
        ));
    }

    #[test]
    fn debugger_hook_when_scan_step_landing_then_pauses_at_first_instruction() {
        use crate::debug_hook::{DebugHook, HookAction};
        let table = BreakpointTable::new();
        let mut hook = DebuggerHook::new(&table);
        hook.land_scan_step();
        // The landing half is a step stop at the very first instruction of the
        // new scan -- and it is not itself a scan step, so the driver does not
        // run another cycle out.
        assert!(!hook.stepping_scan());
        assert!(matches!(
            hook.before_instruction(FunctionId::SCAN, 0, 0),
            HookAction::Pause(PauseReason::Step)
        ));
        // One-shot: the following instruction runs.
        assert!(matches!(
            hook.before_instruction(FunctionId::SCAN, 3, 0),
            HookAction::Continue
        ));
    }

    #[test]
    fn debugger_hook_when_not_stepping_scan_then_driver_sees_no_scan_step() {
        use crate::debug_hook::DebugHook;
        let table = BreakpointTable::new();
        let mut hook = DebuggerHook::new(&table);
        assert!(!hook.stepping_scan());
        hook.step_over();
        assert!(!hook.stepping_scan());
        // The default trait impl keeps a non-debugger hook out of the way.
        assert!(!crate::debug_hook::NoopDebugHook.stepping_scan());
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
