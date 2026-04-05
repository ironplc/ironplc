# Split compile.rs into focused modules

## Problem

`compiler/codegen/src/compile.rs` is 6,011 lines, exceeding the 1,000-line module
limit by 6x. The file mixes expression compilation, statement/control-flow,
function call dispatch, and string operations.

## Approach

Extract four new modules, keeping `compile.rs` as orchestration:

| Module | Responsibility | ~Lines |
|--------|---------------|--------|
| `compile.rs` | Entry point, types, context, variable setup, tests | ~1000 |
| `compile_expr.rs` | Expression dispatch, constants, variable reads, opcode helpers | ~850 |
| `compile_stmt.rs` | Statements, control flow (IF/CASE/FOR/WHILE/REPEAT) | ~700 |
| `compile_call.rs` | Function call dispatch, builtins, type conversions, shifts | ~950 |
| `compile_string.rs` | String function compilation (LEN, FIND, CONCAT, etc.) | ~500 |

Follow the existing `compile_array.rs` / `compile_struct.rs` split pattern:
private `mod` in `lib.rs`, `pub(crate)` for cross-module items, `super::compile::*`
imports.

## Not changing

- No functional changes. Pure refactor.
- Tests stay in `compile.rs` (they test the public `compile()` entry point).
- `compile_array.rs`, `compile_struct.rs`, `emit.rs` are untouched.
