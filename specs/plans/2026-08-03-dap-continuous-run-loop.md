# Plan: DAP Phase 4c — Continuous Run Loop

## Context

Phases 1–5 of the debugger (`specs/design/debugger-support.md`) delivered debug
info, the iterative VM, the VM debug engine, the single-threaded DAP server
(`ironplcdap`), and the VS Code integration. The DAP server's post-launch
run/stop loop (`compiler/vm-cli/src/dap/server.rs`, `launched_session`) is still
the Phase 4 **single-scan** minimal loop: on `RoundOutcome::Completed` it emits
`terminated` after exactly one scan cycle.

That is the explicitly tracked follow-up in the design's §"Status — continuous
run loop deferred (Phase 4c)":

> A real debugger must instead keep scanning — loop back on `Completed` to run
> the next scan, so breakpoints fire every cycle and variables evolve across
> cycles — bounded by the launch `scanLimit` (runaway prevention) and honoring
> `stopOnEntry`.

Without this, a breakpoint fires at most once (in the first scan) and continuing
past it ends the session — the debugger cannot observe a PLC program across scan
cycles, which is its headline use case.

## Goals

1. **Keep scanning.** On `RoundOutcome::Completed`, run the next scan instead of
   terminating, so a breakpoint re-fires every cycle and variable state evolves
   across cycles.
2. **`scanLimit` runaway prevention.** When the launch config supplies
   `scanLimit: N`, terminate the session (emit `terminated`) once `scan_count`
   reaches `N`. Both `stopOnEntry` and `scanLimit` are already parsed into
   `LaunchRequestArguments` (`dap/types.rs`) but currently ignored.
3. **`stopOnEntry`.** When set, pause with `reason: "entry"` before the first
   instruction of the first scan executes, so the client can inspect initial
   state / set breakpoints before any logic runs.

## Non-goals

- Single-stepping (`next`/`stepIn`/`stepOut`) server wiring — a separate change.
- Trap → `stopped{reason:"exception"}` — a trap still ends the session as
  `terminated` (unchanged here).
- Server-side `ironplc/stepScan` / `ironplc/scanCount` custom requests.
- Interactive `pause`-while-running (a Phase 6 cut; the single-threaded loop
  cannot service stdin mid-run, so a free-running no-breakpoint program with no
  `scanLimit` runs until the client kills the adapter process — as designed).

## Design

### VM crate — one-shot entry pause (`compiler/vm/src/debug.rs`)

`DebuggerHook` already produces `PauseReason::{Breakpoint, Step}` but nothing
produces `PauseReason::Entry`. Add a one-shot arming flag:

- new field `stop_on_entry: bool` (default `false`);
- `pub fn stop_on_entry(&mut self)` sets it;
- in `before_instruction`, before the breakpoint/step checks: if armed, disarm
  and return `HookAction::Pause(PauseReason::Entry)`.

The hook is rebuilt each round by the server loop, so the server arms this only
on the very first round; scan 2+ never re-arms it.

### Server crate — continuous loop (`compiler/vm-cli/src/dap/server.rs`)

`launched_session` gains the parsed `LaunchRequestArguments` (thread them
through `load_and_check`, which already parses them):

- **`stopOnEntry`:** a `first_round` flag; on the first `Running` iteration, if
  `stop_on_entry`, call `hook.stop_on_entry()` before `run_round_debug`.
- **Continuous loop:** the `Ok(RoundOutcome::Completed)` arm no longer emits
  `terminated`. Instead it checks the bound: if `scan_limit` is `Some(n)` and
  `running.scan_count() >= n`, emit `terminated` and go `Terminated`; otherwise
  stay in `Running` so the loop drives the next scan. `PausedAfterScan` (not yet
  produced) is grouped with `Completed` for totality.
- `Paused(reason)` and `Err(fault)` arms are unchanged.

`scan_count()` is the VM's own per-`Completed` counter, so no separate counter is
needed.

## Testing

Unit (`vm/src/debug.rs`):
- `stop_on_entry` arms a one-shot `Entry` pause, then a subsequent instruction
  runs normally.

Integration (`vm-cli/src/dap/server.rs`, using the existing `run_server` harness
and a scan function that increments a variable each cycle):
- **Breakpoint re-fires across scans:** a breakpoint after the increment pauses
  every scan; inspecting the variable at each pause shows `1, 2, 3, …` —
  proving the loop keeps scanning and state evolves across cycles.
- **`scanLimit` terminates a free-running program:** no breakpoint,
  `scanLimit: N`; the session terminates after N scans (proving the bound; the
  test would hang without it).
- **`stopOnEntry` pauses before the first instruction:** `reason: "entry"`, and
  the incremented variable is still `0` at that pause.
- Existing single-scan tests are updated to pass `scanLimit: 1` (their intent —
  "run one scan then terminate" — is now expressed as an explicit bound), since
  an unbounded no-breakpoint program would otherwise scan forever.

## Out of scope / follow-ups

- Single-stepping server support (separate change).
- Trap-stop `exception` events; `ironplc/stepScan` / `ironplc/scanCount`.
