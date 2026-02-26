# Design: VM Testing Strategy

## Overview

The IronPLC bytecode VM (`ironplc-vm`) executes IEC 61131-3 programs compiled to a stack-based bytecode format. The VM is in an early state — it supports 5 opcodes (`LOAD_CONST_I32`, `LOAD_VAR_I32`, `STORE_VAR_I32`, `ADD_I32`, `RET_VOID`), a freewheeling task scheduler, and a typestate lifecycle (`Vm` → `VmReady` → `VmRunning` → `VmStopped`/`VmFaulted`).

This document describes what VM testing should look like: what properties to verify, at what granularity, and how the testing approach interacts with the VM's design constraints (embedded targets, deterministic execution, safety-first philosophy).

### Building On

- **[ADR-0000: Stack-Based Bytecode VM](../adrs/0000-stack-based-bytecode-vm.md)** — the execution model
- **[ADR-0005: Safety-First Design Principle](../adrs/0005-safety-first-design-principle.md)** — correctness over performance
- **[ADR-0006: Bytecode Verification Requirement](../adrs/0006-bytecode-verification-requirement.md)** — the verifier that runs before execution
- **[ADR-0009: Typestate VM Lifecycle](../adrs/0009-typestate-vm-lifecycle.md)** — the state machine enforced by Rust's type system
- **[ADR-0010: no_std VM for Embedded Targets](../adrs/0010-no-std-vm-for-embedded-targets.md)** — the embedded deployment constraint

## Current State of Testing

### What Exists

The VM has tests at four levels today:

**1. Unit tests (inline `#[cfg(test)]` modules)**

| Module | Tests | What they cover |
|---|---|---|
| `value.rs` | 2 | `Slot` roundtrip for positive and negative i32 |
| `stack.rs` | 3 | Push/pop, overflow, underflow |
| `variable_table.rs` | 7 | Load/store, out-of-bounds, `VariableScope` access checks |
| `error.rs` | 1 | `Trap::Display` formatting |
| `scheduler.rs` | 8 | Task readiness, priority ordering, cyclic timing, overrun detection, disabled tasks |
| `vm.rs` | 6 | Load/start/stop/fault lifecycle, steel thread execution, invalid opcode trap, stop handle |
| `logger.rs` | 1 | Verbosity bound check |

Total: **28 unit tests** across 7 modules.

**2. Integration tests (`tests/` directory)**

| File | Tests | What they cover |
|---|---|---|
| `steel_thread.rs` | 1 | Full round-trip: hand-assembled bytecode → container serialize/deserialize → VM execution → variable read |
| `cli.rs` | 5 | CLI binary: run with valid/invalid containers, `--dump-vars`, golden file check, version command |

Total: **6 integration tests**.

**3. End-to-end tests (in `codegen/tests/`)**

| File | Tests | What they cover |
|---|---|---|
| `end_to_end.rs` | 8 | Source text → parse → codegen → VM execution → variable check. Covers: simple assignment, add, chain of adds, multiple assignments, negative constants, zero, variable copy, idempotent multi-scan |

Total: **8 end-to-end tests**.

**4. Codegen unit tests (in `codegen/src/`)**

| Module | Tests | What they cover |
|---|---|---|
| `compile.rs` | 8 | Source → container: bytecode shape, constant pool, deduplication, empty program, error on unsupported constructs |
| `emit.rs` | 7 | Emitter: each opcode, stack depth tracking, endianness |

Total: **15 codegen tests**.

### What Is Missing

The current tests cover the "happy path" steel-thread scenario well but have significant gaps:

**Instruction-level coverage**: Only `LOAD_CONST_I32`, `LOAD_VAR_I32`, `STORE_VAR_I32`, `ADD_I32`, and `RET_VOID` are tested (the only 5 opcodes that exist). As the instruction set grows per the [bytecode instruction set plan](../plans/bytecode-instruction-set.md), each new opcode needs systematic testing.

**Error path coverage**: The `execute()` function has 5 `Trap` variants that can fire during execution, but only `InvalidInstruction` is tested through the VM. `StackOverflow`, `StackUnderflow`, `InvalidConstantIndex`, and `InvalidVariableIndex` are tested at the component level (stack, variable table) but not through the full `execute()` path.

**Scheduler integration**: The scheduler is well unit-tested, but there are no tests that exercise the scheduler driving actual bytecode execution across multiple tasks with separate variable scopes.

**Multi-scan statefulness**: Only one test (`end_to_end_when_multiple_scans_then_idempotent`) runs more than one scan cycle, and it verifies idempotency of a stateless program. There are no tests for programs that accumulate state across scans (e.g., a counter that increments each cycle).

**Watchdog behavior**: The `WatchdogTimeout` trap is defined and the check exists in `run_round()`, but there are no tests that trigger it.

**Variable scope isolation**: `VariableScope::check_access` is unit-tested, but no integration test verifies that one program instance cannot access another instance's variables when running through the scheduler.

**Container format robustness**: The container read/write path is tested for valid containers. There are no tests for truncated containers, containers with out-of-range indices, or containers that would cause the VM to fault.

## Testing Layers

The VM testing strategy uses four layers, each with a distinct purpose:

### Layer 1: Component Unit Tests

**Purpose**: Verify individual data structures and algorithms in isolation.

**Scope**: `Slot`, `OperandStack`, `VariableTable`, `VariableScope`, `Trap`, `TaskScheduler`, `Emitter`.

**Properties to test**:
- Data roundtripping (put a value in, get the same value out)
- Boundary conditions (empty, full, off-by-one)
- Error conditions (overflow, underflow, out-of-bounds)
- Ordering invariants (scheduler priority, LIFO stack order)

**Who provides these tests**: The modules themselves, in `#[cfg(test)]` blocks.

**Current coverage**: Good for existing components. Each new data type or component should follow the same pattern.

### Layer 2: Instruction Tests

**Purpose**: Verify that each bytecode instruction executes correctly in isolation.

**Scope**: The `execute()` function in `vm.rs`, exercised one instruction (or short instruction sequence) at a time.

**Properties to test for each opcode**:
- **Normal execution**: Given well-formed inputs, the instruction produces the correct stack and variable state
- **Edge values**: Boundary values for the type (i32::MIN, i32::MAX, 0, -1)
- **Overflow/underflow behavior**: Wrapping, saturating, or trapping per the type semantics (see ADR-0002)
- **Error conditions**: What happens when the instruction's preconditions are violated (empty stack, invalid index)

**How to write these tests**: Use `ContainerBuilder` to construct a minimal container with just enough bytecode for the instruction under test, then verify the VM state after one `run_round()`. This is the approach already used by `vm_run_round_when_steel_thread_then_x_is_10_y_is_42` and `vm_run_round_when_invalid_opcode_then_trap`.

**Pattern**:
```rust
#[test]
fn execute_add_i32_when_max_plus_one_then_wraps() {
    // Bytecode: LOAD_CONST pool[0] (i32::MAX), LOAD_CONST pool[1] (1), ADD_I32, STORE_VAR var[0], RET_VOID
    let bytecode = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]
        0x30,              // ADD_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let container = ContainerBuilder::new()
        .num_variables(1)
        .add_i32_constant(i32::MAX)
        .add_i32_constant(1)
        .add_function(0, &bytecode, 2, 1)
        .build();

    let mut vm = Vm::new().load(container).start();
    vm.run_round().unwrap();

    assert_eq!(vm.read_variable(0).unwrap(), i32::MIN); // wrapping add
}
```

**Where these tests live**: `compiler/vm/src/vm.rs` in the `#[cfg(test)]` module. As the instruction set grows, a dedicated `tests/instructions.rs` integration test file may be warranted to keep `vm.rs` under the 1000-line limit.

### Layer 3: Scenario Tests

**Purpose**: Verify that the VM executes realistic PLC program patterns correctly across multiple components working together.

**Scope**: Complete programs — possibly hand-assembled bytecode, possibly compiled from source — exercising the scheduler, multiple program instances, variable scoping, and multi-scan execution.

**Scenarios to test**:

| Scenario | What it exercises |
|---|---|
| Counter that increments each scan | State persistence across scans, read-modify-write pattern |
| Two tasks with different priorities | Scheduler priority ordering, variable scope isolation |
| Cyclic task timing | Scheduler interval tracking, `next_due_us` behavior |
| Program that faults mid-execution | Trap propagation, `VmFaulted` state, variable dump after fault |
| Empty program (RET_VOID only) | Degenerate case, no work per scan |
| Program with maximum stack usage | Stack depth equals `max_stack_depth` exactly, no overflow |
| Program that exceeds stack depth | `StackOverflow` trap through `execute()` |

**Where these tests live**: `compiler/vm/tests/` as integration test files. Group by concern:
- `tests/instructions.rs` — per-opcode tests (Layer 2, when extracted from unit tests)
- `tests/scenarios.rs` — multi-scan and multi-task scenarios
- `tests/faults.rs` — error paths and trap handling

### Layer 4: End-to-End Pipeline Tests

**Purpose**: Verify the complete path from IEC 61131-3 source text through the parser, codegen, container serialization, and VM execution.

**Scope**: These tests live in `codegen/tests/end_to_end.rs` and represent the highest level of integration.

**Properties to test**:
- Source language constructs produce the correct runtime behavior
- The codegen → container → VM contract is upheld
- New language features work end-to-end, not just in codegen

**What distinguishes this from Layer 3**: Layer 3 uses hand-assembled bytecode (testing the VM in isolation from the compiler). Layer 4 starts from source text (testing the whole pipeline). Both are necessary — Layer 3 catches VM bugs without compiler interference, Layer 4 catches integration bugs between compiler and VM.

## Test Properties by VM Component

### Execute Function (`vm.rs:execute`)

The `execute()` function is the core interpreter loop. It is a free function (not a method) to allow split borrows of immutable container data and mutable stack/variable state. Key properties:

| Property | Description | Test approach |
|---|---|---|
| Instruction dispatch correctness | Each opcode performs its documented operation | One test per opcode with known inputs/outputs |
| Stack discipline | Instructions push/pop the correct number of values | Verify stack state after instruction sequences |
| Program counter advancement | Each instruction advances `pc` by `1 + operand_size` | Implicit — if the next instruction executes correctly, pc advanced correctly |
| Termination on `RET_VOID` | Execution stops and returns `Ok(())` | Already tested |
| Termination on end-of-bytecode | Execution stops if `pc >= bytecode.len()` without `RET_VOID` | Needs test — currently returns `Ok(())` silently |
| Trap propagation | Each error condition returns the correct `Trap` variant | One test per trap variant through `execute()` |

### Scheduler (`scheduler.rs`)

| Property | Description | Test approach |
|---|---|---|
| Freewheeling tasks always ready | `collect_ready_tasks` returns freewheeling tasks every round | Existing test |
| Cyclic timing | Tasks become ready at `next_due_us` intervals | Existing tests |
| Priority ordering | Lower priority number executes first | Existing test |
| Overrun detection | When a cyclic task misses its deadline, overrun count increments and timing realigns | Existing test |
| Disabled tasks skipped | Tasks with `enabled == false` are never scheduled | Existing test |
| Multi-program tasks | Programs within a task execute in declaration order | Needs scenario test through `run_round()` |

### Variable Scope Isolation

| Property | Description | Test approach |
|---|---|---|
| Shared globals accessible | Both task instances can read/write indices `0..shared_globals_size` | Scenario test with shared global communication |
| Instance variables private | Instance A cannot access Instance B's variable partition | Scenario test where out-of-scope access traps |
| Scope check on every access | Both `LOAD_VAR` and `STORE_VAR` validate through `VariableScope` | Unit test in `vm.rs` with restricted scope |

### Typestate Lifecycle

The typestate pattern (ADR-0009) means that invalid state transitions are compile-time errors. The testing strategy for this is:

- **Positive tests**: Verify the valid transition sequence compiles and works (`Vm::new().load(c).start().run_round()`)
- **Negative tests**: Not needed at runtime — the compiler rejects invalid sequences. A `compile_fail` test could document this, but it is low priority since the typestate is already structurally enforced.

### Fault Handling

| Property | Description | Test approach |
|---|---|---|
| `VmFaulted` captures context | `trap`, `task_id`, and `instance_id` are preserved | Existing test (`vm_fault_when_called_then_returns_faulted_with_context`) |
| Variable readable after fault | `read_variable()` works on `VmFaulted` | Needs test |
| Fault during multi-task round | If task 2 faults, task 1's side effects are visible | Scenario test |

## Test Naming Convention

All VM tests follow the project's BDD convention:

```
{subject}_when_{condition}_then_{expected_result}
```

Examples:
- `execute_add_i32_when_both_positive_then_correct_sum`
- `execute_add_i32_when_max_plus_one_then_wraps`
- `scheduler_run_round_when_two_tasks_then_priority_order`
- `vm_stop_when_running_then_scan_count_preserved`

For per-opcode tests, the subject is `execute_{opcode_name}`.

## Embedded Target Testing Considerations

ADR-0010 specifies that the VM will become `no_std` for embedded targets. This affects the testing strategy:

**What stays testable on the host**: The core execution engine, scheduler, stack, variable table, and all instruction logic. These components do not use `std` today (they use `Vec`, which will move to fixed-size buffers). All tests in Layers 1-3 remain valid and run on the host.

**What needs platform-specific testing**: The actual embedded deployment — loading a container from flash, allocating buffers on the stack, executing within timing constraints. This is out of scope for the current test suite but should be planned for:
- A `thumbv7em-none-eabihf` compilation check (CI: "does it compile for ARM?")
- A QEMU-based execution test (if the team has bandwidth for an embedded test harness)

**Test portability**: Tests should not depend on `std` features that would be unavailable in `no_std`. Specifically:
- Avoid `std::time::Instant` in test assertions (the scheduler already takes `current_time_us` as a parameter, making it clock-agnostic)
- Avoid filesystem I/O in instruction/scenario tests (use in-memory containers from `ContainerBuilder`)
- The CLI tests (`tests/cli.rs`) are inherently `std`-only and will move to the `ironplc-vm-cli` crate when the split happens

## Test Infrastructure

### `ContainerBuilder` as the Primary Test Tool

The `ContainerBuilder` in `ironplc-container` is the right tool for constructing test containers. It provides:
- Fluent API for adding constants, functions, tasks, program instances
- Automatic synthesis of a default freewheeling task when no tasks are specified
- Correct header computation

For instruction tests, a helper function in the test module can reduce boilerplate:

```rust
/// Builds a container with one function from the given bytecode,
/// with `num_vars` variables and the given constants.
fn single_function_container(bytecode: &[u8], num_vars: u16, constants: &[i32]) -> Container {
    let mut builder = ContainerBuilder::new().num_variables(num_vars);
    for &c in constants {
        builder = builder.add_i32_constant(c);
    }
    let max_stack = 16; // generous for tests
    builder.add_function(0, bytecode, max_stack, num_vars).build()
}
```

### Assertion Helpers

For checking variable state after execution, the existing `read_variable(index)` method suffices. For checking fault conditions:

```rust
/// Asserts that a run_round produces a specific trap.
fn assert_trap(vm: &mut VmRunning, expected: Trap) {
    let result = vm.run_round();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().trap, expected);
}
```

These helpers should live in a shared test utility, not in the production code.

## Relationship to the Bytecode Verifier

ADR-0006 specifies that a bytecode verifier will run before execution, rejecting malformed containers. This creates a division:

- **Verifier tests**: Verify that the verifier rejects bad bytecode (invalid indices, type mismatches, unreachable code). These are tests of the verifier, not the VM.
- **VM execute tests**: Verify that the VM handles runtime traps correctly (traps that can occur even in verified bytecode, like divide-by-zero or watchdog timeout). Also verify defense-in-depth: what happens if unverified bytecode reaches the VM.

Both are needed. The VM's `Trap` handling is a safety net, not a substitute for verification. But the safety net must be tested independently.

## Test Priority

For the immediate next step (before expanding the instruction set), the highest-value tests to add are:

1. **Execute error paths**: `StackOverflow`, `StackUnderflow`, `InvalidConstantIndex`, `InvalidVariableIndex` through the `execute()` function (not just at the component level)
2. **Multi-scan counter**: A program that increments a variable each scan, verifying state persistence
3. **Watchdog timeout**: A test that triggers `WatchdogTimeout` through `run_round()`
4. **Variable scope isolation**: Two program instances with separate variable partitions
5. **End-of-bytecode without RET_VOID**: Document whether this is valid behavior or should trap

These fill the most critical gaps in the current test suite and establish patterns that scale as the instruction set grows.
