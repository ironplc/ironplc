# Fix optimizer crash on string opcodes (optimize.rs:366)

## Problem

The peephole optimizer crashes with a HashMap key-not-found panic at
`optimize.rs:366` when processing bytecode containing string operations
followed by jump instructions.

## Root cause

The `instruction_size()` function in `optimize.rs` used incorrect byte sizes
for 12 string opcodes. The opcode doc comments said `u16` for `data_offset`
operands, but the emitter and VM both use `u32` (4 bytes). The optimizer's
decoder therefore read too few bytes for string instructions, got out of sync,
and built an `offset_map` with wrong instruction boundaries. When a jump
target's real offset was not in this corrupted map, the lookup panicked.

## Fix

1. Move `instruction_size()` into `ironplc_container::opcode` as a shared
   function so that the emitter and optimizer use a single source of truth.
2. Delete the duplicate `instruction_len()` from `emit.rs` and the local
   `instruction_size()` from `optimize.rs`; both now call the shared function.
3. Correct the string opcode doc comments in `opcode.rs` (`u16` -> `u32`).
4. Add regression tests that construct bytecode with string opcodes followed
   by jumps, verifying the optimizer does not panic.
