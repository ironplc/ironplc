# Plan: Implement `ironplc/stepScan` in the debug server

## Context

The VS Code extension contributes a **Step Scan Cycle** button to the debug
toolbar (`integrations/vscode/package.json`, `src/customRequests.ts`), but
`Command::from_request` in `compiler/vm-cli/src/dap/state.rs` does not model
`ironplc/stepScan`, so the server answers `requestNotApplicable` and the
extension pops a "not supported by this debug server" warning. A control that is
always visible during a session always fails — see
[#1398](https://github.com/ironplc/ironplc/issues/1398).

The issue offers two ways out: implement the request, or hide the button. This
plan implements it, which is what the design already calls for — the button is
not the problem, the missing server half is.

### What the design says

`specs/design/debugger-support.md` §"Scan Cycle Control" defines the behavior:

> **Step Scan** — Run one complete scan cycle (INPUT_FREEZE → EXECUTE →
> OUTPUT_FLUSH), then pause before the next

and §"Single-threaded DAP loop (v1)" lists "Step Scan landing" as a natural stop
point signalled by `RoundOutcome::PausedAfterScan`. That outcome is declared in
`compiler/vm/src/vm.rs` but has **no producer**, and `StepMode` has no
scan-level variant, so the debug engine needs the work first.

### Where the stop lands

`RoundOutcome::PausedAfterScan` is a *scan-boundary* signal: at the boundary the
frame stack has drained, so a stop reported there would answer `stackTrace` with
zero frames. A DAP client with no frames requests no scopes, so the Variables
pane would be empty — for the one command whose entire purpose is watching
values change from cycle to cycle. It also breaks a following `next`: a step
armed against an empty frame stack measures its origin from `(depth 0, offset
0)`, which is the first instruction, so it would land on the *second* statement
of the new scan.

So the stop lands **at the first instruction of the next scan**, which is also
what the issue asks for ("run to the end of the current scan and pause at the
start of the next"). The outputs of the finished cycle are flushed and
`scanCount` has advanced, exactly as at the boundary, but the client sees a real
frame, a highlighted line, and a populated Variables pane.

That makes a scan step two halves across two rounds, because the DAP loop
rebuilds the `DebuggerHook` every round (it mutates the `BreakpointTable`
between rounds while the hook borrows it):

1. **Run half** — the round runs with `StepMode::Scan` armed. The step never
   lands intra-scan; when the scan completes, `run_round_debug` reports
   `RoundOutcome::PausedAfterScan`.
2. **Landing half** — the server arms the next round's hook to pause before its
   first instruction, which surfaces as `RoundOutcome::Paused(PauseReason::Step)`
   and a `stopped{reason:"step"}` event.

The server already carries `pending_step`, `pending_stop_on_entry`, and
`suppress_bp` across rounds the same way, so this adds one more flag of the same
kind rather than a new mechanism.

A breakpoint reached during the run half still wins: the hook checks breakpoints
before step landings, so the scan step is abandoned at the breakpoint stop. That
matches how `next`/`stepIn` already behave and is what every debugger does.

`scanLimit` still bounds the run: if the completed cycle reaches the bound, the
session terminates instead of landing the step.

## Goals

1. Pressing **Step Scan Cycle** while paused runs the rest of the current scan
   cycle and stops at the start of the next, with the call stack and variables
   inspectable at the stop.
2. `ironplc/stepScan` is a modelled request, legal exactly where the other
   execution-control requests are (a non-terminal pause) and refused elsewhere.
3. The extension no longer reports the command as unsupported.
4. The design doc and the debugger reference no longer describe it as deferred.

## Non-goals

- **Pause Between Scans** and **Run to Scan N** (the other two rows of §"Scan
  Cycle Control"). `scanLimit` covers the runaway case today.
- Multi-instance scan stepping — v1 rejects multi-instance programs at launch.

## Architecture

| Layer | Change |
|-------|--------|
| `vm` debug engine | `StepMode::Scan`; `DebuggerHook::step_scan()` (run half) and `land_scan_step()` (landing half); `DebugHook::stepping_scan()` so the driver can ask |
| `vm` driver | `run_round_debug` reports `RoundOutcome::PausedAfterScan` when a completed scan had a scan step armed — the outcome finally gets its producer |
| DAP legality | `Command::StepScan`, mapped from `ironplc/stepScan`, legal in `Paused` |
| DAP loop | `stepScan` arms `StepMode::Scan`; `PausedAfterScan` arms the landing on the next round (or terminates at `scanLimit`) |
| Extension | Drop the "not supported" catch; report a genuine failure instead |

`StepController::landed` returns `false` for `StepMode::Scan`: a scan step has
no intra-scan landing, so the decision belongs to the driver at the round
boundary, not to the per-instruction check.

## Design doc reference

`specs/design/debugger-support.md` — §"Scan Cycle Control", §"Step Modes",
§"Single-threaded DAP loop (v1)", §"Custom DAP Requests".

## File map

| File | Change |
|------|--------|
| `compiler/vm/src/debug.rs` | `StepMode::Scan`, `step_scan()`, `land_scan_step()`, `stepping_scan()`, `landed` arm; unit tests |
| `compiler/vm/src/debug_hook.rs` | `DebugHook::stepping_scan()` with a `false` default |
| `compiler/vm/src/vm.rs` | `run_round_debug` produces `RoundOutcome::PausedAfterScan`; doc comment updated |
| `compiler/vm/tests/it/debug_engine.rs` | Integration tests for both halves |
| `compiler/vm-cli/src/dap/state.rs` | `Command::StepScan` + legality; unit tests |
| `compiler/vm-cli/src/dap/server.rs` | `stepScan` handler, landing flag, `PausedAfterScan` arm; server tests |
| `integrations/vscode/src/customRequests.ts` | Error path no longer says "not supported" |
| `integrations/vscode/src/debugAdapterLogic.ts` | Rename/retarget `customRequestFailedMessage` |
| `docs/reference/editor/debugging.rst` | Replace the known-limitation note with the real behavior |
| `specs/design/debugger-support.md` | Mark `ironplc/stepScan` implemented |

## Tasks

- [ ] Commit this plan
- [ ] `vm`: `StepMode::Scan` + hook arming/landing + `stepping_scan()`
- [ ] `vm`: `run_round_debug` produces `PausedAfterScan`
- [ ] `vm`: unit + integration tests
- [ ] DAP: model `ironplc/stepScan` and its legality
- [ ] DAP: server loop handles the request and the two-round landing
- [ ] DAP: server tests (stop position, scanCount advance, scanLimit, legality)
- [ ] Extension: fix the failure message and its test
- [ ] Docs: debugger reference + design doc
- [ ] `cd compiler && just` passes
