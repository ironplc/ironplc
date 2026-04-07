# Plan: Peephole Identity Optimizer (Post-Emission)

## Context

The codegen emitter already performs two in-line peephole optimizations
(see `specs/plans/2026-04-07-optimize-vm-with-dup.md`):

1. Consecutive identical loads → `DUP` (1 byte instead of 3).
2. Store-load of the same variable → `DUP; STORE; NOP; NOP`.

Neither of these handles arithmetic identities (`x + 0`, `x * 1`) or
redundant self-assignment (`LOAD_VAR x; STORE_VAR x`) because those
patterns depend on knowing constant pool values, which the emitter does
not consult. This plan adds a small post-emission pass that runs over the
final byte buffer with access to the constant pool.

## Architecture

New module: `compiler/codegen/src/optimize.rs`.

Runs after `emitter.bytecode()` returns, before the bytes flow into
`ContainerBuilder::add_function`. Receives a `&[PoolConstant]` so it can
look up constant values by pool index.

Algorithm:

1. Decode the byte buffer into a `Vec<Instruction>` (offset + raw bytes),
   recording jump targets as it goes.
2. Scan adjacent pairs. Skip any pair where either instruction is a jump
   target (preserves basic-block boundaries).
3. If a pair matches a removable pattern, mark both for deletion.
4. Rebuild the byte buffer, rewriting `JMP` / `JMP_IF_NOT` offsets via an
   old-offset → new-offset map.

## Patterns

| Pattern | Width | Removed |
|---------|-------|---------|
| `LOAD_VAR_X n; STORE_VAR_X n` (same var, same type) | I32/I64/F32/F64 | both |
| `LOAD_CONST_X pool[i]; ADD_X` where `pool[i] == 0` | I32/I64/F32/F64 | both |
| `LOAD_CONST_X pool[i]; SUB_X` where `pool[i] == 0` | I32/I64/F32/F64 | both |
| `LOAD_CONST_X pool[i]; MUL_X` where `pool[i] == 1` | I32/I64/F32/F64 | both |
| `LOAD_CONST_X pool[i]; DIV_X` where `pool[i] == 1` | I32/I64/F32/F64 | both |

## File Map

| File | Change |
|------|--------|
| `compiler/codegen/src/optimize.rs` | **New** — peephole pass + unit tests |
| `compiler/codegen/src/lib.rs` | `mod optimize;` |
| `compiler/codegen/src/compile.rs` | Call `optimize()` for init/scan; make `PoolConstant` and `CompileContext::constants` `pub(crate)` |
| `compiler/codegen/src/compile_fn.rs` | Call `optimize()` for user functions and FB bodies |

## Verification

`cd compiler && just` must pass. Unit tests in `optimize.rs` cover each
pattern across all widths plus jump-offset adjustment and jump-target
safety. Existing end-to-end tests implicitly verify semantic equivalence.
