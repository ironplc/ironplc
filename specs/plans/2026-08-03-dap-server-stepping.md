# Plan: DAP Server-Side Single-Stepping

## Context

Phases 1–5 of the debugger (`specs/design/debugger-support.md`) delivered the VM
debug engine, whose `DebuggerHook` (`compiler/vm/src/debug.rs`) already
implements `next`/`stepIn`/`stepOut` via a `StepController` (`step_over`,
`step_in`, `step_out`; `StepMode::{Over, In, Out}`). The DAP server, however,
still refuses those requests with `requestNotApplicable` — the design's Phase 5
note lists "single-stepping (`next`/`stepIn`/`stepOut`) server support" as a
later phase.

This change wires the three execution-control requests through to the engine
that already supports them. It touches only the DAP server plus one small
addition to the hook; it does not add new stepping semantics.

## The one wrinkle: seeding the resumed position

The single-threaded server rebuilds the `DebuggerHook` fresh each round (so the
`BreakpointTable` can be mutated between rounds). A fresh hook starts at call
depth `0` and offset `0`. That is correct when a scan starts from entry, but a
step is armed while **paused mid-scan**, and the `StepController` measures
"landed" relative to the *origin* depth/offset captured when the step is armed.

So before arming a step on the per-round hook, the server must seed the hook's
depth/offset mirror to where the VM actually paused. The VM already exposes the
live frames via `VmRunning::debug_frames()` (outermost-first, each `Frame`
carrying `function_id` and `pc`): the origin depth is `frames.len() - 1` (entry
frame = depth 0, matching the hook's `before_call`/`after_return` mirror) and the
origin offset is the innermost frame's `pc`.

## Design

### VM crate (`compiler/vm/src/debug.rs`)

Add `DebuggerHook::seed_resume_position(depth, offset)` — sets the depth and
last-offset mirror so a step armed immediately after uses the real paused
location as its origin.

### Server crate (`compiler/vm-cli/src/dap/server.rs`)

- A `pending_step: Option<StepMode>` local in `launched_session`.
- `next`/`stepIn`/`stepOut` handlers (already legal while `Paused` per
  `state::legal`): answer success, set `pending_step`, transition to `Running`.
- When building the per-round hook, if a step is pending: read
  `running.debug_frames()`, seed the hook's position, and arm the matching
  `step_over`/`step_in`/`step_out`. The existing `suppress_bp` (set on every
  pause) already skips a co-located breakpoint on the resume instruction, so a
  step off a breakpoint makes forward progress.

The `PauseReason::Step → "step"` stopped-event mapping already exists in the loop.

## Non-goals

- Continuous multi-scan run loop / `scanLimit` / `stopOnEntry` (separate change).
- Trap → `stopped{reason:"exception"}` events.
- Server-side `ironplc/stepScan` / `ironplc/scanCount` custom requests.

## Testing

Unit (`vm/src/debug.rs`): `step_over` lands on the next instruction at the same
depth and skips a callee body; `step_in` lands on the first instruction of a
callee; `step_out` lands only after the origin frame returns — each armed from a
seeded resume position.

Integration (`vm-cli/src/dap/server.rs`): from a breakpoint pause, `next`
advances the paused pc statement-by-statement (observed via `stackTrace`);
`stepIn`/`stepOut` are wired and resume the VM. The prior
"step ⇒ requestNotApplicable" test is replaced by these.

## Note on merge order

This branch and the continuous-run-loop change both edit the `launched_session`
run loop, so whichever merges second needs a small rebase. They are functionally
independent.
