# Plan: CLI Sleep for Cyclic Tasks

## Goal

Stop the VM CLI run loop from tight-spinning by sleeping until the next cyclic task is due. This reduces CPU usage from 100% to near-zero between task intervals.

## Architecture

The scheduler already tracks `next_due_us` per cyclic task and provides `TaskScheduler::next_due_us()` to return the earliest deadline. The CLI just needs to call it after each round and sleep for the remaining time.

## Design doc reference

- `specs/design/vm-task-scheduler.md` — step 6 of Scheduling Round: "If no tasks were ready: sleep until earliest `next_due_us`"

## File map

| File | Change |
|------|--------|
| `compiler/vm/src/scheduler.rs` | Remove `#[allow(dead_code)]` from `next_due_us()` |
| `compiler/vm/src/vm.rs` | Add `pub fn next_due_us(&self) -> Option<u64>` to `VmRunning` |
| `compiler/vm-cli/src/cli.rs` | Add sleep after `run_round()` using `std::thread::sleep` |

## Tasks

- [x] Write plan
- [ ] Remove `#[allow(dead_code)]` and outdated comment from `TaskScheduler::next_due_us()`
- [ ] Add `next_due_us()` to `VmRunning` (inline iterator over task_states)
- [ ] Add sleep logic to CLI `run()` loop after `run_round()`
- [ ] Run full CI pipeline (`cd compiler && just`)
