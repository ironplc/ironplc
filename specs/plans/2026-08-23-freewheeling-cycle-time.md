# Assumed Cycle Time for Freewheeling Tasks Under Simulated Time

Fixes [#1413](https://github.com/ironplc/ironplc/issues/1413).

## Context

The `run` MCP tool executes a container for `duration_ms` of *simulated* time,
deriving the cycle schedule from the task table. That works when every task
declares an `INTERVAL`. It does not work when a task is freewheeling: a
freewheeling task has no cycle rate, so there is nothing to advance simulated
time by.

`runner::execute` (`compiler/mcp/src/runner.rs`) advances time from
`VmRunning::next_due_us()`, which filters to `TaskType::Cyclic`
(`compiler/vm/src/vm.rs`). A freewheeling task yields `None`, so the round loop
breaks after a single cycle and reports `ok: true`, `terminated_reason:
"completed"` — a clean-looking result for a run that never happened.

Every program without a `CONFIGURATION` hits this: `ContainerBuilder`
synthesizes a freewheeling task (`compiler/container/src/builder.rs`), and
codegen also compiles `INTERVAL := T#0s` to freewheeling
(`compiler/codegen/src/compile.rs`), because a zero-interval cyclic task would
be permanently overdue.

The debugger has the same missing input in a different form: it advances a flat
1 ms per scan and bypasses the scheduler ([#1397](https://github.com/ironplc/ironplc/issues/1397)).

## Design

A freewheeling task's cycle rate is a property of the machine that would run it,
which the container cannot know and the server should not invent silently. The
resolution is to make it an *input*:

1. **Simulated-time execution assumes a cycle time for freewheeling tasks.**
   The assumption defaults to **100 ms** and is caller-overridable.
2. **The assumption is always reported**, so a trace is self-describing: `run`
   echoes it in `summary`, and the debugger prints it to the console.
3. **A freewheeling task plus an assumed cycle time is exactly a cyclic task at
   that interval**, for simulation purposes. Applying the assumption is
   therefore a rewrite of the task table before the VM loads it — no new
   scheduling path, no change to the round loop.

This keeps `duration_ms` as the single stop criterion, so the tool's input shape
gains one optional field and breaks nothing. It also confines the invented
number: it never decides how long a run is, only what the clock reads, and it is
stated in the response rather than buried in the server.

Real-clock execution (`ironplcvm run`) is unaffected — it has an actual clock.

### Why not the alternatives

- *Advance by a nominal 1 ms when nothing is due.* Same invention, unreported,
  and 1 ms is an unlikely rate for a real freewheeling scan.
- *Reject freewheeling containers.* Makes `run` unusable for the simplest
  program an agent can write, which is the common case for `compile` + `run`.
- *Stop after N cycles instead of a duration.* Resolves the stop criterion but
  leaves the clock undefined, so `time_ms` stays meaningless and `TON` never
  elapses. It also needs a second sandbox ceiling and a second
  `terminated_reason`.

### Requirement to add

`specs/design/mcp-server.md` gains **REQ-TOL-mcp-049** for the assumption, and
REQ-TOL-mcp-040 is revised to stop implying that every container declares an
interval.

## Architecture

`ironplc-vm` gains a `freewheeling` module — both `ironplc-mcp` and
`ironplc-vm-cli` already depend on it, and the policy is about simulated
execution rather than the container format:

```rust
pub const DEFAULT_FREEWHEELING_INTERVAL_US: u64 = 100_000;

/// Rewrites every enabled freewheeling task into a cyclic task at
/// `interval_us`. Returns the number of tasks rewritten.
pub fn assume_freewheeling_interval(container: &mut Container, interval_us: u64) -> usize;

/// True when the container has at least one enabled freewheeling task.
pub fn has_freewheeling_task(container: &Container) -> bool;
```

`Vm::load` copies `task_type` and `interval_us` from the container's task table
into `TaskState`, so a rewrite performed between `Container::read_from` and
`Vm::load` is picked up by `next_due_us()`, the round loop, `time_ms` stamping,
and `completed_cycles` with no further change.

### Scope boundary with #1397

This plan gives the debugger the freewheeling half: an assumed interval, the
console notice, and a clock that advances by it. The debugger's *cyclic* clock
(a flat 1 ms regardless of `INTERVAL`) stays as it is — that is #1397, which
also carries tutorial updates. The shared module makes that fix a one-line
generalization: advance by the executing task's `interval_us`, which after the
rewrite every task has.

### Prerequisite: `compile` must report the truth

The rule "supply a rate when the container is freewheeling" is only actionable
if the caller can see that it is. `compile` currently reports `kind: "event"`
for a program with no `CONFIGURATION` and `kind: "cyclic"` for `INTERVAL :=
T#0s`; both compile to freewheeling. `TaskMeta.kind` must mirror codegen.

## File map

| File | Change |
|------|--------|
| `specs/design/mcp-server.md` | Revise REQ-TOL-mcp-040; add REQ-TOL-mcp-049; document the input field |
| `compiler/vm/src/freewheeling.rs` | New — the assumption constant and task-table rewrite |
| `compiler/vm/src/lib.rs` | Export the new module |
| `compiler/mcp/src/tools/run.rs` | `freewheeling_interval_ms` input, validation, `summary` echo |
| `compiler/mcp/src/runner.rs` | Apply the assumption before load; report what was applied |
| `compiler/mcp/src/tools/compile.rs` | `TaskMeta.kind` mirrors codegen |
| `compiler/mcp/src/spec_conformance.rs` | Conformance test for REQ-TOL-mcp-049 |
| `compiler/vm-cli/src/dap/types.rs` | `freewheelingIntervalMs` launch argument |
| `compiler/vm-cli/src/dap/server.rs` | Console notice; clock advances by the assumed interval |
| `docs/reference/mcp/tools.rst` | Document the `run` input and the `summary` field |
| `docs/reference/runtime/ironplcvmd.rst` | Document the launch argument |

## Tasks

- [ ] Add `freewheeling` module to `ironplc-vm` with unit tests
- [ ] Apply the assumption in `runner::execute`; thread the effective interval
      into `RunOutcome`
- [ ] Add `freewheeling_interval_ms` to `RunInput`; validate finite and `> 0`
- [ ] Echo the assumption in `RunSummary`
- [ ] Fix `TaskMeta.kind` for the no-`CONFIGURATION` and zero-interval cases
- [ ] Revise the design doc; add REQ-TOL-mcp-049 and its conformance test
- [ ] Add the DAP launch argument, the console notice, and the freewheeling
      clock advance
- [ ] Update both documentation pages
- [ ] `cd compiler && just`
