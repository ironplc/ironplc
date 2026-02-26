# Plan: VM Testing Implementation

This plan describes the step-by-step implementation of the testing strategy defined in [vm-testing design](../design/vm-testing.md). Each phase is independently shippable — it adds tests, passes CI, and can be merged without depending on later phases.

## Prerequisites

- Familiarity with the [vm-testing design](../design/vm-testing.md), which describes the testing layers and what is missing
- The VM crate structure: `compiler/vm/src/` for production code, `compiler/vm/tests/` for integration tests
- The `ContainerBuilder` API in `compiler/container/src/builder.rs`
- The opcode definitions in `compiler/container/src/opcode.rs`
- The codegen end-to-end tests in `compiler/codegen/tests/end_to_end.rs`

## Phase 1: Execute Error Path Tests

**Goal**: Every `Trap` variant that can fire inside `execute()` has a test that triggers it through the full `Vm::new().load(c).start().run_round()` path.

**Why first**: These are the highest-value missing tests. The error paths exist in production code but are only tested at the component level (stack, variable table), not through the interpreter loop. A regression in `execute()` error handling would go undetected.

### Step 1.1: Add `execute` error path tests to `vm.rs`

Add the following tests to the `#[cfg(test)]` module in `compiler/vm/src/vm.rs`:

**`execute_when_stack_overflow_then_trap`**
- Build a container with `max_stack_depth: 1` and bytecode that pushes two values
- Bytecode: `LOAD_CONST_I32 pool[0], LOAD_CONST_I32 pool[1]` (pushes 2 items onto a size-1 stack)
- Assert: `run_round()` returns `Err` with `trap == Trap::StackOverflow`

**`execute_when_stack_underflow_then_trap`**
- Build a container with bytecode that pops from an empty stack
- Bytecode: `ADD_I32` (tries to pop 2 from empty stack)
- Assert: `run_round()` returns `Err` with `trap == Trap::StackUnderflow`

**`execute_when_invalid_constant_index_then_trap`**
- Build a container with 0 constants but bytecode that references pool index 0
- Bytecode: `LOAD_CONST_I32 pool[0]`
- Assert: `run_round()` returns `Err` with `trap == Trap::InvalidConstantIndex(0)`

**`execute_when_invalid_variable_index_on_store_then_trap`**
- Build a container with 1 variable but bytecode that stores to variable index 5
- Bytecode: `LOAD_CONST_I32 pool[0], STORE_VAR_I32 var[5]`
- Assert: `run_round()` returns `Err` with `trap == Trap::InvalidVariableIndex(5)`

**`execute_when_invalid_variable_index_on_load_then_trap`**
- Build a container with 1 variable but bytecode that loads from variable index 5
- Bytecode: `LOAD_VAR_I32 var[5]`
- Assert: `run_round()` returns `Err` with `trap == Trap::InvalidVariableIndex(5)`

### Step 1.2: Add `execute` edge-case tests to `vm.rs`

**`execute_when_empty_bytecode_then_ok`**
- Build a container with empty bytecode (0-length function)
- Assert: `run_round()` returns `Ok(())`
- Rationale: Documents the current behavior where `pc >= bytecode.len()` exits cleanly. If this behavior should change (e.g., require RET_VOID), update this test.

**`execute_when_add_i32_wraps_at_max_then_correct`**
- Constants: `i32::MAX`, `1`
- Bytecode: `LOAD_CONST pool[0], LOAD_CONST pool[1], ADD_I32, STORE_VAR var[0], RET_VOID`
- Assert: `var[0] == i32::MIN` (wrapping addition per ADR-0002)

**`execute_when_add_i32_wraps_at_min_then_correct`**
- Constants: `i32::MIN`, `-1`
- Bytecode: `LOAD_CONST pool[0], LOAD_CONST pool[1], ADD_I32, STORE_VAR var[0], RET_VOID`
- Assert: `var[0] == i32::MAX`

### Files changed

- `compiler/vm/src/vm.rs` — add 7 tests to the `#[cfg(test)]` module

### Verification

```bash
cd compiler && cargo test -p ironplc-vm
```

All new tests pass. No changes to production code.

## Phase 2: Multi-Scan Scenario Tests

**Goal**: Verify that programs accumulate state correctly across scan cycles and that the VM lifecycle transitions work with repeated execution.

### Step 2.1: Create `compiler/vm/tests/scenarios.rs`

This file contains scenario tests that exercise multi-scan execution patterns.

**`scenario_when_counter_increments_each_scan_then_accumulates`**

A program that reads a variable, adds 1, and writes it back. After N scans, the variable should equal N.

```
Program logic: x := x + 1
Bytecode:
  LOAD_VAR_I32 var[0]     // push current x
  LOAD_CONST_I32 pool[0]  // push 1
  ADD_I32                  // x + 1
  STORE_VAR_I32 var[0]     // write back
  RET_VOID
```

- Run 10 scans
- Assert: `var[0] == 10`

This is the canonical "stateful PLC program" test. It proves that variable state persists across scan cycles (variables are not reset between scans).

**`scenario_when_stop_then_scan_count_reflects_completed_rounds`**

- Run 5 scans, then stop
- Assert: `stopped.scan_count() == 5`
- Assert: variables reflect 5 scans of execution

**`scenario_when_fault_on_scan_3_then_first_2_scans_visible`**

Build a container where the program writes a counter (scan 1: x=1, scan 2: x=2) but on scan 3, a fault occurs (e.g., due to a second function with bad bytecode triggered by a different task arrangement). Verify the faulted VM shows x=2 (the state after the last successful scan).

Note: This test is more complex to set up because it requires a fault to occur at a specific scan. A simpler approach: use a program that always faults (invalid opcode as the first instruction). Run one round, verify it faults, and check that pre-fault variable state is accessible.

**`scenario_when_variables_read_after_fault_then_accessible`**

- Build a container where the program stores a value then hits an invalid instruction
- Bytecode: `LOAD_CONST pool[0] (42), STORE_VAR var[0], 0xFF` (invalid opcode)
- Run one round
- Assert: fault occurs
- Assert: `faulted.read_variable(0) == 42` (the store before the fault is visible)

### Files changed

- `compiler/vm/tests/scenarios.rs` — new file, 4 tests

### Verification

```bash
cd compiler && cargo test -p ironplc-vm --test scenarios
```

## Phase 3: Multi-Task and Variable Scope Tests

**Goal**: Verify that the scheduler drives multiple tasks with separate variable scopes, and that variable isolation is enforced at the VM level.

### Step 3.1: Add multi-task tests to `compiler/vm/tests/scenarios.rs`

**`scenario_when_two_freewheeling_tasks_then_both_execute`**

Build a container with:
- 4 variables total
- Shared globals: 0
- Task 0 (priority 0): program instance 0, function 0, variables [0, 2)
- Task 1 (priority 1): program instance 1, function 1, variables [2, 4)
- Function 0 bytecode: `LOAD_CONST pool[0] (10), STORE_VAR var[0], RET_VOID`
- Function 1 bytecode: `LOAD_CONST pool[1] (20), STORE_VAR var[2], RET_VOID`

After one round:
- Assert: `var[0] == 10` (set by task 0)
- Assert: `var[2] == 20` (set by task 1)

This uses `ContainerBuilder::add_task()` and `ContainerBuilder::add_program_instance()` to explicitly construct the multi-task layout.

**`scenario_when_tasks_share_global_then_communication_works`**

Build a container with:
- 4 variables total
- Shared globals: 1 (variable 0 is global)
- Task 0 (priority 0): writes 99 to var[0] (global)
- Task 1 (priority 1): reads var[0] (global), stores to var[2] (its private variable)

After one round (task 0 runs first due to higher priority):
- Assert: `var[0] == 99` (global, written by task 0)
- Assert: `var[2] == 99` (task 1 read the global)

This verifies that `shared_globals_size` in `VariableScope` enables cross-task communication.

**`scenario_when_scope_violation_then_trap`**

Build a container where a program instance tries to access a variable outside its scope:
- 4 variables total
- Program instance 0: scope is variables [2, 4), shared globals = 0
- Bytecode for function 0: `LOAD_VAR_I32 var[0]` (index 0 is outside the scope)

After one round:
- Assert: trap occurs with `InvalidVariableIndex(0)`

This verifies that `VariableScope::check_access()` is actually called during `execute()` and that scope violations are caught at runtime.

### Step 3.2: Add watchdog test

**`scenario_when_watchdog_exceeded_then_trap`**

This test is conceptually simple but practically difficult because the watchdog checks elapsed real time. The approach:

Build a container with:
- A task with `watchdog_us: 1` (1 microsecond — impossibly short)
- A program that does some work (e.g., several load/add/store operations)

After one round:
- Assert: trap occurs with `WatchdogTimeout(task_id)`

The 1μs watchdog virtually guarantees the program takes longer, making the test deterministic without platform-specific timing assumptions.

Note: If the test proves flaky (bytecode execution can be very fast), an alternative is to use a loop of many instructions. But 1μs is likely sufficient since even a single instruction involves function call overhead, memory access, and VM dispatch.

### Files changed

- `compiler/vm/tests/scenarios.rs` — add 4 tests (extending the file from Phase 2)

### Verification

```bash
cd compiler && cargo test -p ironplc-vm --test scenarios
```

## Phase 4: End-to-End Pipeline Expansion

**Goal**: Expand `codegen/tests/end_to_end.rs` with tests that exercise codegen → VM for patterns that will matter as the instruction set grows.

### Step 4.1: Add stateful program end-to-end test

**`end_to_end_when_counter_program_then_increments_across_scans`**

```iec
PROGRAM main
  VAR
    count : INT;
  END_VAR
  count := count + 1;
END_PROGRAM
```

- Run 5 scans
- Assert: `var[0] == 5`

This depends on the codegen correctly emitting `LOAD_VAR + LOAD_CONST + ADD + STORE` for `count := count + 1`, and the VM correctly persisting variable state across scans. It bridges the gap between "codegen produces correct bytecode" and "the VM executes it correctly over time."

### Step 4.2: Add large-expression end-to-end test

**`end_to_end_when_deeply_nested_expression_then_correct_result`**

```iec
PROGRAM main
  VAR
    result : INT;
  END_VAR
  result := 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10;
END_PROGRAM
```

- Run 1 scan
- Assert: `var[0] == 55`

This exercises the stack depth tracking in the emitter and verifies that the codegen-computed `max_stack_depth` is sufficient for the actual execution.

### Files changed

- `compiler/codegen/tests/end_to_end.rs` — add 2 tests

### Verification

```bash
cd compiler && cargo test -p ironplc-codegen --test end_to_end
```

## Phase 5: Test Infrastructure Helpers

**Goal**: Reduce boilerplate across test files by extracting common patterns into shared helpers.

### Step 5.1: Evaluate whether helpers are needed

After Phases 1-4, review the test code for repeated patterns. Common candidates:

- Building a single-function container from bytecode, constants, and variable count
- Running N scans and returning the VM
- Asserting a specific trap from `run_round()`

If 3+ tests share the same setup pattern, extract it. Otherwise, leave the test code explicit — clarity is more important than DRY in tests.

### Step 5.2: Extract helpers if warranted

If extraction is warranted, place helpers in:
- `compiler/vm/src/vm.rs` `#[cfg(test)]` module for helpers used only by unit tests in that file (like the existing `steel_thread_container()`)
- `compiler/vm/tests/` as a `mod helpers;` if shared across multiple integration test files

Do **not** add test helpers to production code. The existing `VariableScope::permissive()` pattern (annotated with `#[cfg(test)]`) is acceptable for simple constructors.

### Files changed

- Possibly `compiler/vm/tests/helpers.rs` — new file (only if Phase 1-4 review shows enough duplication)
- Possibly `compiler/vm/tests/scenarios.rs` and `compiler/vm/src/vm.rs` — refactor to use helpers

### Verification

```bash
cd compiler && just
```

Full CI pipeline must pass after any refactoring.

## Summary: Test Count Projection

| Phase | New tests | Cumulative total (VM crate) |
|---|---|---|
| Current state | 0 | 34 (28 unit + 6 integration) |
| Phase 1 | 7 | 41 |
| Phase 2 | 4 | 45 |
| Phase 3 | 4 | 49 |
| Phase 4 | 2 (in codegen) | 49 + 2 codegen |
| Phase 5 | 0 (refactor) | 49 + 2 codegen |

## Ongoing: Test-per-Opcode Discipline

As the instruction set grows (per the [bytecode instruction set plan](bytecode-instruction-set.md)), each new opcode should be accompanied by:

1. **One emit test** in `codegen/src/emit.rs` — verifying the emitter produces correct bytes
2. **One or more execute tests** in `vm/src/vm.rs` — verifying the VM executes it correctly with normal inputs, edge values, and error conditions
3. **One end-to-end test** in `codegen/tests/end_to_end.rs` — verifying the complete pipeline produces the correct runtime result

This "test-per-opcode" discipline ensures that no instruction is added without being tested at all three levels of the stack.
