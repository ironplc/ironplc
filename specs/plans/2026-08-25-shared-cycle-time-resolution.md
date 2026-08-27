# One Place Resolves a Simulated Run's Cycle Time

**Issue:** [#1397](https://github.com/ironplc/ironplc/issues/1397) (follow-up)
**Follows:** [2026-08-25-debugger-honors-declared-interval.md](./2026-08-25-debugger-honors-declared-interval.md)

## Goal

Collapse the rate-resolution sequence that the `run` MCP tool and the debugger
each spell out for themselves into one function, so the step that decides what
cycle time a simulated run executes at exists once.

## Background

Both drivers run the same four steps before they can start a clock: decide
whether an assumed rate is needed, apply it to the task table, read back the
rate the container ends up at, and handle the case where there is none. After
the previous change they agree on every step — but they agree by having the
same lines written twice, which is the arrangement that let them disagree in
the first place.

## Architecture

Add `resolve_cycle_time(&mut Container, Option<Duration>) -> Option<Duration>`
to `ironplc_vm::freewheeling`: it applies an assumed rate to the freewheeling
tasks and returns the rate the container will run at, or `None` when the
container needs an assumed rate and the caller supplied none.

Mechanism moves into the function; **policy stays at the call sites**, because
the two callers deliberately differ there and the previous change recorded why:

- `run` treats `None` as an error, rather than build an agent a trace on a rate
  it never chose.
- The debugger supplies a default so a session still starts, and separately
  treats a zero rate as "no rate named" so the clock cannot freeze.

`assume_freewheeling_interval` and `first_task_interval` then have one caller
each — the new function — so both become private and leave the crate's public
surface.

### Not shared: the round loop

The two advance loops stay separate, and this is deliberate. `run_round_debug`
**bypasses the scheduler** (`vm.rs`), so `next_due_us()` never moves under the
debugger and the runner's scheduler-driven advance cannot be used there. They
compute the same answer for the single-task containers the VM supports, by
genuinely different means. Merging them would be a shared name over two
algorithms, which is what this change is trying to get rid of.

## File map

| File | Change |
|------|--------|
| `compiler/vm/src/freewheeling.rs` | Add `resolve_cycle_time`; make the two helpers private; tests for the sequence |
| `compiler/vm/src/lib.rs` | Drop the two helpers from the public re-exports |
| `compiler/mcp/src/runner.rs` | Replace the match + read-back with one call |
| `compiler/vm-cli/src/dap/server.rs` | Replace the rewrite block + filter chain with one call |

## Tasks

- [ ] Add `resolve_cycle_time` with tests covering each arm
- [ ] Make `assume_freewheeling_interval` and `first_task_interval` private
- [ ] Move `runner.rs` onto it, preserving today's error text and zero result
- [ ] Move `server.rs` onto it, preserving the default and zero fallbacks
- [ ] `cd compiler && just`

## Verification

Behavior is unchanged, so the existing tests are the specification: the MCP
conformance test for REQ-TOL-mcp-049, and the debugger timing tests added with
the previous change, must pass untouched.
