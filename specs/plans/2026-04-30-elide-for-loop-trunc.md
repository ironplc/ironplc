# Plan: Elide TRUNC in FOR Loops With Constant, In-Range Bounds

## Context

FOR loops over narrow integer types (`SINT`, `INT`, `USINT`, `UINT`) currently
emit a `TRUNC_*` opcode in two places per loop:

1. After the `from` expression (`compile_stmt.rs:849-851`).
2. After the increment `ADD` (`compile_stmt.rs:898-901`).

The increment-side TRUNC executes once per iteration, so the profiling test at
`compiler/benchmarks/tests/profile_for_loop.rs:127` measures
`TRUNC_I16 >= 100` for a 100-iteration `FOR i:INT := 1 TO 100` loop. The test's
own comment calls this out as a planned optimisation pointing at
`specs/design/vm-performance.md` §13a (interval analysis).

This plan adds a conservative, FOR-loop-local check that elides both TRUNC
sites when the iteration values are provably within the declared type's range.
It is the first slice of §13a — not a general interval-analysis pass — and
yields the headline win (one fewer opcode per FOR iteration over
`INT`/`SINT`/`UINT`/`USINT`) with very small surface area.

## Approach

Add helpers in `compile_stmt.rs` that ask: **given `from`, `to`, `step` and
the declared type's range, is every value the control variable can hold
(initial value, every body-visible value, and the post-final-increment value)
within `[T_min, T_max]`?** When the answer is yes, skip both
`emit_truncation` calls.

### Soundness condition

For declared narrow type with range `[T_min, T_max]` and constant bounds
`from`, `to`, constant `step` (default `1`):

- **Positive step (`step > 0`)**: elide when
  - `T_min <= from <= T_max` (init in range), AND
  - `to <= T_max` (every body value is `<= to`), AND
  - `to.checked_add(step)` exists and `<= T_max` (post-final increment in
    range — guards against the `FOR i:INT := 1 TO 32767` boundary case
    where the loop's existing wrap-around behaviour is preserved verbatim).
- **Negative step (`step < 0`)**: mirror — `T_min <= to`, `from <= T_max`,
  `to.checked_add(step) >= T_min`.
- **Zero step**: keep TRUNC (degenerate; do not optimise).
- **Any non-constant `from`/`to`/`step`**: keep TRUNC.
- **Non-narrow type** (`storage_bits in {32, 64}`): `emit_truncation` is
  already a no-op; nothing changes.

For positive step the body sees values in `[from, to]` and the post-final
stored value is at most `to + step`; all are in `[T_min, T_max]`, so
wrap-around cannot change observable behaviour. The argument is symmetric
for negative step.

### Why not "always elide"

Unconditionally removing the increment TRUNC would change post-loop
observable behaviour (the residual value of `i` after the loop) for cases
that currently rely on wrap-around at the type boundary. The conservative
rule keeps current behaviour for any program where the bounds aren't both
constants in range, and for in-range constant bounds the residual value is
already what TRUNC would have produced.

## File Map

| File | Change |
|------|--------|
| `compiler/codegen/src/compile_stmt.rs` | Add `try_constant_i64`, `narrow_type_range`, `for_loop_trunc_can_be_elided` near `try_constant_sign`; gate both `emit_truncation` sites in `compile_for` on the new check |
| `compiler/codegen/tests/compile_loops.rs` | Bytecode tests for elision (INT/SINT/UINT, negative step) and for non-elision (boundary `to`, non-constant `to`) |
| `compiler/codegen/tests/end_to_end_loops.rs` | Execution tests for `INT`/`SINT`/`UINT` FOR loops, including the boundary `INT := 32760 TO 32767` case |
| `compiler/benchmarks/tests/profile_for_loop.rs` | Update `profile_for_loop_int_*` to assert `TRUNC_I16 == 0`; refresh the comment at lines 139-142 to point at this plan |

## Reused Infrastructure

- `signed_integer_to_i64` (`compile_expr.rs:1559`) — literal to `i64` (handles
  both `IntegerLiteral` and unary-`Neg(IntegerLiteral)` patterns).
- `try_constant_sign` (`compile_stmt.rs:777`) — same AST patterns we need to
  inspect; the new `try_constant_i64` reuses the literal-extraction logic.
- `emit_truncation` (`compile_expr.rs:1586`) — unchanged; we just stop
  calling it conditionally.
- `VarTypeInfo` (`compile.rs:88`) — already carries `op_width`,
  `signedness`, `storage_bits`, all that's needed for the range table.

## Verification

From `compiler/`:

1. `just compile` — clean build.
2. `cargo test -p ironplc-codegen --test compile_loops` — bytecode tests
   pass.
3. `cargo test -p ironplc-codegen --test end_to_end_loops` — runtime
   correctness, including the boundary-`32767` case.
4. `cargo test -p ironplc-benchmarks --features profiling -- --nocapture
   --test-threads=1 profile_for_loop` — `FOR_LOOP_INT` histogram now shows
   `TRUNC_I16 == 0` and unchanged `ADD_I32` / `STORE_VAR_I32` counts.
5. `just` — full CI (clippy + fmt + coverage >= 85 %).

## Out of Scope

- General interval-analysis pass over arbitrary expressions, array indices,
  or division operands (the rest of `vm-performance.md` §13a).
- Eliding TRUNC after `STORE` for assignments outside FOR loops.
- Changing the boundary-wrap behaviour in non-optimised cases (when bounds
  are non-constant or out of range, the current TRUNC-then-loop behaviour
  is preserved verbatim).
