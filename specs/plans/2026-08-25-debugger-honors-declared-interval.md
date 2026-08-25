# Debugger Honors the Declared Task INTERVAL

**Issue:** [#1397](https://github.com/ironplc/ironplc/issues/1397)

## Goal

Make program time under `ironplcdap` advance by the task's cycle time rather
than a flat 1 ms, so a `TON` in a debugged program elapses after the same
number of scans it would elsewhere, and remove the second place the debug
driver could pick a cycle time.

## Background

`ironplc_vm::freewheeling` already owns the cycle-time rules: the default
(`DEFAULT_FREEWHEELING_INTERVAL`, 100 ms), the rewrite that turns a
freewheeling task into a cyclic one (`assume_freewheeling_interval`), and the
read-back of the rate a container ends up running at (`first_task_interval`).
The `run` MCP tool drives simulated time entirely from those helpers, so its
clock follows the task table.

The debugger uses them only for the freewheeling case. When the program
declares an `INTERVAL`, `launched_session` falls back to a private
`DEBUG_SCAN_ADVANCE` constant of 1 ms and never reads `interval_us` from the
task table. Timers therefore elapse after a scan count unrelated to the
program's configuration, and the two drivers hold two independent notions of
how fast a debugged scan is.

A second defect sits next to it: the clock advances after *every*
`run_round_debug` call, including rounds that returned `Paused`. A round that
stopped at a breakpoint has not completed a scan, so stepping through one scan
N times moves the clock N intervals. What a timer does under the debugger
currently depends on how the user stepped, which defeats the determinism the
issue asks to keep.

## Architecture

Take the debugger's per-scan advance from the container's task table, the same
way `mcp/src/runner.rs` does:

1. Apply `assume_freewheeling_interval` to the container when it carries a
   freewheeling task, exactly as the runner does, instead of keeping the
   assumed rate in a driver-local variable. The VM copies `interval_us` into
   each `TaskState` at load, so the rewrite is what makes the assumed rate a
   property of the program rather than of the driver.
2. Read the advance back with `first_task_interval`. Both the declared and the
   assumed case now come from one function, which deletes the branch that
   `DEBUG_SCAN_ADVANCE` lived in.
3. Advance only on a completed scan.

The debugger keeps its flat per-scan advance rather than routing through
`next_due_us()`: `run_round_debug` bypasses the scheduler by design, and
scheduler-driven rounds would produce rounds where no task is due — idle stops
with nothing to step through. For the single-task containers the VM supports
today the two are observably identical: the runner lands rounds at 0, 100,
200 ms for a 100 ms task, and so does scan N at N x 100 ms.

Moving the clock itself into a shared `ironplc_vm` type is deliberately left
out of this change; it is tracked separately.

## File map

| File | Change |
|------|--------|
| `compiler/vm-cli/src/dap/server.rs` | Take the advance from the task table; delete `DEBUG_SCAN_ADVANCE`; advance only on a completed scan |
| `compiler/vm-cli/src/dap/server.rs` (tests) | Tighten the uptime test; cover the pause case and the declared interval |
| `compiler/vm-cli/tests/dap.rs` | End-to-end regression: the issue's `INTERVAL := T#100ms` + `PT := T#500ms` repro |
| `compiler/vm/src/freewheeling.rs` | Record on `DEFAULT_FREEWHEELING_INTERVAL` why the two callers differ on whether to apply it |

## Tasks

- [x] Rewrite the container's freewheeling tasks in `launched_session` and take
      the scan advance from `first_task_interval`
- [x] Delete `DEBUG_SCAN_ADVANCE` and the `has_freewheeling_task` branch that
      selected it
- [x] Advance `uptime_us` only when the round completed a scan
- [x] Document the default-application policy on `DEFAULT_FREEWHEELING_INTERVAL`
- [x] Unit tests: declared interval drives uptime; a pause does not advance it
- [x] End-to-end test: `TON` with `PT := T#500ms` on a 100 ms task elapses at
      scan 5
- [x] `cd compiler && just`

## Verification

The issue's reproduction is the acceptance test: with
`TASK plc_task(INTERVAL := T#100ms)` and a `TON` declared `PT := T#500ms`, `Q`
becomes `TRUE` at `scanCount` 5, not 500.
