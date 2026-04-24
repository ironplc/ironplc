# Plan: Support reading function block instance fields via dot-access

## Summary

Codegen currently rejects reading a function block instance's output via dot-access syntax (e.g. `result := PulseTimer.Q`), emitting `P9999: Variable 'PulseTimer' is not a structure`. The analyzer already accepts the program; the missing capability is in codegen. This plan adds a dedicated emission path for the single-level `fb.field` rvalue case for both standard library FBs (TON, TOF, TP, counters, etc.) and user-defined FBs.

## Problem

This program fails compilation, though it follows IEC 61131-3 semantics:

```iec
PROGRAM main
  VAR
    Button : BOOL;
    Buzzer : BOOL;
    PulseTimer : TON;
  END_VAR
  PulseTimer(IN := NOT Button, PT := T#500ms);
  Buzzer := PulseTimer.Q;
END_PROGRAM
```

In `compiler/codegen/src/compile_expr.rs`, `compile_variable_read` handles `Variable::Symbolic(SymbolicVariableKind::Structured(..))` by calling `walk_struct_chain`. That helper only looks up `ctx.struct_vars`; FB instances live in the parallel `ctx.fb_instances` map (populated by `compile_setup.rs:109-157` for both stdlib and user FBs). When the root is an FB instance, `walk_struct_chain` produces the misleading "'X' is not a structure" diagnostic. The same defect exists for user-defined FBs (`fb.y` after `fb(x := 7)`).

## Change

### `compiler/codegen/src/compile_expr.rs`

In the `Structured` arm of `compile_variable_read` (around line 621), insert an early branch **before** `walk_struct_chain` is called:

1. If `structured.record` is `SymbolicVariableKind::Named(named)` and `named.name` is a key in `ctx.fb_instances`:
   - Lookup `field_idx = fb_info.field_indices.get(&lowercase(structured.field))`.
   - If the field name is unknown: emit an inline diagnostic ("Unknown field 'X' on function block 'Y'"). This is a better message than the current fall-through.
   - Else emit the read sequence:
     1. `emitter.emit_fb_load_instance(fb_info.var_index)` — pushes `fb_ref` (stack: `[fb_ref]`)
     2. `emitter.emit_fb_load_param(field_idx)` — VM peeks `fb_ref`, reads field from data region, pushes value (stack: `[fb_ref, value]`)
     3. `emitter.emit_swap()` — stack: `[value, fb_ref]`
     4. `emitter.emit_pop()` — stack: `[value]`
   - Return `Ok(())`.

Net stack effect: +1 (one value), matching the contract of `compile_variable_read`.

Rationale for not modifying `walk_struct_chain`: its contract is "walk a `struct_vars`-rooted chain." FB fields use a separate storage region (addressed via `FB_LOAD_PARAM`), so a unified walker would contaminate the STRING/array branches that depend on its current return shape.

### Reused, not duplicated

- `ctx.fb_instances: HashMap<Id, FbInstanceInfo>` — `compiler/codegen/src/compile.rs:632`
- `emit_fb_load_instance` / `emit_fb_load_param` / `emit_swap` / `emit_pop` — `compiler/codegen/src/emit.rs:614, 630, 324, 608`
- `FbInstanceInfo.field_indices: HashMap<String, u8>` with lowercase keys — matches the lookup idiom used by `compile_fb_call` in `compile_stmt.rs:397, 413`.

## Tests

### `compiler/codegen/tests/end_to_end_fb_ton.rs`

Add:

1. **Regression — user's original program.** `PulseTimer : TON`, call `PulseTimer(IN := NOT Button, PT := T#500ms);` then `Buzzer := PulseTimer.Q;`. Run round at t=0, then t=600ms with `Button = FALSE` (so `IN = TRUE`): expect Buzzer TRUE. Fresh run, round at t=0 then t=100ms: expect Buzzer FALSE.
2. **Dot-access on ET (non-BOOL field).** `timer(IN := TRUE, PT := T#10s); elapsed := timer.ET;` — at t=3s, expect `elapsed == 3000`.

### `compiler/codegen/tests/end_to_end_user_fb.rs`

Add:

3. **User FB dot-access read.** Mirror of the existing `DOUBLER` test (line 15) using `fb(x := 7); result := fb.y;` — expect `result == 14`.
4. **User FB dot-access across rounds (stack hygiene).** Reuse the `ACCUMULATOR` pattern: `acc(val := 10); result := acc.sum;` across three rounds. Expect 10, 20, 30. Proves SWAP+POP cleans the fb_ref and the operand stack doesn't drift between rounds.

## Out of scope (tracked separately)

- Writing to FB fields via dot-access (`fb.IN := TRUE`) — [#949](https://github.com/ironplc/ironplc/issues/949).
- Nested chains (`fb1.fb2.Q`) — [#950](https://github.com/ironplc/ironplc/issues/950).
- Partial/bit access on FB fields (`fb.state.0`) — [#951](https://github.com/ironplc/ironplc/issues/951).

## Verification

From `compiler/`:

1. `just test` — all new and existing codegen tests pass (in particular `end_to_end_fb_ton` and `end_to_end_user_fb`).
2. End-to-end reproduction of the user's original program via the CLI:
   ```bash
   cargo run --bin ironplcc -- compile --output /tmp/ton.iplc /tmp/ton_test.st
   ```
   completes with exit 0 (currently emits P9999 and exits non-zero).
3. `just` — full CI (build + coverage ≥ 85% + clippy + fmt) green before opening the PR.

## Critical files

- `compiler/codegen/src/compile_expr.rs` — single arm edit.
- `compiler/codegen/tests/end_to_end_fb_ton.rs` — stdlib regression tests.
- `compiler/codegen/tests/end_to_end_user_fb.rs` — user FB dot-access tests.
- Reference-only (no changes):
  - `compiler/codegen/src/compile_stmt.rs` (emission model, `resolve_fb_field_op_type`).
  - `compiler/codegen/src/compile.rs` (`FbInstanceInfo`).
  - `compiler/codegen/src/emit.rs` (opcode helpers).
  - `compiler/vm/src/vm.rs:1685-1715` (VM semantics for `FB_LOAD_INSTANCE`, `FB_LOAD_PARAM`, `SWAP`).
