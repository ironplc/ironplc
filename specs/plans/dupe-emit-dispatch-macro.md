# Plan: Generate codegen `emit_<binop>` dispatch functions with a macro

## Problem

`compiler/codegen/src/compile_expr.rs` defines ~16 near-identical typed-opcode
dispatch functions (`emit_add`, `emit_sub`, ..., `emit_ge`). Each maps an
operand `OpType` to the matching `Emitter` method. The functions fall into a
small number of identical shapes, producing significant duplicated code that
the duplication gate flags.

## Approach

Introduce three `macro_rules!` generators in `compile_expr.rs` that use the
already-in-workspace `paste` crate to build the `Emitter` method idents by
concatenation. Every generated function keeps the SAME `pub(crate)` name and
`(emitter: &mut Emitter, op_type: OpType)` signature, so existing imports and
function-pointer uses in `compile_call.rs` and `compile_stmt.rs` are unchanged.

### Groups (folded)

1. `emit_width_op!(stem)` — width-only, matches `op_type.0`, calls
   `emit_<stem>_i32/_i64/_f32/_f64`. Applies to: `add`, `sub`, `mul`, `neg`,
   `eq`, `ne`.
2. `emit_signed_op!(stem)` — matches `(OpWidth, Signedness)`, calls
   `emit_<stem>_{i32,u32,i64,u64,f32,f64}`. Applies to: `div`, `lt`, `le`,
   `gt`, `ge`.
3. `emit_logical_op!(stem)` — matches `(OpWidth, Signedness)`; unsigned ints
   emit `emit_bit_<stem>_32/_64`, all else `emit_bool_<stem>`. Applies to:
   `and`, `or`, `xor`.

### Left hand-written (genuine one-offs)

- `emit_mod` — width+sign shape, but float arms are a no-op `{}` (integer-only
  MOD), so it does not fit `emit_signed_op!` (which would call `emit_mod_f32`).
- `emit_pow` — width-only shape, but calls `emit_builtin(opcode::builtin::EXPT_*)`
  rather than `emit_pow_i32`.
- `emit_load_var` / `emit_store_var` — different signature (`VarIndex` arg),
  out of scope.

## Validation

No behavior change: each generated match arm maps to the identical `Emitter`
method as today. VM end-to-end arithmetic/comparison tests are the safety net.
Run the full CI pipeline (compile, coverage >=85%, clippy, fmt, dupes gate).
