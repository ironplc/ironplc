# Optimize VM Codegen with DUP Opcode

## Summary

Use the DUP VM opcode to reduce bytecode size and improve execution speed
by eliminating redundant load instructions via two peephole optimizations.

## Changes

### Optimization 1: Consecutive Identical Load Elimination

When the same LOAD_VAR or LOAD_CONST instruction is emitted twice in a row
(e.g., `x * x`), the second load is replaced with a 1-byte DUP instead of
the original 3-byte load instruction.

Implemented via `emit_load_with_dup_check` in the Emitter. All non-load
emissions go through `emit_opcode()` which automatically clears the tracker,
avoiding duplicated `clear_last_load()` calls.

### Optimization 2: Store-Load Elimination

When STORE_VAR is immediately followed by LOAD_VAR of the same variable
(e.g., `x := expr; y := x + 1`), the pair is rewritten in-place to
`DUP, STORE_VAR, NOP, NOP` — same total length preserves jump offsets.

Implemented as a post-bytecode peephole pass that runs after label resolution.

### Infrastructure

- Added NOP (0xA3) opcode for peephole padding
- Added `emit_opcode()` helper to centralize `last_load` invalidation
