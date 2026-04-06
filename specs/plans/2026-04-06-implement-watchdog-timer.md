# Implement Watchdog Timer

## Goal

Replace the watchdog timer stub in the VM with real wall-clock elapsed time tracking so that tasks exceeding their configured `watchdog_us` threshold trigger a `Trap::WatchdogTimeout`.

## Architecture

Use `std::time::Instant` to measure elapsed time before and after executing each task's program instances in `run_round()`. If `watchdog_us > 0` and the measured elapsed time exceeds the threshold, return `Err(FaultContext)` with `Trap::WatchdogTimeout`. Pass the real elapsed time to `record_execution` for accurate timing statistics.

## File Map

- `compiler/vm/src/vm.rs` — Add `Instant`-based timing around task execution in `run_round()`
- `compiler/vm/tests/scenarios.rs` — Add integration tests for watchdog timeout and disabled watchdog

## Tasks

- [x] Add `use std::time::Instant` import to `vm.rs`
- [x] Measure wall-clock time before/after task execution in `run_round()`
- [x] Compare elapsed time against `task.watchdog_us`; emit `Trap::WatchdogTimeout` on violation
- [x] Pass real elapsed to `record_execution` instead of hardcoded 0
- [x] Add test: watchdog fires when execution exceeds threshold
- [x] Add test: watchdog disabled (`watchdog_us = 0`) does not fire
- [x] Verify full CI pipeline passes
