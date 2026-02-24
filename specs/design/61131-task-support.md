# Design: IEC 61131-3 Task, Configuration, and Resource Support

## Overview

The IronPLC VM currently executes a single program with a single scan cycle. This is insufficient for real IEC 61131-3 applications, which organize execution around **configurations**, **resources**, and **tasks** — allowing multiple programs to run at different priorities and cycle times within a single PLC.

This document describes the changes needed to the container format, VM execution model, codegen, and CLI to support the IEC 61131-3 task model.

### Building On

- **[Bytecode Container Format](../plans/bytecode-container-format.md)** — the current single-entry-point container
- **[Runtime Execution Model](../plans/runtime-execution-model.md)** — the current single-scan-cycle model
- **[Bytecode Instruction Set](../plans/bytecode-instruction-set.md)** — the instruction set (unchanged by this design)

## IEC 61131-3 Task Model

Section 2.7 of IEC 61131-3 defines a hierarchical execution model:

```
CONFIGURATION
├── Global Variables (VAR_GLOBAL)
├── RESOURCE (maps to a processing element / CPU)
│   ├── Resource-scoped Global Variables
│   ├── TASK declarations
│   │   ├── INTERVAL (cyclic period, e.g., T#10ms)
│   │   ├── PRIORITY (0 = highest, higher numbers = lower priority)
│   │   └── SINGLE (boolean variable for event-triggered execution)
│   └── PROGRAM instances (associated WITH a task)
│       ├── Instance name
│       ├── Program type
│       ├── Task association (WITH task_name)
│       └── I/O connections (sources and sinks)
└── Access Paths (for communication services)
```

### Task Types

The standard defines two task triggering mechanisms:

**Cyclic tasks** have an `INTERVAL` parameter specifying the execution period. The runtime executes the task's associated programs at this fixed interval. Example:

```
TASK fast_task(INTERVAL := T#10ms, PRIORITY := 0);
TASK slow_task(INTERVAL := T#100ms, PRIORITY := 5);
```

**Event-triggered tasks** have a `SINGLE` parameter referencing a boolean variable. The task executes once on the rising edge of that variable. Example:

```
TASK alarm_task(SINGLE := alarm_trigger, PRIORITY := 0);
```

**Unassociated programs** — programs not associated with any task — are mentioned in some implementations as "freewheeling" tasks that execute every scan cycle as fast as possible, at the lowest priority.

### Program Instances

Programs are instantiated within a resource and associated with a task:

```
RESOURCE resource1 ON PLC
  TASK cyclic(INTERVAL := T#10ms, PRIORITY := 1);
  TASK slow(INTERVAL := T#100ms, PRIORITY := 5);

  PROGRAM motor_control WITH cyclic : MotorController;
  PROGRAM data_logging WITH slow : DataLogger;
  PROGRAM status_display WITH slow : StatusDisplay;
END_RESOURCE
```

Key properties:
- Each `PROGRAM` line creates a **distinct instance** with its own variable state
- Multiple instances of the same program type can exist (e.g., two `MotorController` instances)
- A single task can have multiple programs — they execute sequentially in declaration order within the task
- Program instances persist across scan cycles (they are stateful, like function block instances)

### Variable Scoping

IEC 61131-3 defines three scopes for global variables:

| Scope | Declaration | Visibility |
|-------|-------------|------------|
| Configuration | `VAR_GLOBAL` in CONFIGURATION | All resources and programs |
| Resource | `VAR_GLOBAL` in RESOURCE | All programs in that resource |
| Program | `VAR` in PROGRAM | Only that program instance |

Programs can also connect to I/O through **located variables** (`AT %IX0.0`) and through explicit source/sink mappings in the program configuration.

### Priority and Scheduling

IEC 61131-3 specifies that:
- Priority 0 is the highest priority
- Higher numeric values mean lower priority
- The standard does **not** mandate preemptive vs. cooperative scheduling — this is implementation-defined
- When multiple tasks are ready to execute simultaneously, the highest-priority task runs first

The standard is deliberately silent on:
- What happens when a task overruns its cycle time
- Whether tasks can preempt each other mid-execution
- Maximum number of priority levels

## Industry Practice

### CODESYS

CODESYS is the most widely-used IEC 61131-3 development system and provides a useful reference implementation.

**Task types:**
- **Cyclic** — executes at a fixed interval
- **Event** — executes on rising edge of a boolean variable
- **Freewheeling** — executes as fast as possible, lowest priority, fills idle time
- **External event** — triggered by hardware interrupt (platform-specific)

**Scheduling:**
- Cooperative scheduling by default on single-core targets
- Preemptive scheduling available on multi-core targets (each task can be pinned to a core)
- Tasks within the same priority level are time-sliced

**Cycle overrun handling:**
- If a task overruns its cycle time, the next cycle starts immediately when the current one completes
- A **watchdog timer** per task detects excessive overruns — configurable response (log warning, stop task, stop PLC)
- Overrun counter is exposed in diagnostics

**I/O update model:**
- Each task has its own I/O update — inputs are read at the start of the task cycle, outputs are written at the end
- The "task image" concept: each task sees a consistent snapshot of I/O for its entire execution
- Bus cycle time is independent of task cycle time (I/O bus runs at its own rate)

### Beckhoff TwinCAT

**Task types:**
- Real-time tasks with cycle times from 50 microseconds
- Tasks bound to isolated CPU cores for deterministic timing
- Supports up to 10+ priority levels

**Scheduling:**
- Fully preemptive, priority-based scheduling
- Tasks run on dedicated real-time cores
- Higher-priority tasks preempt lower-priority tasks

**Notable features:**
- Per-task watchdog with configurable timeout
- Cycle time jitter monitoring and statistics
- Tasks can be enabled/disabled at runtime

### Siemens S7 / TIA Portal

Siemens uses **Organization Blocks (OBs)** rather than the IEC 61131-3 task syntax:

| OB | Purpose | IEC 61131-3 Equivalent |
|----|---------|----------------------|
| OB1 | Main scan cycle | Default cyclic task |
| OB10-OB17 | Time-of-day interrupts | Event tasks |
| OB20-OB23 | Time-delay interrupts | One-shot event tasks |
| OB30-OB38 | Cyclic interrupts | Cyclic tasks with different intervals |
| OB40-OB47 | Hardware interrupts | External event tasks |
| OB80-OB87 | Error OBs | Fault handlers |
| OB100 | Startup | Initialization |

**Scheduling:**
- Fully preemptive — higher-priority OBs interrupt lower-priority ones
- OB1 runs continuously at lowest priority
- Cycle monitoring: if OB1 exceeds max scan time, the PLC goes to STOP

**Notable features:**
- Separate "last-good-value" mode vs. "substitute value" mode per output
- Startup OB for one-time initialization before cyclic execution begins

### OpenPLC

**Approach:**
- Single cyclic task model — one main program, one cycle time
- No multi-task support in the open-source runtime
- Simpler model suitable for soft-real-time applications

### Summary of Industry Features

| Feature | CODESYS | TwinCAT | Siemens S7 | OpenPLC |
|---------|---------|---------|-----------|---------|
| Cyclic tasks | Yes | Yes | Yes (OB30+) | 1 only |
| Event tasks | Yes | Yes | Yes (OB40+) | No |
| Freewheeling tasks | Yes | No | Yes (OB1) | Yes (default) |
| Per-task watchdog | Yes | Yes | Yes | Global only |
| Preemptive scheduling | Optional | Yes | Yes | N/A |
| Per-task I/O image | Yes | Yes | Yes | N/A |
| Startup hook | No | No | Yes (OB100) | No |
| Fault handler tasks | No | No | Yes (OB80+) | No |
| Task enable/disable | Yes | Yes | No | No |
| Cycle overrun diagnostics | Yes | Yes | Yes | No |

## Current Architecture Gaps

### What already exists

The IronPLC **parser and AST** already handle the full IEC 61131-3 configuration syntax:

- `ConfigurationDeclaration` with global variables and resource declarations (`compiler/dsl/src/configuration.rs`)
- `ResourceDeclaration` with tasks and program configurations
- `TaskConfiguration` with `name`, `priority` (u32), and `interval` (optional `DurationLiteral`)
- `ProgramConfiguration` with `task_name`, sources, and sinks
- Semantic validation that task names referenced by programs actually exist (`compiler/analyzer/src/rule_program_task_definition_exists.rs`)

### What is missing

**Container format:**
- Only a single `entry_function_id` — no concept of multiple programs or tasks
- No task metadata (interval, priority) in the container
- No program instance table — cannot represent multiple instances of the same program type
- No support for configuration-scoped or resource-scoped global variables

**VM execution model:**
- Single scan cycle with a single entry point
- No task scheduler — no concept of priority or intervals
- No program instance isolation — the variable table is global and flat
- `scan_mode` and `scan_interval` are VM-level configuration, not per-task
- No per-task watchdog
- No per-task I/O images

**Codegen:**
- Finds and compiles only the first PROGRAM declaration
- Ignores CONFIGURATION, RESOURCE, and TASK declarations entirely
- No support for compiling multiple programs into a single container

**CLI:**
- Runs exactly one scan cycle and exits — no continuous execution loop

## Design

### Design Principles

1. **Cooperative scheduling first** — implement non-preemptive, single-threaded task scheduling. This avoids thread synchronization complexity and matches IronPLC's deterministic execution model. Preemptive scheduling can be layered on later.

2. **One container per resource** — each container file represents one resource (one processing element). Multi-resource configurations produce multiple container files. This keeps the container self-contained and the VM stateless with respect to other VMs.

3. **Isolated program instances** — each program instance gets its own variable table partition. Configuration and resource globals are shared through a separate shared region.

4. **Backward compatible** — containers without task metadata (format version 1) continue to work as a single-program, single-task execution.

### Container Format Changes

#### New Section: Task Table

Add a **task table section** to the container, between the type section and the constant pool. This section defines the task schedule for the resource.

```
┌─────────────────────────────────────────┐  offset 0
│ File Header (256+ bytes, fixed size)    │
├─────────────────────────────────────────┤
│ Content Signature Section               │
├─────────────────────────────────────────┤
│ Debug Signature Section (optional)      │
├─────────────────────────────────────────┤
│ Type Section                            │
├─────────────────────────────────────────┤
│ Task Table Section (NEW)                │
├─────────────────────────────────────────┤
│ Constant Pool Section                   │
├─────────────────────────────────────────┤
│ Code Section                            │
├─────────────────────────────────────────┤
│ Debug Section (optional)                │
└─────────────────────────────────────────┘
```

#### Task Table Format

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | num_tasks | u16 | Number of task entries |
| 2 | num_program_instances | u16 | Total number of program instances across all tasks |
| 4 | shared_globals_size | u16 | Number of variable slots for configuration + resource globals |
| 6 | tasks | [TaskEntry; num_tasks] | Task descriptors |
| varies | programs | [ProgramInstanceEntry; num_program_instances] | Program instance descriptors |

Each **TaskEntry** (16 bytes):

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | task_id | u16 | Unique task identifier |
| 2 | priority | u16 | Task priority (0 = highest) |
| 4 | task_type | u8 | 0 = cyclic, 1 = event, 2 = freewheeling |
| 5 | flags | u8 | Bit 0: enabled at start. Reserved bits must be zero. |
| 6 | interval_us | u64 | Cycle interval in microseconds (0 for event/freewheeling tasks) |
| 14 | single_var_index | u16 | Variable index of SINGLE trigger variable (0xFFFF if not event task) |

Each **ProgramInstanceEntry** (16 bytes):

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | instance_id | u16 | Unique instance identifier |
| 2 | task_id | u16 | Task this instance belongs to (matches TaskEntry.task_id) |
| 4 | entry_function_id | u16 | Function ID of this program's entry point |
| 6 | var_table_offset | u16 | Starting index in the variable table for this instance's variables |
| 8 | var_table_count | u16 | Number of variable slots for this instance |
| 10 | fb_instance_offset | u16 | Starting index in the FB instance table for this instance |
| 12 | fb_instance_count | u16 | Number of FB instance slots for this instance |
| 14 | reserved | u16 | Reserved; must be zero |

#### Header Changes

Add to the file header (using reserved bytes at offset 226):

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 226 | task_section_offset | u32 | Offset of task table section (0 if absent) |
| 230 | task_section_size | u32 | Size of task table section |
| 234 | num_tasks | u16 | Number of tasks (duplicated from task table for fast reject) |
| 236 | num_program_instances | u16 | Total program instances |
| 238 | shared_globals_count | u16 | Variable slots for shared globals |
| 240 | reserved | [u8; 16] | Remaining reserved bytes |

Add a new flag bit:
- `flags` bit 3: has task table section

#### Backward Compatibility

When `flags` bit 3 is clear (no task table), the VM falls back to the current behavior: `entry_function_id` from the header is the sole program, run on a single implicit cyclic task using the VM-level `scan_interval`. This means existing format version 1 containers work without modification — they just run as a single-task, single-program configuration.

When `flags` bit 3 is set, the VM ignores `entry_function_id` in the header and uses the task table instead.

### Variable Table Partitioning

The variable table is partitioned into regions:

```
Variable Table Layout:
┌─────────────────────────────────────────────┐  index 0
│ Shared Globals (config + resource scope)    │
│   indices 0 .. shared_globals_count - 1     │
├─────────────────────────────────────────────┤
│ Program Instance 0 variables                │
│   indices var_table_offset .. +count        │
├─────────────────────────────────────────────┤
│ Program Instance 1 variables                │
│   indices var_table_offset .. +count        │
├─────────────────────────────────────────────┤
│ ...                                         │
├─────────────────────────────────────────────┤
│ Program Instance N variables                │
└─────────────────────────────────────────────┘
```

Shared globals are accessible from all program instances via their absolute variable index. The compiler emits `LOAD_VAR`/`STORE_VAR` instructions using the absolute index for shared globals and instance-relative indices for program-local variables.

The compiler resolves all variable references to absolute indices at compile time. No runtime indirection is needed — the variable table is a flat array, and the partitioning is purely a compiler concern. This maintains the current zero-overhead variable access model.

### VM Task Scheduler

#### Scheduler State

The VM maintains a `TaskScheduler` that tracks the state of each task:

```
TaskScheduler
├── tasks: Vec<TaskState>
├── program_instances: Vec<ProgramInstanceState>
├── ready_queue: BinaryHeap<ReadyTask>  (sorted by priority)
└── current_time: i64 (microseconds)

TaskState
├── task_id: u16
├── priority: u16
├── task_type: TaskType (Cyclic | Event | Freewheeling)
├── interval_us: u64
├── next_due: i64 (microseconds, monotonic)
├── single_var_index: Option<u16>
├── single_prev_value: bool (for edge detection)
├── enabled: bool
├── overrun_count: u64
├── last_execute_duration: u64
├── max_execute_duration: u64
└── scan_count: u64

ProgramInstanceState
├── instance_id: u16
├── task_id: u16
├── entry_function_id: u16
├── var_table_offset: u16
└── var_table_count: u16
```

#### Scheduling Algorithm

The scheduler runs a single-threaded cooperative loop:

```
RUNNING loop:
  1. current_time = read_monotonic_clock()

  2. For each cyclic task:
       if current_time >= task.next_due:
         mark task as ready
         task.next_due += task.interval_us
         if task.next_due <= current_time:
           task.overrun_count += 1  (skipped cycle detection)
           task.next_due = current_time + task.interval_us

  3. For each event task:
       current_value = read_variable(task.single_var_index)
       if current_value == true AND task.single_prev_value == false:
         mark task as ready  (rising edge detected)
       task.single_prev_value = current_value

  4. For each freewheeling task:
       mark task as ready  (always ready)

  5. Sort ready tasks by priority (lowest number = highest priority)
     Break ties by task_id (lower task_id first — declaration order)

  6. For each ready task (in priority order):
       execute_task(task)

  7. If no tasks were ready:
       sleep until next cyclic task is due (or yield to OS)
```

#### Task Execution

Each task execution follows the existing scan cycle pattern:

```
execute_task(task):
  1. INPUT_FREEZE — snapshot inputs into the process image
  2. For each program instance associated with this task (in declaration order):
       a. EXECUTE — call instance's entry_function_id
       b. The entry function reads/writes the shared variable table
          (shared globals + instance-local variables)
  3. OUTPUT_FLUSH — hand staging buffer to I/O driver
  4. Record timing: task.last_execute_duration, update task.max_execute_duration
  5. Increment task.scan_count
```

This means all programs within a single task share the same I/O snapshot — inputs are frozen once at the start of the task cycle, and outputs are flushed once at the end. This matches CODESYS behavior and ensures consistency within a task.

#### I/O Update Granularity

For the initial implementation, all tasks share a single process image:

- **INPUT_FREEZE** occurs once before the highest-priority ready task executes
- **OUTPUT_FLUSH** occurs once after the lowest-priority ready task completes
- All tasks within a scheduling round see the same input snapshot

This is simpler than per-task I/O images and sufficient for single-threaded cooperative scheduling. Per-task I/O images become important when moving to preemptive scheduling (where a high-priority task could preempt a low-priority task mid-execution, requiring isolated I/O views).

**Future extension:** When preemptive scheduling is added, each task should get its own input snapshot and output staging buffer. The header would need per-task `input_image_bytes` / `output_image_bytes` fields, or these could be stored in the task table entries.

#### Watchdog

Each task has its own watchdog timeout. For now, the VM-level `max_scan_time` applies to each task execution individually. If any single task's EXECUTE phase exceeds `max_scan_time`, the VM traps.

**Future extension:** Per-task watchdog timeouts could be stored in the task table (adding a `watchdog_us` field to `TaskEntry`).

#### Trap Handling

When a trap occurs during task execution:

1. The current task's execution is aborted
2. OUTPUT_FLUSH is skipped for the current scheduling round
3. The VM transitions to FAULTED state (same as current behavior)
4. The trap diagnostic includes the `task_id` and `instance_id` that caused the trap

This is the simplest safe behavior. More sophisticated options (per-task fault isolation, where only the faulting task stops while others continue) can be added later.

### Concepts Missing From Current Design

Beyond tasks themselves, the following IEC 61131-3 and industry concepts are not addressed in the current VM/container design and should be considered:

#### 1. Startup / Initialization Task

Siemens S7 has OB100 (startup OB) that runs once before cyclic execution begins. CODESYS has an initialization phase. IEC 61131-3 allows initial values on variables, but doesn't define a startup task.

**Recommendation:** Add an optional `startup_function_id` field to the container header (or task table). When present, the VM calls this function once during the READY → RUNNING transition, after initialization but before the first scan cycle. This is useful for one-time setup that depends on I/O state (e.g., reading a configuration DIP switch).

#### 2. Fault Handler Tasks

Siemens S7 has OB80-OB87 for handling various fault conditions. When a fault occurs, the fault handler OB runs instead of transitioning to STOP.

**Recommendation:** Defer to a future design. The current FAULTED state with last-good-value output hold is the safe default. Fault handler tasks add complexity (what if the fault handler itself faults?) and are rarely used outside Siemens ecosystems.

#### 3. RETAIN / PERSISTENT Variables

Variables that survive power cycles. Relevant to tasks because RETAIN variables must be saved at defined points (typically at the end of each scan cycle or on controlled shutdown).

**Recommendation:** Out of scope for this design (already listed as out of scope in the runtime execution model). Note that the task model doesn't change the RETAIN design — it just means RETAIN variables could belong to any program instance or to shared globals.

#### 4. Communication Between Program Instances

When multiple program instances run within the same resource, they may need to share data. IEC 61131-3 provides:
- Configuration and resource global variables (shared memory)
- Access paths (for external communication services)

**Recommendation:** Shared globals (already part of this design's variable table partitioning) handle intra-resource communication. Access paths are a separate concern for external communication.

#### 5. Program Connection Sources and Sinks

IEC 61131-3 allows explicit I/O mapping in program configurations:

```
PROGRAM motor WITH fast_task : MotorController
  (setpoint := %IW0, output => %QW0);
```

This maps specific I/O addresses to program instance variables.

**Recommendation:** The compiler should resolve these mappings to `LOAD_INPUT` / `STORE_OUTPUT` instructions within the program's entry function. No VM changes needed — the compiler inserts I/O copy instructions at the start (for sources) and end (for sinks) of each program instance's code. This is the approach CODESYS uses internally.

#### 6. Task Enable/Disable at Runtime

CODESYS and TwinCAT allow tasks to be enabled or disabled at runtime through diagnostic interfaces or from program logic.

**Recommendation:** Include the `enabled` flag in `TaskState` (already in the design above). Expose through the diagnostic interface. Defer programmatic enable/disable (from user code) to a future design.

### Codegen Changes

The compiler's code generation must change from "find first PROGRAM and compile it" to:

1. **Find the CONFIGURATION declaration** (or synthesize one if the source only contains standalone programs — for backward compatibility)

2. **For each RESOURCE in the configuration:**
   a. Assign variable indices for configuration globals (shared across all resources)
   b. Assign variable indices for resource globals (shared within resource)
   c. For each PROGRAM instance:
      - Assign variable indices for the program's local variables
      - Compile the program body into a function
   d. Emit the task table section with task metadata
   e. Build the container for this resource

3. **Synthesize a default configuration** when the source contains bare PROGRAM declarations without a CONFIGURATION wrapper:
   ```
   (* Implicit configuration for backward compatibility *)
   CONFIGURATION default_config
     RESOURCE default_resource ON default_cpu
       TASK default_task(INTERVAL := T#10ms, PRIORITY := 0);
       PROGRAM instance0 WITH default_task : <first_program>;
     END_RESOURCE
   END_CONFIGURATION
   ```

4. **Variable index assignment** follows the deterministic ordering rules from the container format spec, extended to handle the partitioned layout:
   - Shared globals first (sorted by qualified name)
   - Then per-instance variables (instances in declaration order, variables within each instance sorted by name)

### CLI Changes

The `ironplcvm run` command needs to support continuous execution:

```
ironplcvm run [OPTIONS] <FILE>

Options:
  --dump-vars <PATH>     Write variable dump after execution
  --scans <N>            Run N scheduling rounds (default: 1)
  --continuous           Run until interrupted (Ctrl+C)
```

In single-scan mode (`--scans 1`, the default), the scheduler executes one round: all ready tasks run once. This preserves backward compatibility.

In continuous mode, the scheduler runs its loop indefinitely, respecting task intervals and priorities, until the process receives SIGINT/SIGTERM.

### Diagnostic Interface Extensions

The diagnostic interface should be extended to expose per-task information:

| Category | Fields | Update frequency |
|----------|--------|-----------------|
| Task list | task_id, priority, type, interval, enabled | On request |
| Task status | scan_count, last_execute_duration, max_execute_duration, overrun_count | Every task execution |
| Program instances | instance_id, task_id, entry_function_id | On request |
| Ready queue | Currently ready tasks and their order | On request |

## Phased Implementation

### Phase 1: Container Format (task table section)

- Add the task table section to the container format
- Add header fields for task table offset/size
- Update the container builder to accept task entries
- Update the container reader to parse task entries
- Backward-compatible: containers without task table work as before

### Phase 2: VM Task Scheduler (cyclic tasks only)

- Implement `TaskScheduler` with cyclic task support
- Implement the cooperative scheduling loop
- Implement per-task execution (INPUT_FREEZE, EXECUTE per program instance, OUTPUT_FLUSH)
- Variable table partitioning (shared globals + per-instance regions)
- Per-task trap diagnostics
- Update CLI with `--scans` and `--continuous` options

### Phase 3: Event and Freewheeling Tasks

- Add event task support (SINGLE variable edge detection)
- Add freewheeling task support
- Per-task watchdog timeouts (task table extension)

### Phase 4: Codegen Integration

- Compile CONFIGURATION/RESOURCE/TASK declarations
- Emit task table section
- Variable table partitioning in codegen
- Synthesize default configuration for bare PROGRAM declarations
- Program connection source/sink compilation (I/O mappings)

### Phase 5: Startup Function

- Add optional `startup_function_id` support
- Execute once during READY → RUNNING transition

## Open Questions

1. **Per-task I/O images vs. shared I/O image.** The initial design uses a shared process image for simplicity. Should we plan the container format to support per-task images from the start (adding fields now even if unused), or defer entirely?

2. **Task overrun policy.** When a cyclic task can't keep up with its interval, should the VM: (a) skip the missed cycle and schedule at the next interval boundary, (b) execute immediately and let cycles bunch up, or (c) be configurable? The current design uses option (a) with an overrun counter. CODESYS uses option (b) by default.

3. **Maximum number of tasks and priority levels.** The current design uses u16 for task_id and priority, allowing 65536 tasks and priority levels. In practice, PLCs have 4–32 tasks. Should we restrict this in the verifier?

4. **Variable table size limits.** With partitioned variable tables, the total variable count could exceed u16 range for large configurations. Is this a realistic concern, or is u16 sufficient?

5. **Multi-resource support.** This design compiles each resource to a separate container. Should the CLI support loading multiple containers for multi-resource configurations, or is that a host application concern?

6. **Per-task fault isolation.** Should a fault in one task stop all tasks (current design), or should only the faulting task stop while others continue? The latter is more resilient but more complex (what about shared state consistency?).
