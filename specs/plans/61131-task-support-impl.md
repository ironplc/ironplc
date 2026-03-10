# Implementation Plan: IEC 61131-3 Task Support

**Design:** [IEC 61131-3 Task Support](../design/61131-task-support.md)

## Phased Implementation

### Phase 0: Parser and AST Completeness

- Add `SINGLE` parameter to `TaskConfiguration` AST (new `DataSourceKind` type)
- Parse `SINGLE` in `task_initialization` rule (support both constants and variable references)
- Fix `INTERVAL` parser panic — return proper error for non-duration types
- Fix renderer typo (`INTERNAL` → `INTERVAL`) and add `SINGLE` rendering
- Add semantic rule: task names unique within a resource (P4019)

### Phase 1: Container Format (task table section)

- Add the task table section to the container format
- Replace `entry_function_id` with task table — task table is always present
- Add header fields for task table offset/size
- Update the container builder to accept task entries
- Update the container reader to parse task entries

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
- Enable per-task watchdog enforcement (field already in task table from Phase 1)

### Phase 4: Codegen Integration

- Compile CONFIGURATION/RESOURCE/TASK declarations
- Emit task table section
- Variable table partitioning in codegen
- Synthesize default configuration for bare PROGRAM declarations
- Program connection source/sink compilation (I/O mappings)

### Phase 5: Startup Function

- Add optional `startup_function_id` support
- Execute once during READY → RUNNING transition
