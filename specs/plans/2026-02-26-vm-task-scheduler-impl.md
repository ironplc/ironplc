# VM Task Scheduler Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a cooperative task scheduler to the VM that reads the task table, executes cyclic and freewheeling tasks in priority order, enforces watchdog timeouts, and supports continuous execution.

**Architecture:** `TaskScheduler` is a new struct owned by `VmRunning`, initialized from the container's task table. The state machine gains `VmStopped` and `VmFaulted` terminal states. `run_round()` replaces `run_single_scan()` — each round collects ready tasks, sorts by priority, and executes their program instances. The CLI defaults to continuous mode with `--scans N` for bounded execution. A `VariableScope` enforces per-instance variable partition boundaries at runtime.

**Tech Stack:** Rust, ironplc-vm crate, ironplc-container crate, ctrlc crate (signal handling)

---

### Task 1: Add WatchdogTimeout trap variant

**Files:**
- Modify: `compiler/vm/src/error.rs`

**Step 1: Write the failing test**

In `compiler/vm/src/error.rs`, add to the `#[cfg(test)] mod tests` block (create it if it doesn't exist — currently there are no tests in this file):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trap_display_when_watchdog_timeout_then_includes_task_id() {
        let trap = Trap::WatchdogTimeout(3);
        assert_eq!(format!("{trap}"), "watchdog timeout on task 3");
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd compiler && cargo test --package ironplc-vm -- trap_display_when_watchdog_timeout`
Expected: FAIL — `WatchdogTimeout` variant doesn't exist yet.

**Step 3: Add the variant and Display arm**

In `compiler/vm/src/error.rs`, add to the `Trap` enum:

```rust
WatchdogTimeout(u16),
```

Add to the `Display` impl:

```rust
Trap::WatchdogTimeout(id) => write!(f, "watchdog timeout on task {id}"),
```

**Step 4: Run test to verify it passes**

Run: `cd compiler && cargo test --package ironplc-vm -- trap_display_when_watchdog_timeout`
Expected: PASS

**Step 5: Commit**

```bash
git add compiler/vm/src/error.rs
git commit -m "Add WatchdogTimeout trap variant"
```

---

### Task 2: Add VariableScope to enforce partition boundaries

**Files:**
- Modify: `compiler/vm/src/variable_table.rs`
- Modify: `compiler/vm/src/vm.rs`

**Step 1: Write failing tests for scope checking**

In `compiler/vm/src/variable_table.rs`, add these tests to the existing `mod tests`:

```rust
#[test]
fn scope_check_when_index_in_shared_globals_then_ok() {
    let scope = VariableScope {
        shared_globals_size: 4,
        instance_offset: 10,
        instance_count: 5,
    };
    assert!(scope.check_access(0).is_ok());
    assert!(scope.check_access(3).is_ok());
}

#[test]
fn scope_check_when_index_in_instance_range_then_ok() {
    let scope = VariableScope {
        shared_globals_size: 4,
        instance_offset: 10,
        instance_count: 5,
    };
    assert!(scope.check_access(10).is_ok());
    assert!(scope.check_access(14).is_ok());
}

#[test]
fn scope_check_when_index_between_globals_and_instance_then_error() {
    let scope = VariableScope {
        shared_globals_size: 4,
        instance_offset: 10,
        instance_count: 5,
    };
    assert!(scope.check_access(5).is_err());
    assert!(scope.check_access(9).is_err());
}

#[test]
fn scope_check_when_index_past_instance_then_error() {
    let scope = VariableScope {
        shared_globals_size: 4,
        instance_offset: 10,
        instance_count: 5,
    };
    assert!(scope.check_access(15).is_err());
}

#[test]
fn scope_check_when_permissive_then_all_ok() {
    let scope = VariableScope::permissive(10);
    for i in 0..10 {
        assert!(scope.check_access(i).is_ok());
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cd compiler && cargo test --package ironplc-vm -- scope_check`
Expected: FAIL — `VariableScope` doesn't exist yet.

**Step 3: Implement VariableScope**

In `compiler/vm/src/variable_table.rs`, add above the `VariableTable` struct:

```rust
/// Defines which variable indices a program instance may access.
///
/// Each program instance can access shared globals (indices 0..shared_globals_size)
/// and its own partition (indices instance_offset..instance_offset+instance_count).
pub struct VariableScope {
    pub shared_globals_size: u16,
    pub instance_offset: u16,
    pub instance_count: u16,
}

impl VariableScope {
    /// Creates a permissive scope that allows access to all `num_variables` slots.
    /// Used when there is only one program instance (the common case).
    pub fn permissive(num_variables: u16) -> Self {
        VariableScope {
            shared_globals_size: num_variables,
            instance_offset: 0,
            instance_count: num_variables,
        }
    }

    /// Checks whether a variable index is within this scope's allowed range.
    pub fn check_access(&self, index: u16) -> Result<(), Trap> {
        if index < self.shared_globals_size
            || (index >= self.instance_offset
                && index < self.instance_offset + self.instance_count)
        {
            Ok(())
        } else {
            Err(Trap::InvalidVariableIndex(index))
        }
    }
}
```

**Step 4: Run scope tests to verify they pass**

Run: `cd compiler && cargo test --package ironplc-vm -- scope_check`
Expected: PASS (all 5 tests)

**Step 5: Integrate scope into execute()**

In `compiler/vm/src/vm.rs`, change the `execute` function signature to accept a scope:

```rust
fn execute(
    bytecode: &[u8],
    container: &Container,
    stack: &mut OperandStack,
    variables: &mut VariableTable,
    scope: &VariableScope,
) -> Result<(), Trap> {
```

Add scope checks to the `LOAD_VAR_I32` and `STORE_VAR_I32` arms, before the variable access:

```rust
opcode::LOAD_VAR_I32 => {
    let index = read_u16_le(bytecode, &mut pc);
    scope.check_access(index)?;
    let slot = variables.load(index)?;
    stack.push(slot)?;
}
opcode::STORE_VAR_I32 => {
    let index = read_u16_le(bytecode, &mut pc);
    scope.check_access(index)?;
    let slot = stack.pop()?;
    variables.store(index, slot)?;
}
```

Update `run_single_scan` to pass a permissive scope:

```rust
pub fn run_single_scan(&mut self) -> Result<(), Trap> {
    let entry_id: u16 = 0;
    let bytecode = self
        .container
        .code
        .get_function_bytecode(entry_id)
        .ok_or(Trap::InvalidFunctionId(entry_id))?;

    let scope = VariableScope::permissive(self.container.header.num_variables);

    execute(
        bytecode,
        &self.container,
        &mut self.stack,
        &mut self.variables,
        &scope,
    )?;

    self.scan_count += 1;
    Ok(())
}
```

Add the import at the top of `vm.rs`:

```rust
use crate::variable_table::VariableScope;
```

**Step 6: Run all VM tests to verify nothing broke**

Run: `cd compiler && cargo test --package ironplc-vm`
Expected: All existing tests pass.

**Step 7: Commit**

```bash
git add compiler/vm/src/variable_table.rs compiler/vm/src/vm.rs
git commit -m "Add VariableScope for per-instance variable boundary checking"
```

---

### Task 3: Create TaskScheduler module

**Files:**
- Create: `compiler/vm/src/scheduler.rs`
- Modify: `compiler/vm/src/lib.rs`

**Step 1: Write failing tests for scheduler construction**

Create `compiler/vm/src/scheduler.rs` with the test module first:

```rust
use ironplc_container::{TaskEntry, TaskTable, TaskType, ProgramInstanceEntry};

/// Per-task runtime state tracked by the scheduler.
#[derive(Clone, Debug)]
pub struct TaskState {
    pub task_id: u16,
    pub priority: u16,
    pub task_type: TaskType,
    pub interval_us: u64,
    pub watchdog_us: u64,
    pub enabled: bool,
    pub next_due_us: u64,
    pub scan_count: u64,
    pub last_execute_us: u64,
    pub max_execute_us: u64,
    pub overrun_count: u64,
}

/// Per-program-instance runtime state.
#[derive(Clone, Debug)]
pub struct ProgramInstanceState {
    pub instance_id: u16,
    pub task_id: u16,
    pub entry_function_id: u16,
    pub var_table_offset: u16,
    pub var_table_count: u16,
}

/// Cooperative task scheduler that determines which tasks to execute each round.
///
/// The scheduler is time-agnostic: callers pass the current time as a `u64`
/// microsecond value. This makes the scheduler fully testable without mocking clocks.
pub struct TaskScheduler {
    pub task_states: Vec<TaskState>,
    pub program_instances: Vec<ProgramInstanceState>,
    pub shared_globals_size: u16,
}

impl TaskScheduler {
    /// Builds a scheduler from a container's task table.
    pub fn from_task_table(table: &TaskTable) -> Self {
        let task_states = table
            .tasks
            .iter()
            .map(|t| TaskState {
                task_id: t.task_id,
                priority: t.priority,
                task_type: t.task_type,
                interval_us: t.interval_us,
                watchdog_us: t.watchdog_us,
                enabled: (t.flags & 0x01) != 0,
                next_due_us: 0, // all tasks are immediately due on first round
                scan_count: 0,
                last_execute_us: 0,
                max_execute_us: 0,
                overrun_count: 0,
            })
            .collect();

        let program_instances = table
            .programs
            .iter()
            .map(|p| ProgramInstanceState {
                instance_id: p.instance_id,
                task_id: p.task_id,
                entry_function_id: p.entry_function_id,
                var_table_offset: p.var_table_offset,
                var_table_count: p.var_table_count,
            })
            .collect();

        TaskScheduler {
            task_states,
            program_instances,
            shared_globals_size: table.shared_globals_size,
        }
    }

    /// Returns indices into `task_states` for tasks that are ready to execute,
    /// sorted by priority (ascending) then task_id (ascending).
    pub fn collect_ready_tasks(&self, current_time_us: u64) -> Vec<usize> {
        let mut ready: Vec<usize> = self
            .task_states
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                if !t.enabled {
                    return false;
                }
                match t.task_type {
                    TaskType::Freewheeling => true,
                    TaskType::Cyclic => current_time_us >= t.next_due_us,
                    TaskType::Event => false, // Phase 3
                }
            })
            .map(|(i, _)| i)
            .collect();

        ready.sort_by(|&a, &b| {
            let ta = &self.task_states[a];
            let tb = &self.task_states[b];
            ta.priority
                .cmp(&tb.priority)
                .then(ta.task_id.cmp(&tb.task_id))
        });

        ready
    }

    /// Records that a task executed, updating timing and overrun tracking.
    pub fn record_execution(
        &mut self,
        task_index: usize,
        elapsed_us: u64,
        current_time_us: u64,
    ) {
        let task = &mut self.task_states[task_index];
        task.scan_count += 1;
        task.last_execute_us = elapsed_us;
        if elapsed_us > task.max_execute_us {
            task.max_execute_us = elapsed_us;
        }

        if task.task_type == TaskType::Cyclic {
            task.next_due_us += task.interval_us;
            if task.next_due_us <= current_time_us {
                task.overrun_count += 1;
                task.next_due_us = current_time_us + task.interval_us;
            }
        }
    }

    /// Returns the program instances associated with a task, in declaration order.
    pub fn programs_for_task(&self, task_id: u16) -> Vec<&ProgramInstanceState> {
        self.program_instances
            .iter()
            .filter(|p| p.task_id == task_id)
            .collect()
    }

    /// Returns the earliest `next_due_us` across all enabled cyclic tasks,
    /// or `None` if no cyclic tasks exist.
    pub fn next_due_us(&self) -> Option<u64> {
        self.task_states
            .iter()
            .filter(|t| t.enabled && t.task_type == TaskType::Cyclic)
            .map(|t| t.next_due_us)
            .min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn freewheeling_task_table() -> TaskTable {
        TaskTable {
            shared_globals_size: 0,
            tasks: vec![TaskEntry {
                task_id: 0,
                priority: 0,
                task_type: TaskType::Freewheeling,
                flags: 0x01,
                interval_us: 0,
                single_var_index: 0xFFFF,
                watchdog_us: 0,
                input_image_offset: 0,
                output_image_offset: 0,
                reserved: [0; 4],
            }],
            programs: vec![ProgramInstanceEntry {
                instance_id: 0,
                task_id: 0,
                entry_function_id: 0,
                var_table_offset: 0,
                var_table_count: 2,
                fb_instance_offset: 0,
                fb_instance_count: 0,
                reserved: 0,
            }],
        }
    }

    fn two_cyclic_tasks_table() -> TaskTable {
        TaskTable {
            shared_globals_size: 2,
            tasks: vec![
                TaskEntry {
                    task_id: 0,
                    priority: 5,
                    task_type: TaskType::Cyclic,
                    flags: 0x01,
                    interval_us: 100_000, // 100ms
                    single_var_index: 0xFFFF,
                    watchdog_us: 0,
                    input_image_offset: 0,
                    output_image_offset: 0,
                    reserved: [0; 4],
                },
                TaskEntry {
                    task_id: 1,
                    priority: 0, // higher priority
                    task_type: TaskType::Cyclic,
                    flags: 0x01,
                    interval_us: 10_000, // 10ms
                    single_var_index: 0xFFFF,
                    watchdog_us: 0,
                    input_image_offset: 0,
                    output_image_offset: 0,
                    reserved: [0; 4],
                },
            ],
            programs: vec![
                ProgramInstanceEntry {
                    instance_id: 0,
                    task_id: 0,
                    entry_function_id: 0,
                    var_table_offset: 2,
                    var_table_count: 3,
                    fb_instance_offset: 0,
                    fb_instance_count: 0,
                    reserved: 0,
                },
                ProgramInstanceEntry {
                    instance_id: 1,
                    task_id: 1,
                    entry_function_id: 1,
                    var_table_offset: 5,
                    var_table_count: 3,
                    fb_instance_offset: 0,
                    fb_instance_count: 0,
                    reserved: 0,
                },
            ],
        }
    }

    #[test]
    fn from_task_table_when_freewheeling_then_one_task_one_program() {
        let sched = TaskScheduler::from_task_table(&freewheeling_task_table());
        assert_eq!(sched.task_states.len(), 1);
        assert_eq!(sched.program_instances.len(), 1);
        assert_eq!(sched.task_states[0].task_type, TaskType::Freewheeling);
        assert!(sched.task_states[0].enabled);
    }

    #[test]
    fn collect_ready_when_freewheeling_then_always_ready() {
        let sched = TaskScheduler::from_task_table(&freewheeling_task_table());
        let ready = sched.collect_ready_tasks(0);
        assert_eq!(ready, vec![0]);
    }

    #[test]
    fn collect_ready_when_cyclic_at_time_zero_then_all_due() {
        let sched = TaskScheduler::from_task_table(&two_cyclic_tasks_table());
        let ready = sched.collect_ready_tasks(0);
        // task 1 (priority 0) before task 0 (priority 5)
        assert_eq!(ready, vec![1, 0]);
    }

    #[test]
    fn collect_ready_when_cyclic_not_due_then_empty() {
        let mut sched = TaskScheduler::from_task_table(&two_cyclic_tasks_table());
        // Simulate both tasks ran at time 0
        sched.record_execution(0, 100, 0);
        sched.record_execution(1, 100, 0);
        // At time 5000 (5ms), neither is due (10ms and 100ms intervals)
        let ready = sched.collect_ready_tasks(5_000);
        assert!(ready.is_empty());
    }

    #[test]
    fn collect_ready_when_fast_task_due_slow_not_then_only_fast() {
        let mut sched = TaskScheduler::from_task_table(&two_cyclic_tasks_table());
        sched.record_execution(0, 100, 0);
        sched.record_execution(1, 100, 0);
        // At 10ms: task 1 (10ms interval) is due, task 0 (100ms) is not
        let ready = sched.collect_ready_tasks(10_000);
        assert_eq!(ready, vec![1]);
    }

    #[test]
    fn collect_ready_when_task_disabled_then_skipped() {
        let mut table = freewheeling_task_table();
        table.tasks[0].flags = 0x00; // disabled
        let sched = TaskScheduler::from_task_table(&table);
        let ready = sched.collect_ready_tasks(0);
        assert!(ready.is_empty());
    }

    #[test]
    fn record_execution_when_cyclic_overrun_then_realigns() {
        let mut sched = TaskScheduler::from_task_table(&two_cyclic_tasks_table());
        // Task 1 has 10ms interval. Simulate it ran at time 0.
        sched.record_execution(1, 100, 0);
        assert_eq!(sched.task_states[1].next_due_us, 10_000);

        // Now simulate we're at 25ms (missed the 10ms and 20ms deadlines)
        sched.record_execution(1, 100, 25_000);
        // Should realign: next_due = 25_000 + 10_000 = 35_000
        assert_eq!(sched.task_states[1].next_due_us, 35_000);
        assert_eq!(sched.task_states[1].overrun_count, 1);
    }

    #[test]
    fn programs_for_task_when_two_tasks_then_returns_correct_programs() {
        let sched = TaskScheduler::from_task_table(&two_cyclic_tasks_table());
        let progs = sched.programs_for_task(1);
        assert_eq!(progs.len(), 1);
        assert_eq!(progs[0].entry_function_id, 1);
    }

    #[test]
    fn next_due_when_cyclic_tasks_then_returns_earliest() {
        let mut sched = TaskScheduler::from_task_table(&two_cyclic_tasks_table());
        sched.record_execution(0, 100, 0); // next_due = 100_000
        sched.record_execution(1, 100, 0); // next_due = 10_000
        assert_eq!(sched.next_due_us(), Some(10_000));
    }

    #[test]
    fn next_due_when_only_freewheeling_then_none() {
        let sched = TaskScheduler::from_task_table(&freewheeling_task_table());
        assert_eq!(sched.next_due_us(), None);
    }
}
```

**Step 2: Register the module in lib.rs**

In `compiler/vm/src/lib.rs`, add:

```rust
pub(crate) mod scheduler;
```

**Step 3: Run tests to verify they pass**

Run: `cd compiler && cargo test --package ironplc-vm -- scheduler`
Expected: All 10 scheduler tests pass.

**Step 4: Commit**

```bash
git add compiler/vm/src/scheduler.rs compiler/vm/src/lib.rs
git commit -m "Add TaskScheduler with scheduling logic and timing"
```

---

### Task 4: Add VmStopped, VmFaulted, and StopHandle types

**Files:**
- Modify: `compiler/vm/src/vm.rs`
- Modify: `compiler/vm/src/lib.rs`

**Step 1: Write tests for the new types**

In `compiler/vm/src/vm.rs`, add to the existing `mod tests`:

```rust
#[test]
fn vm_stop_handle_when_request_stop_then_stop_requested() {
    let vm = Vm::new().load(steel_thread_container()).start();
    let handle = vm.stop_handle();
    assert!(!vm.stop_requested());
    handle.request_stop();
    assert!(vm.stop_requested());
}

#[test]
fn vm_stop_when_called_then_returns_stopped() {
    let vm = Vm::new().load(steel_thread_container()).start();
    let stopped = vm.stop();
    assert_eq!(stopped.read_variable(0).unwrap(), 0); // not yet executed
}

#[test]
fn vm_fault_when_called_then_returns_faulted_with_context() {
    let vm = Vm::new().load(steel_thread_container()).start();
    let faulted = vm.fault(Trap::WatchdogTimeout(3), 3, 1);
    assert_eq!(*faulted.trap(), Trap::WatchdogTimeout(3));
    assert_eq!(faulted.task_id(), 3);
    assert_eq!(faulted.instance_id(), 1);
}
```

**Step 2: Run tests to verify they fail**

Run: `cd compiler && cargo test --package ironplc-vm -- vm_stop`
Expected: FAIL — `stop_handle`, `VmStopped`, etc. don't exist yet.

**Step 3: Implement the types**

In `compiler/vm/src/vm.rs`, add imports at the top:

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
```

Add `StopHandle` struct:

```rust
/// A cloneable handle for requesting the VM to stop.
/// Used by signal handlers to stop the VM from another context.
#[derive(Clone)]
pub struct StopHandle {
    flag: Arc<AtomicBool>,
}

impl StopHandle {
    /// Requests the VM to stop after the current scheduling round.
    pub fn request_stop(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }
}
```

Add `stop_flag` field to `VmRunning` and the stop-related methods:

```rust
pub struct VmRunning {
    container: Container,
    stack: OperandStack,
    variables: VariableTable,
    scan_count: u64,
    stop_flag: Arc<AtomicBool>,
}
```

Update `VmReady::start()`:

```rust
pub fn start(self) -> VmRunning {
    VmRunning {
        container: self.container,
        stack: self.stack,
        variables: self.variables,
        scan_count: 0,
        stop_flag: Arc::new(AtomicBool::new(false)),
    }
}
```

Add methods to `VmRunning`:

```rust
/// Returns a cloneable handle that can request the VM to stop.
pub fn stop_handle(&self) -> StopHandle {
    StopHandle {
        flag: self.stop_flag.clone(),
    }
}

/// Returns true if a stop has been requested.
pub fn stop_requested(&self) -> bool {
    self.stop_flag.load(Ordering::Relaxed)
}

/// Requests the VM to stop after the current round.
pub fn request_stop(&self) {
    self.stop_flag.store(true, Ordering::Relaxed);
}

/// Transitions to the stopped state (clean shutdown).
pub fn stop(self) -> VmStopped {
    VmStopped {
        container: self.container,
        variables: self.variables,
        scan_count: self.scan_count,
    }
}

/// Transitions to the faulted state (trap occurred).
pub fn fault(self, trap: Trap, task_id: u16, instance_id: u16) -> VmFaulted {
    VmFaulted {
        trap,
        task_id,
        instance_id,
        container: self.container,
        variables: self.variables,
    }
}
```

Add `VmStopped`:

```rust
/// A VM that has been cleanly stopped.
pub struct VmStopped {
    container: Container,
    variables: VariableTable,
    scan_count: u64,
}

impl VmStopped {
    /// Reads a variable value as an i32.
    pub fn read_variable(&self, index: u16) -> Result<i32, Trap> {
        let slot = self.variables.load(index)?;
        Ok(slot.as_i32())
    }

    /// Returns the number of variable slots.
    pub fn num_variables(&self) -> u16 {
        self.container.header.num_variables
    }

    /// Returns the total number of completed scheduling rounds.
    pub fn scan_count(&self) -> u64 {
        self.scan_count
    }
}
```

Add `VmFaulted`:

```rust
/// A VM that has stopped due to a trap.
pub struct VmFaulted {
    trap: Trap,
    task_id: u16,
    instance_id: u16,
    container: Container,
    variables: VariableTable,
}

impl VmFaulted {
    /// Returns the trap that caused the fault.
    pub fn trap(&self) -> &Trap {
        &self.trap
    }

    /// Returns the task that was executing when the trap occurred.
    pub fn task_id(&self) -> u16 {
        self.task_id
    }

    /// Returns the program instance that was executing when the trap occurred.
    pub fn instance_id(&self) -> u16 {
        self.instance_id
    }

    /// Reads a variable value as an i32.
    pub fn read_variable(&self, index: u16) -> Result<i32, Trap> {
        let slot = self.variables.load(index)?;
        Ok(slot.as_i32())
    }

    /// Returns the number of variable slots.
    pub fn num_variables(&self) -> u16 {
        self.container.header.num_variables
    }
}
```

**Step 4: Update lib.rs exports**

In `compiler/vm/src/lib.rs`, change the `vm` exports line to:

```rust
pub use vm::{StopHandle, Vm, VmFaulted, VmReady, VmRunning, VmStopped};
```

**Step 5: Run tests to verify they pass**

Run: `cd compiler && cargo test --package ironplc-vm`
Expected: All tests pass (existing + 3 new).

**Step 6: Commit**

```bash
git add compiler/vm/src/vm.rs compiler/vm/src/lib.rs
git commit -m "Add VmStopped, VmFaulted, and StopHandle types"
```

---

### Task 5: Integrate scheduler into VmRunning and replace run_single_scan

**Files:**
- Modify: `compiler/vm/src/vm.rs`
- Modify: `compiler/vm/tests/steel_thread.rs`

**Step 1: Write test for run_round with default freewheeling task**

In `compiler/vm/src/vm.rs`, add to `mod tests`:

```rust
#[test]
fn vm_run_round_when_steel_thread_then_x_is_10_y_is_42() {
    let mut vm = Vm::new().load(steel_thread_container()).start();

    vm.run_round().unwrap();

    assert_eq!(vm.read_variable(0).unwrap(), 10);
    assert_eq!(vm.read_variable(1).unwrap(), 42);
}
```

**Step 2: Run test to verify it fails**

Run: `cd compiler && cargo test --package ironplc-vm -- vm_run_round_when_steel_thread`
Expected: FAIL — `run_round` doesn't exist yet.

**Step 3: Add scheduler to VmRunning and implement run_round**

In `compiler/vm/src/vm.rs`, add import:

```rust
use std::time::Instant;
use crate::scheduler::TaskScheduler;
use crate::variable_table::VariableScope;
```

Update `VmRunning` struct:

```rust
pub struct VmRunning {
    container: Container,
    stack: OperandStack,
    variables: VariableTable,
    scheduler: TaskScheduler,
    scan_count: u64,
    stop_flag: Arc<AtomicBool>,
    start_instant: Instant,
}
```

Update `VmReady::start()`:

```rust
pub fn start(self) -> VmRunning {
    let scheduler = TaskScheduler::from_task_table(&self.container.task_table);
    VmRunning {
        container: self.container,
        stack: self.stack,
        variables: self.variables,
        scheduler,
        scan_count: 0,
        stop_flag: Arc::new(AtomicBool::new(false)),
        start_instant: Instant::now(),
    }
}
```

Implement `run_round`:

```rust
/// Executes one scheduling round: collects ready tasks, executes them
/// in priority order, and updates timing.
///
/// Returns `Ok(())` if the round completes. Returns `Err(Trap)` if
/// a trap occurs during execution. The caller should transition to
/// `VmFaulted` on trap.
pub fn run_round(&mut self) -> Result<(), Trap> {
    let current_us = self.start_instant.elapsed().as_micros() as u64;

    let ready = self.scheduler.collect_ready_tasks(current_us);

    if ready.is_empty() {
        // Sleep until the next cyclic task is due
        if let Some(next_due) = self.scheduler.next_due_us() {
            if next_due > current_us {
                std::thread::sleep(std::time::Duration::from_micros(next_due - current_us));
            }
        }
        return Ok(());
    }

    // Stub: INPUT_FREEZE (no-op)

    for &task_idx in &ready {
        let task_state = &self.scheduler.task_states[task_idx];
        let task_id = task_state.task_id;
        let watchdog_us = task_state.watchdog_us;

        let programs = self.scheduler.programs_for_task(task_id);
        let task_start = self.start_instant.elapsed().as_micros() as u64;

        for prog in &programs {
            let bytecode = self
                .container
                .code
                .get_function_bytecode(prog.entry_function_id)
                .ok_or(Trap::InvalidFunctionId(prog.entry_function_id))?;

            let scope = VariableScope {
                shared_globals_size: self.scheduler.shared_globals_size,
                instance_offset: prog.var_table_offset,
                instance_count: prog.var_table_count,
            };

            execute(
                bytecode,
                &self.container,
                &mut self.stack,
                &mut self.variables,
                &scope,
            )?;
        }

        let task_elapsed = self.start_instant.elapsed().as_micros() as u64 - task_start;

        // Watchdog check
        if watchdog_us > 0 && task_elapsed > watchdog_us {
            return Err(Trap::WatchdogTimeout(task_id));
        }

        self.scheduler
            .record_execution(task_idx, task_elapsed, task_start);
    }

    // Stub: OUTPUT_FLUSH (no-op)

    self.scan_count += 1;
    Ok(())
}
```

**Step 4: Remove run_single_scan and update callers**

Remove `run_single_scan` from `VmRunning`. Update existing tests in `vm.rs`:

Replace `vm_run_single_scan_when_steel_thread_then_x_is_10_y_is_42`:
```rust
#[test]
fn vm_run_round_when_steel_thread_then_x_is_10_y_is_42() {
    let mut vm = Vm::new().load(steel_thread_container()).start();

    vm.run_round().unwrap();

    assert_eq!(vm.read_variable(0).unwrap(), 10);
    assert_eq!(vm.read_variable(1).unwrap(), 42);
}
```

Replace `vm_run_single_scan_when_invalid_opcode_then_trap`:
```rust
#[test]
fn vm_run_round_when_invalid_opcode_then_trap() {
    let bytecode = vec![0xFF]; // invalid opcode
    let container = ContainerBuilder::new()
        .num_variables(0)
        .add_function(0, &bytecode, 1, 0)
        .build();

    let mut vm = Vm::new().load(container).start();

    let result = vm.run_round();

    assert!(matches!(result, Err(Trap::InvalidInstruction(0xFF))));
}
```

**Step 5: Update steel_thread integration test**

In `compiler/vm/tests/steel_thread.rs`, replace `run_single_scan()` with `run_round()`:

```rust
let mut vm = Vm::new().load(loaded).start();
vm.run_round().unwrap();
```

**Step 6: Run all VM tests**

Run: `cd compiler && cargo test --package ironplc-vm`
Expected: All tests pass.

**Step 7: Commit**

```bash
git add compiler/vm/src/vm.rs compiler/vm/tests/steel_thread.rs
git commit -m "Replace run_single_scan with scheduler-driven run_round"
```

---

### Task 6: Update CLI for continuous execution

**Files:**
- Modify: `compiler/vm/Cargo.toml`
- Modify: `compiler/vm/bin/main.rs`
- Modify: `compiler/vm/src/cli.rs`
- Modify: `compiler/vm/tests/cli.rs`

**Step 1: Add ctrlc dependency**

In `compiler/vm/Cargo.toml`, add to `[dependencies]`:

```toml
ctrlc = "3"
```

**Step 2: Update CLI argument parsing**

In `compiler/vm/bin/main.rs`, update the `Run` variant:

```rust
/// Loads and executes a bytecode container file.
Run {
    /// Path to the bytecode container file (.iplc).
    file: PathBuf,

    /// Write variable dump to the specified file after execution.
    #[arg(long)]
    dump_vars: Option<PathBuf>,

    /// Run N scheduling rounds then stop (default: continuous until Ctrl+C).
    #[arg(long)]
    scans: Option<u64>,
},
```

Update the match arm:

```rust
Action::Run { file, dump_vars, scans } => cli::run(&file, dump_vars.as_deref(), scans),
```

**Step 3: Rewrite cli::run()**

Replace the entire contents of `compiler/vm/src/cli.rs`:

```rust
//! Implements the command line behavior.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::vm::Vm;

/// Loads a container file and executes it.
///
/// When `scans` is `Some(n)`, runs exactly `n` scheduling rounds.
/// When `scans` is `None`, runs continuously until Ctrl+C.
/// When `dump_vars` is `Some(path)`, writes all variable values after stopping.
pub fn run(path: &Path, dump_vars: Option<&Path>, scans: Option<u64>) -> Result<(), String> {
    let mut file =
        File::open(path).map_err(|e| format!("Unable to open {}: {}", path.display(), e))?;

    let container = ironplc_container::Container::read_from(&mut file)
        .map_err(|e| format!("Unable to read container {}: {e}", path.display()))?;

    let mut running = Vm::new().load(container).start();

    // Install signal handler for clean shutdown
    let handle = running.stop_handle();
    ctrlc::set_handler(move || handle.request_stop())
        .map_err(|e| format!("Failed to set signal handler: {e}"))?;

    let mut rounds = 0u64;
    loop {
        if running.stop_requested() {
            break;
        }
        if let Some(max) = scans {
            if rounds >= max {
                break;
            }
        }
        if let Err(trap) = running.run_round() {
            let faulted = running.fault(trap, 0, 0);
            let err_msg = format!(
                "VM trap: {} (task {}, instance {})",
                faulted.trap(),
                faulted.task_id(),
                faulted.instance_id()
            );
            if let Some(dump_path) = dump_vars {
                dump_variables_faulted(&faulted, dump_path)?;
            }
            return Err(err_msg);
        }
        rounds += 1;
    }

    let stopped = running.stop();

    if let Some(dump_path) = dump_vars {
        dump_variables_stopped(&stopped, dump_path)?;
    }

    Ok(())
}

fn dump_variables_stopped(
    stopped: &crate::vm::VmStopped,
    dump_path: &Path,
) -> Result<(), String> {
    let num_vars = stopped.num_variables();
    let mut out = File::create(dump_path)
        .map_err(|e| format!("Unable to create dump file {}: {e}", dump_path.display()))?;
    for i in 0..num_vars {
        let value = stopped
            .read_variable(i)
            .map_err(|e| format!("Unable to read variable {i}: {e}"))?;
        writeln!(out, "var[{i}]: {value}")
            .map_err(|e| format!("Unable to write dump file: {e}"))?;
    }
    Ok(())
}

fn dump_variables_faulted(
    faulted: &crate::vm::VmFaulted,
    dump_path: &Path,
) -> Result<(), String> {
    let num_vars = faulted.num_variables();
    let mut out = File::create(dump_path)
        .map_err(|e| format!("Unable to create dump file {}: {e}", dump_path.display()))?;
    for i in 0..num_vars {
        let value = faulted
            .read_variable(i)
            .map_err(|e| format!("Unable to read variable {i}: {e}"))?;
        writeln!(out, "var[{i}]: {value}")
            .map_err(|e| format!("Unable to write dump file: {e}"))?;
    }
    Ok(())
}
```

Note: The `fault(trap, 0, 0)` call uses placeholder task/instance IDs. A follow-up improvement would thread the actual task/instance context through the run_round error path (e.g., by wrapping Trap in a struct that includes context). For now, this matches the existing behavior where traps don't carry context.

**Step 4: Update CLI tests — add `--scans 1`**

In `compiler/vm/tests/cli.rs`, update every test that calls `ironplcvm run` to include `--scans 1`:

In `run_when_valid_container_file_then_ok`:
```rust
cmd.arg("run").arg(&container_path).arg("--scans").arg("1");
```

In `run_when_valid_container_file_and_dump_vars_then_writes_variables`:
```rust
cmd.arg("run")
    .arg(&container_path)
    .arg("--dump-vars")
    .arg(&dump_path)
    .arg("--scans")
    .arg("1");
```

In `run_when_golden_container_file_then_ok`:
```rust
cmd.arg("run")
    .arg(&golden_path)
    .arg("--dump-vars")
    .arg(&dump_path)
    .arg("--scans")
    .arg("1");
```

The error tests (`run_when_file_not_found_then_err` and `run_when_invalid_file_then_err`) don't need `--scans` because they fail before the execution loop.

**Step 5: Run all tests**

Run: `cd compiler && cargo test --package ironplc-vm`
Expected: All tests pass.

**Step 6: Commit**

```bash
git add compiler/vm/Cargo.toml compiler/vm/bin/main.rs compiler/vm/src/cli.rs compiler/vm/tests/cli.rs
git commit -m "Update CLI for continuous execution with --scans option"
```

---

### Task 7: Improve fault context, regenerate golden file, and run full CI

**Files:**
- Modify: `compiler/vm/src/vm.rs`
- Modify: `compiler/vm/src/cli.rs`
- Modify: `compiler/vm/resources/test/steel_thread.iplc` (regenerated)

**Step 1: Thread task/instance context through traps**

The `run_round` method currently returns `Err(Trap)` which loses the task/instance context. Add a `FaultContext` struct to carry this through:

In `compiler/vm/src/vm.rs`, add:

```rust
/// Context for a fault that occurred during task execution.
pub struct FaultContext {
    pub trap: Trap,
    pub task_id: u16,
    pub instance_id: u16,
}
```

Change `run_round` return type to `Result<(), FaultContext>`:

```rust
pub fn run_round(&mut self) -> Result<(), FaultContext> {
```

Inside `run_round`, wrap trap returns with context. For traps during program execution:

```rust
execute(
    bytecode,
    &self.container,
    &mut self.stack,
    &mut self.variables,
    &scope,
).map_err(|trap| FaultContext {
    trap,
    task_id,
    instance_id: prog.instance_id,
})?;
```

For watchdog trap:
```rust
if watchdog_us > 0 && task_elapsed > watchdog_us {
    return Err(FaultContext {
        trap: Trap::WatchdogTimeout(task_id),
        task_id,
        instance_id: 0,
    });
}
```

Update `fault()` to accept `FaultContext`:

```rust
pub fn fault(self, ctx: FaultContext) -> VmFaulted {
    VmFaulted {
        trap: ctx.trap,
        task_id: ctx.task_id,
        instance_id: ctx.instance_id,
        container: self.container,
        variables: self.variables,
    }
}
```

Update `cli.rs` to use `FaultContext`:

```rust
if let Err(ctx) = running.run_round() {
    let faulted = running.fault(ctx);
    let err_msg = format!(
        "VM trap: {} (task {}, instance {})",
        faulted.trap(),
        faulted.task_id(),
        faulted.instance_id()
    );
    if let Some(dump_path) = dump_vars {
        dump_variables_faulted(&faulted, dump_path)?;
    }
    return Err(err_msg);
}
```

Update the vm.rs tests that check for trap returns:

```rust
#[test]
fn vm_run_round_when_invalid_opcode_then_trap() {
    let bytecode = vec![0xFF];
    let container = ContainerBuilder::new()
        .num_variables(0)
        .add_function(0, &bytecode, 1, 0)
        .build();

    let mut vm = Vm::new().load(container).start();

    let result = vm.run_round();

    assert!(result.is_err());
    let ctx = result.unwrap_err();
    assert!(matches!(ctx.trap, Trap::InvalidInstruction(0xFF)));
}
```

Update the `vm_fault_when_called_then_returns_faulted_with_context` test:

```rust
#[test]
fn vm_fault_when_called_then_returns_faulted_with_context() {
    let vm = Vm::new().load(steel_thread_container()).start();
    let ctx = FaultContext {
        trap: Trap::WatchdogTimeout(3),
        task_id: 3,
        instance_id: 1,
    };
    let faulted = vm.fault(ctx);
    assert_eq!(*faulted.trap(), Trap::WatchdogTimeout(3));
    assert_eq!(faulted.task_id(), 3);
    assert_eq!(faulted.instance_id(), 1);
}
```

Export `FaultContext` from `lib.rs`:

```rust
pub use vm::{FaultContext, StopHandle, Vm, VmFaulted, VmReady, VmRunning, VmStopped};
```

**Step 2: Run all VM tests**

Run: `cd compiler && cargo test --package ironplc-vm`
Expected: All tests pass.

**Step 3: Regenerate the golden test file**

Run: `cd compiler && cargo test -p ironplc-vm --test cli generate_golden -- --ignored --nocapture`

Note: The golden file doesn't actually change since the container format hasn't changed — only the VM execution model changed. But regenerate to confirm.

**Step 4: Run full CI pipeline**

Run: `cd compiler && just`
Expected: compile, coverage (85%+), and lint all pass.

If format issues: `cd compiler && just format`
If clippy issues: fix and re-run.

**Step 5: Run VS Code CI**

Run: `cd integrations/vscode && just ci`
Expected: All checks pass (VS Code extension is unaffected by VM changes).

**Step 6: Commit**

```bash
git add compiler/vm/src/vm.rs compiler/vm/src/cli.rs compiler/vm/src/lib.rs compiler/vm/resources/test/steel_thread.iplc
git commit -m "Add fault context to traps and run full CI"
```
