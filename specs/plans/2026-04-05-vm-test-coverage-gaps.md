# VM-Level Test Coverage Gaps

## Goal

Add focused VM-level integration tests with hand-crafted bytecode covering six
gap areas: f32 arithmetic, f64 arithmetic, string operations, user function
calls (CALL/RET), stack overflow, and data region boundary violations.

## Architecture

All tests follow the existing VM integration test pattern:

1. Construct bytecode by hand using opcode constants
2. Build a `Container` via `ContainerBuilder`
3. Allocate `VmBuffers`, load, start, and run one round
4. Assert variable values or expected traps

## File Map

| File | Purpose |
|------|---------|
| `compiler/vm/tests/common/mod.rs` | Add `run_and_read_f32` and `run_and_read_f64` helpers |
| `compiler/vm/tests/execute_arith_f32.rs` | f32 arithmetic: ADD, SUB, MUL, DIV, NEG |
| `compiler/vm/tests/execute_arith_f64.rs` | f64 arithmetic: ADD, SUB, MUL, DIV, NEG |
| `compiler/vm/tests/execute_string_ops.rs` | String ops: INIT, STORE, LOAD, LEN, CONCAT, FIND |
| `compiler/vm/tests/execute_call_ret.rs` | CALL/RET: single call, nested calls, return values |
| `compiler/vm/tests/execute_stack_overflow.rs` | Stack overflow trap on excessive pushes |
| `compiler/vm/tests/execute_data_region_oob.rs` | Data region out-of-bounds traps |

## Tasks

1. Add `run_and_read_f32` / `run_and_read_f64` helpers to `common/mod.rs`
2. Create each test file listed above
3. Run `cd compiler && just` to verify CI passes
