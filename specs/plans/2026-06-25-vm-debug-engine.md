# Phase 3: VM Debug Engine

## Goal

Turn the VM's existing instruction-level hook into a debugger-grade engine
that can **pause at breakpoints, single-step (over / in / out), and inspect
a paused frame stack** — all driven purely by `(FunctionId,
bytecode_offset)` coordinates. This is Phase 3 of the debugger design
(`specs/design/debugger-support.md` §"Phase 3: VM Debug Engine").

> **Scope cut.** This plan drops **logpoints** (`Logpoint` / `LogpointTable`
> / `LogSink`) and any expression evaluation from the first debugger phase,
> to match the cut-down Phase 4 (`2026-06-25-dap-server-scaffold.md`). The
> engine is breakpoints + stepping only. Logpoints are a cheap follow-up
> once this lands (a breakpoint entry that emits instead of pausing), but
> they are not in the first cut. Reverse this together with the Phase 4
> logpoint decision if logpoints should ship first.

The deliverable is a `DebuggerHook` (implementing the extended `DebugHook`
trait) plus a re-entrant `VmRunning::run_round_debug` entry point that runs
the single program instance to the next natural stop point and reports why
it stopped. No DAP, no source-line translation, no VS Code, no logpoints —
those are Phases 4 and 5 / deferred.

## Why now

This work is **on the debugger critical path and independent of the
debug-info (Layer 1) work currently in progress.** The two can proceed in
parallel:

- **Phase 2 (iterative dispatch) is done.** `compiler/vm/src/frame_stack.rs`
  defines `Frame` / `FrameStack`; `execute_with_hook` (`vm.rs:680`) is a
  single iterative loop that pushes a frame on `CALL` and pops on `RET`
  instead of recursing; `compute_max_call_depth` writes
  `FileHeader.max_call_depth` (`codegen/src/compile.rs:722`) and the VM
  sizes the frame slice from it. The hard prerequisite — the one the spec
  calls "the largest change in the plan" — is already behind us.
- **The debug engine operates in bytecode-offset space.** A breakpoint is a
  `(FunctionId, bytecode_offset)`. Stepping compares frame depth and the
  current offset against a remembered origin. None of this needs line maps,
  variable names, or type tags. Layer 1 debug info is only consumed later,
  in the DAP server (Phase 4), to translate a source line ↔ a bytecode
  offset and to render slot values. So Phase 3 can be built and fully
  tested today, against hand-written offset breakpoints, while Layer 1
  debug-info emission is still being finished.

After this phase the VM is a complete debuggee; the remaining work is
protocol plumbing (Phase 4) and the editor (Phase 5).

## Current state

`compiler/vm/src/debug_hook.rs` defines the **minimal** hook:

```rust
pub trait DebugHook {
    fn before_instruction(&mut self, function_id: FunctionId, pc: usize, op: u8);
}
```

It returns `()`, so a hook can observe but cannot ask the VM to pause. It
has no call/return callbacks, so a hook cannot track call depth without
re-deriving it. The dispatch loop calls it at `vm.rs:730`, just before
`pc += 1`. `NoopDebugHook` is the zero-cost default used by `run_round`.

The VM has no `Phase` enum and no re-entrant debug entry point:
`run_round` (`vm.rs:295`) runs every ready task's every instance to
completion in one call and cannot stop in the middle.

## Scope

**In:**

- Extend the `DebugHook` trait: `before_instruction` returns a `HookAction`;
  add `before_call` / `after_return` with default no-op bodies; add
  `HookAction` and `PauseReason` types. `NoopDebugHook` keeps its zero-cost
  profile (returns `HookAction::Continue` from an `#[inline(always)]` body).
- Wire the dispatch loop (`execute_with_hook`) to honour `HookAction::Pause`
  and to call `before_call` / `after_return` around frame push/pop.
- Add an `ExecuteOutcome::{Completed, Paused}` return so the loop can yield
  to its caller mid-instance, preserving the `FrameStack` for resume.
- New `compiler/vm/src/debug.rs`: `BreakpointTable` (plain sorted `Vec` —
  no `ArcSwap`, no atomics), `BreakpointId`, `StepMode`, `StepController`,
  and `DebuggerHook` (implements `DebugHook`). **No** `Logpoint` /
  `LogpointTable` / `LogSink` (cut — see §Goal).
- `VmRunning` gains a `Phase` field with paused sub-states and a re-entrant
  `run_round_debug<H: DebugHook>` returning `RoundOutcome::{Completed,
  PausedAfterScan, Paused(PauseReason)}`.
- Re-exports from `vm/src/lib.rs`.
- Tests (see §Tests).

**Out (deferred — matches the spec's v1 cuts and later phases):**

- DAP protocol, framing, server loop, VS Code — Phase 4 / 5.
- Source-line ↔ offset translation and variable-name/type rendering. The
  engine reports offsets; the DAP server (Phase 4) does the lookup against
  the debug section.
- Logpoints and any expression evaluation — cut from the first phase (see
  §Goal). The breakpoint table is pause-only.
- `pause_requested: AtomicBool`, `ArcSwap<BreakpointTable>`, any cross-thread
  pause — explicitly out (`§Single-threaded DAP loop`; the table is owned
  directly by the single-threaded caller).
- `current_instance_id` / multi-instance pause/resume — v1 is single-instance
  (`§Multi-instance: not supported in v1`).
- `force_variable` / write-while-paused — deferred (`§Variable forcing:
  not in v1`).

## Design

### 1. Extended `DebugHook` trait (`vm/src/debug_hook.rs`)

```rust
/// What the VM should do after the hook inspects the upcoming instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookAction {
    /// Execute the instruction normally.
    Continue,
    /// Stop *before* executing the instruction at `(function_id, pc)`.
    /// The frame stack is left intact so execution can resume here.
    Pause(PauseReason),
}

/// Why the VM stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseReason {
    Breakpoint(BreakpointId),
    Step,        // a single-step landing
    Entry,       // stop-on-entry
    // Trap is surfaced through the existing fault path, not here.
}

pub trait DebugHook {
    fn before_instruction(&mut self, function_id: FunctionId, pc: usize, op: u8) -> HookAction;

    /// Called after the hook approves a CALL/FB_CALL instruction and just
    /// before the callee frame is pushed. Default: no-op.
    fn before_call(&mut self, _callee: FunctionId) {}

    /// Called just after a frame is popped (RET / RET_VOID / fall-off-end).
    /// Default: no-op.
    fn after_return(&mut self, _returning_to: FunctionId) {}
}
```

`NoopDebugHook::before_instruction` returns `HookAction::Continue` from an
`#[inline(always)]` empty body so the monomorphized `run_round` path stays
allocation- and branch-free (verified by the existing no-overhead bench).

`BreakpointId` and `PauseReason` live in `debug.rs` and are re-exported;
`debug_hook.rs` imports `BreakpointId` for the `PauseReason` variant. (If
the import direction is awkward, move `PauseReason` into `debug.rs` and have
the trait reference it there — decide during implementation; the trait file
already imports from `ironplc_container`.)

### 2. Dispatch-loop changes (`execute_with_hook`, `vm.rs`)

Three edits to the existing loop (`vm.rs:680`–end):

1. At the current hook call site (`vm.rs:730`), branch on the return:
   ```rust
   match hook.before_instruction(current_function_id, pc, op) {
       HookAction::Continue => {}
       HookAction::Pause(reason) => {
           // Write pc back to the top frame WITHOUT advancing, so the
           // paused instruction re-executes on resume. Return Paused.
           frame_stack.top_mut().pc = pc; // pc not yet incremented
           return Ok(ExecuteOutcome::Paused(reason));
       }
   }
   pc += 1;
   ```
   The pause point is *before* `pc += 1` and before any operand decode, so
   the saved `pc` points at the opcode and the frame stack is a valid resume
   state — exactly the invariant the pause-corpus scaffolding from Phase 2
   already exercises.

2. In the `CALL` / `FB_CALL` arms, call `hook.before_call(callee_id)` right
   before `frame_stack.push(...)`.

3. In `handle_frame_return` (or at each pop site), call
   `hook.after_return(returning_to)` after the pop. Note `handle_frame_return`
   is shared by explicit `RET` and fall-off-the-end; route the callback
   through it once.

Change the return type from `Result<(), Trap>` to
`Result<ExecuteOutcome, Trap>`:

```rust
pub enum ExecuteOutcome { Completed, Paused(PauseReason) }
```

`execute` / the `NoopDebugHook` callers map `Completed => ()` and treat
`Paused` as unreachable (it cannot occur with `NoopDebugHook`, whose action
is always `Continue`) — keep the existing `run_round` behaviour byte-for-byte.

### 3. `DebuggerHook` and tables (new `vm/src/debug.rs`)

```rust
pub struct BreakpointId(pub u32);

struct BreakpointEntry {
    id: BreakpointId,
    function_id: FunctionId,
    offset: usize,
    enabled: bool,
    // pause-only; no logpoint variant in the first phase (see §Goal)
}

/// Sorted by (function_id, offset); lookup is a binary search. No ArcSwap,
/// no atomics — the single-threaded DAP loop owns and mutates this directly.
pub struct BreakpointTable { entries: Vec<BreakpointEntry>, next_id: u32 }

pub enum StepMode { None, Over, In, Out }

/// Remembers the origin of a step so the hook knows when the step has
/// "landed": same-or-shallower depth at a *new* offset for Over; depth+1
/// for In; depth-1 for Out. Depth is FrameStack::len(), tracked via
/// before_call / after_return.
struct StepController { mode: StepMode, origin_depth: usize, origin_offset: usize }

pub struct DebuggerHook<'a> {
    breakpoints: &'a BreakpointTable,
    step:        StepController,
    depth:       usize,          // mirrors FrameStack::len()
}
```

`DebuggerHook::before_instruction`:
1. Just read state.
2. Look up `(function_id, pc)` in the breakpoint table.
   - Hit + enabled → `Pause(Breakpoint(id))`.
3. Else, if a step is active and the step has landed → `Pause(Step)`.
4. Else `Continue`.

`before_call` / `after_return` keep `depth` in sync (the engine's own copy,
so it doesn't have to reach into the VM's `FrameStack`).

### 4. `Phase` and `run_round_debug` (`vm.rs`)

```rust
pub enum Phase { Ready, Running, PausedAt(PauseReason), CompletedScan, Faulted }
```

`VmRunning` gains `phase: Phase`. New re-entrant method:

```rust
pub fn run_round_debug<H: DebugHook>(
    &mut self,
    current_time_us: u64,
    hook: &mut H,
) -> Result<RoundOutcome, FaultContext>;

pub enum RoundOutcome { Completed, PausedAfterScan, Paused(PauseReason) }
```

It mirrors `run_round` (`vm.rs:295`) but:
- routes through `execute_with_hook` with the real `hook` instead of
  `NoopDebugHook`;
- on `ExecuteOutcome::Paused(reason)` it stops, sets
  `Phase::PausedAt(reason)`, leaves `self.frames` intact, and returns
  `Paused(reason)` — the caller can later call `run_round_debug` again to
  resume from the saved frame stack;
- on `Completed` for the instance, runs the scan-boundary logic and returns
  `Completed` (or `PausedAfterScan` when a scan-step is pending).

Because v1 is single-instance, the multi-instance loop in `run_round`
collapses to the one instance; assert/guard `program_instances.len() == 1`
is a Phase 4 launch concern, not enforced here (the loop simply handles the
single instance and is re-entrant across calls via the preserved frame
stack).

### 5. Re-exports (`vm/src/lib.rs`)

Export `DebuggerHook`, `BreakpointTable`, `BreakpointId`, `StepMode`,
`PauseReason`, `RoundOutcome`, `Phase`, `HookAction`, `ExecuteOutcome`.

## Tests

All testable with hand-written `(FunctionId, offset)` breakpoints — no debug
info required.

- **Breakpoint in entry function**: set bp at a known offset in SCAN; assert
  pause at exactly `(SCAN, offset)` and that resume completes the scan with
  the same final `variables` / `data_region` as an unhooked run.
- **Breakpoint inside a callee**: bp at an offset in a called function;
  assert pause with the callee frame on top and the caller frame beneath it
  (walk `FrameStack`).
- **Step-over a CALL**: from a line containing a CALL, step lands on the next
  instruction *after* the CALL at the origin depth (not inside the callee).
- **Step-in**: lands on the first instruction of the callee, depth = origin+1.
- **Step-out**: lands in the caller just after the CALL, depth = origin−1.
- **Pause/resume parity (corpus)**: reuse the Phase 2 pause-corpus scaffold —
  pause at every instruction boundary via a test hook, resume, assert final
  `variables` and `data_region` are bitwise identical to an unpaused run.
- **No overhead**: extend the existing criterion bench to confirm
  `NoopDebugHook` (now returning `HookAction::Continue`) stays within <1% of
  main — guards against the enum return regressing the hot path.
- **Trap during debug**: a trap still surfaces through the existing fault
  path and transitions `Phase` to `Faulted`; `run_round_debug` returns
  `Err(FaultContext)` unchanged.

## Commit order

Each commit must compile and pass `cd compiler && just`.

1. Extend the `DebugHook` trait + `HookAction` / `PauseReason` + update
   `NoopDebugHook`; change `execute_with_hook` to return `ExecuteOutcome`
   and honour `Pause`; keep `run_round` behaviour identical
   (`Completed => ()`). All existing tests pass unchanged. (No new feature
   yet — just the plumbing.)
2. Add `before_call` / `after_return` callbacks and wire them through the
   CALL/FB_CALL and return paths.
3. New `debug.rs`: `BreakpointTable` + `DebuggerHook` (breakpoints only),
   `run_round_debug` + `Phase` + `RoundOutcome`; breakpoint + resume-parity
   tests.
4. Add `StepController` (over/in/out) and step tests.

Splitting this way keeps each commit independently reviewable and keeps the
no-overhead bench honest at step 1 (the riskiest change for the hot path).

## Risks

- **Hot-path regression from the enum return.** `before_instruction` now
  returns `HookAction` on every instruction. Mitigation: `NoopDebugHook`
  returns a const `Continue` from an `#[inline(always)]` body; the optimizer
  should fold the match away. Guarded by the no-overhead bench at commit 1.
- **Resume-state correctness.** The saved `pc` must point at the *opcode*,
  not mid-operand, and the `FrameStack` / `temp_alloc` marks must be valid at
  the pause boundary. Mitigation: pause strictly before `pc += 1` and before
  any operand decode; rely on the Phase 2 pause-corpus parity test.
- **Depth tracking drift.** `DebuggerHook.depth` mirrors `FrameStack::len()`
  via callbacks; an asymmetry between push/pop call sites would desync
  stepping. Mitigation: route every pop through `handle_frame_return` so
  `after_return` fires exactly once per pop; assert `depth == frames.len()`
  in debug builds.
