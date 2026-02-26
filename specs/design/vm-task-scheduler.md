# VM Task Scheduler Design (Phase 2)

**Context:** Phase 2 of the IEC 61131-3 task support design (`specs/design/61131-task-support.md`). Phases 0 (parser) and 1 (container format) are complete.

**Goal:** Implement a cooperative task scheduler in the VM that reads the task table from the container, executes cyclic and freewheeling tasks in priority order, enforces watchdog timeouts, and supports continuous execution.

## Decisions

- **Cyclic + freewheeling tasks** both supported from the start (freewheeling is the default synthesized task)
- **Stub I/O phases** — INPUT_FREEZE and OUTPUT_FLUSH are structurally present as no-ops
- **Continuous mode is the CLI default** — `--scans N` for bounded execution
- **Watchdog enforcement** included (trap if task exceeds `watchdog_us`)
- **Scheduler inside VmRunning** (Approach A) — scheduler is a field of VmRunning, not an external driver

## State Machine

States are things you can be in. Transitions move between states.

```
Ready --start()--> Running --stop()--> Stopped
                           --fault()--> Faulted
```

### Types

```rust
struct VmReady { ... }
struct VmRunning { ... }
struct VmStopped { ... }
struct VmFaulted { ... }
```

### Transitions and Operations

```rust
impl VmReady {
    fn start(self) -> VmRunning;        // transition
}

impl VmRunning {
    fn run_round(&mut self) -> Result<(), Trap>;  // operation within state
    fn request_stop(&self);             // sets internal AtomicBool
    fn stop_requested(&self) -> bool;
    fn stop(self) -> VmStopped;         // transition (clean shutdown)
    fn fault(self, trap: Trap) -> VmFaulted;  // transition (on trap)
}

impl VmStopped {
    fn read_variable(&self, index: u16) -> Result<Slot, Trap>;
    fn scan_count(&self) -> u64;
}

impl VmFaulted {
    fn trap(&self) -> &Trap;
    fn task_id(&self) -> u16;
    fn instance_id(&self) -> u16;
    fn read_variable(&self, index: u16) -> Result<Slot, Trap>;
}
```

The CLI drives the loop externally:

```rust
let mut running = ready.start();
// install signal handler that calls running.request_stop() via Arc<AtomicBool>
while !running.stop_requested() {
    if let Err(trap) = running.run_round() {
        let faulted = running.fault(trap);
        report(&faulted);
        return;
    }
}
let stopped = running.stop();
```

## TaskScheduler

`TaskScheduler` is a struct owned by `VmRunning`, initialized from the container's task table during `start()`.

```
TaskScheduler
├── task_states: Vec<TaskState>
├── program_instances: Vec<ProgramInstanceState>
└── shared_globals_size: u16

TaskState
├── task_id: u16
├── priority: u16
├── task_type: TaskType  (Cyclic | Freewheeling)
├── interval_us: u64
├── watchdog_us: u64
├── enabled: bool
├── next_due_us: i64  (0 for freewheeling; monotonic for cyclic)
├── scan_count: u64
├── last_execute_us: u64
├── max_execute_us: u64
└── overrun_count: u64

ProgramInstanceState
├── instance_id: u16
├── task_id: u16
├── entry_function_id: u16
├── var_table_offset: u16
└── var_table_count: u16
```

Cyclic tasks get `next_due_us = 0` (immediately due on first round). Freewheeling tasks are always ready.

## Scheduling Round

Each `run_round()`:

1. `current_time = monotonic_clock_us()`
2. Collect ready tasks:
   - Cyclic: `current_time >= next_due_us`
   - Freewheeling: always ready
   - Skip disabled tasks
3. Sort ready by `(priority ASC, task_id ASC)`
4. For each ready task:
   a. `start_time = monotonic_clock_us()`
   b. `input_freeze()` — stub no-op
   c. For each program instance belonging to this task (in declaration order):
      - `execute(instance.entry_function_id, ...)`
   d. `output_flush()` — stub no-op
   e. `elapsed = monotonic_clock_us() - start_time`
   f. Update task_state: `last_execute_us`, `max_execute_us`, `scan_count`
   g. Watchdog: if `watchdog_us > 0 && elapsed > watchdog_us` → Trap
5. Update `next_due_us` for cyclic tasks that ran:
   - `next_due_us += interval_us`
   - If `next_due_us <= current_time`: increment `overrun_count`, realign to `current_time + interval_us`
6. If no tasks were ready: sleep until earliest `next_due_us`

Traps from any program instance abort the entire round. No further tasks execute and OUTPUT_FLUSH is skipped. This matches standard PLC behavior (Siemens, CODESYS, B&R all default to stopping on fault).

## Variable Table Partitioning

The variable table remains a flat `Vec<Slot>`. Partitioning is a compile-time concern — the compiler assigns non-overlapping index ranges:

```
┌─────────────────────────────────────┐  index 0
│ Shared Globals                      │
│   0 .. shared_globals_size - 1      │
├─────────────────────────────────────┤
│ Program Instance 0 variables        │
│   var_table_offset .. + count       │
├─────────────────────────────────────┤
│ Program Instance 1 variables        │
└─────────────────────────────────────┘
```

### Variable Access Scope Checking

A `VariableScope` is passed to `execute()` for each program instance. Every `LOAD_VAR`/`STORE_VAR` checks that the index is within the allowed range:

```rust
struct VariableScope {
    shared_globals_size: u16,
    instance_offset: u16,
    instance_count: u16,
}

fn check_access(index: u16, scope: &VariableScope) -> Result<(), Trap> {
    if index < scope.shared_globals_size
        || (index >= scope.instance_offset
            && index < scope.instance_offset + scope.instance_count)
    {
        Ok(())
    } else {
        Err(Trap::InvalidVariableIndex(index))
    }
}
```

Cost: two comparisons per variable access. Catches compiler bugs early instead of silently corrupting another instance's state.

The `execute()` signature changes to include the scope:

```rust
fn execute(bytecode, container, stack, variables, scope) -> Result<(), Trap>
```

## Trap Diagnostics

Traps describe *what* went wrong. The *where* is captured by the scheduler.

```rust
pub enum Trap {
    DivideByZero,
    StackOverflow,
    StackUnderflow,
    InvalidInstruction(u8),
    InvalidConstantIndex(u16),
    InvalidVariableIndex(u16),
    InvalidFunctionId(u16),
    WatchdogTimeout(u16),       // task_id
}
```

`VmFaulted` carries context:

```rust
struct VmFaulted {
    trap: Trap,
    task_id: u16,
    instance_id: u16,
    // also owns container, variables, scheduler for post-mortem inspection
}
```

The bytecode interpreter doesn't know about tasks — it returns a `Trap`, and the scheduler wraps it with context.

## CLI Changes

```
ironplcvm run [OPTIONS] <FILE>

Options:
  --dump-vars <PATH>     Write variable dump after execution
  --scans <N>            Run N scheduling rounds then stop
                         (default: continuous until Ctrl+C)
```

- No flags: continuous mode, Ctrl+C triggers `request_stop()`
- `--scans N`: runs N rounds then stops (`--scans 1` for old single-scan behavior)
- `--dump-vars`: writes variables after stopping (both modes)
- On trap: reports trap with task/instance context, exits non-zero
