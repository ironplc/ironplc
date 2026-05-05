# Plan: Support writing to function block instance fields via dot-access

## Goal

Make `fb.IN := x;` and `fb.PT := T#5s;` compile and execute correctly, for both standard library function blocks (TON, TOF, TP, CTU, CTD, CTUD, R_TRIG, F_TRIG, RS, SR) and user-defined function blocks. Currently codegen emits `P9999: Variable 'X' is not a structure` when the assignment target has the form `Structured { record: Named, field }` and the named root is an FB instance.

This is the lvalue companion to [#945 / `2026-04-21-fb-dot-access-read.md`](2026-04-21-fb-dot-access-read.md), which handled the rvalue case. The analyzer already accepts the syntax; the missing capability is in codegen only.

## Architecture

Mirror the read-path branch: in the `Structured` arm of assignment handling in `compile_stmt.rs`, insert an early check **before** `walk_struct_chain` is called. If the target's root is an FB instance, emit a dedicated FB-write sequence using the existing `FB_STORE_PARAM` opcode (already supported by the VM, see `compiler/vm/src/vm.rs:1767-1776`).

Stack contract for the new emission:

1. `emit_fb_load_instance(var_index)` — pushes `fb_ref` → `[fb_ref]`
2. `compile_expr(... rhs ..., field_op_type)` — pushes value → `[fb_ref, value]`
3. `emit_fb_store_param(field_idx)` — pops value, leaves `fb_ref` → `[fb_ref]`
4. `emit_pop()` — discards `fb_ref` → `[]`

Net stack effect: 0, matching the assignment statement contract.

`field_op_type` is resolved via the existing `resolve_fb_field_op_type(ctx, type_id, field_name)` helper (`compile_stmt.rs:384-395`) so user FBs use `user_fb_types.field_op_types` and stdlib FBs fall back to the hardcoded `fb_field_op_type` mapping. This is identical to what `compile_fb_call` already does for `NamedInput` parameters at `compile_stmt.rs:421`.

If the FB root is recognized but the field name is unknown, emit a targeted "Unknown field 'X' on function block 'Y'" diagnostic — same wording as the read path uses (`compile_expr.rs:643-647`) — instead of falling through into `walk_struct_chain`'s misleading "is not a structure" message.

### Why a separate branch (not a unified walker)

FB fields live in a separate storage region addressed via `FB_STORE_PARAM`/`FB_LOAD_PARAM` (peeks fb_ref, indexes into the data region by field). `walk_struct_chain` is contracted to walk a `struct_vars`-rooted slot chain and returns a slot offset, which is meaningless for an FB instance. Reusing it would require contaminating the STRING/array branches that depend on its return shape. The read path took the same call: insert an early branch and return.

### Out of scope (deliberately deferred)

- **Nested FB chains** (`fb1.fb2.IN := x`) — tracked separately in [#950](https://github.com/ironplc/ironplc/issues/950). The early-exit check matches only `record: Named` (single level), so nested forms fall through to the existing path and continue to error. This is intentional.
- **Partial/bit access on FB fields** (`fb.state.0 := TRUE`) — tracked separately in [#951](https://github.com/ironplc/ironplc/issues/951). The bit-access target check at `compile_stmt.rs:89` runs before the new branch, so this defers via the existing not-implemented path rather than the new one.
- **Restricting writes to `VAR_INPUT` only** — the issue notes IEC 61131-3 reserves dot-assignment for `VAR_INPUT`, and writing to outputs/internal `VAR` should arguably be rejected. Per the issue's "open questions": that is a separate analyzer rule, not a codegen concern. This plan compiles whatever the analyzer accepts; tightening can happen later.

## File map

Modified:

- `compiler/codegen/src/compile_stmt.rs` — insert FB-instance branch in the `Structured` arm of the `Assignment` case in `compile_statement` (around lines 103-144, before `walk_struct_chain`).
- `compiler/codegen/tests/it/end_to_end_fb_ton.rs` — stdlib write-via-dot-access regression tests.
- `compiler/codegen/tests/it/end_to_end_user_fb.rs` — user FB write-via-dot-access tests.

No other files change. All required infrastructure exists:

- `ctx.fb_instances: HashMap<Id, FbInstanceInfo>` — `compile.rs:687`
- `FbInstanceInfo.field_indices` (lowercase keys) — `compile.rs:655`
- `emit_fb_load_instance` / `emit_fb_store_param` / `emit_pop` — `emit.rs:677, 685, 671`
- `resolve_fb_field_op_type` — `compile_stmt.rs:384`
- VM `FB_STORE_PARAM` — `vm.rs:1767-1776`

## Tasks

- [ ] Add the FB-instance early-exit branch in `compile_stmt.rs` Assignment handler:
  - [ ] Pattern-match `Variable::Symbolic(Structured { record, field })` where `record.as_ref()` is `SymbolicVariableKind::Named(named)` and `named.name` is in `ctx.fb_instances`.
  - [ ] Look up `field_idx` via `fb_info.field_indices.get(&field.to_string().to_lowercase()).copied()`.
  - [ ] On unknown field: return `Diagnostic::problem(Problem::NotImplemented, ...)` with message `"Unknown field '{field}' on function block '{name}'"`, span on the field.
  - [ ] Resolve `op_type` via `resolve_fb_field_op_type(ctx, fb_info.type_id, &field_name_lower)`.
  - [ ] Emit: `emit_fb_load_instance(var_index)` → `compile_expr(..., op_type)` → `emit_fb_store_param(field_idx)` → `emit_pop()`.
  - [ ] Place the branch **before** the existing `Structured` block at `compile_stmt.rs:104` so struct-field writes still take the existing path.
- [ ] Add stdlib regression tests in `compiler/codegen/tests/it/end_to_end_fb_ton.rs`:
  - [ ] `end_to_end_when_ton_dot_access_writes_in_then_timer_runs` — `timer.IN := TRUE; timer.PT := T#500ms;` followed by `timer();`. Run round at t=0, then t=600_000us. Assert `timer.Q` (read via dot-access) is TRUE.
  - [ ] `end_to_end_when_ton_dot_access_writes_pt_then_timer_uses_new_period` — set PT via dot-access, verify Q toggles at the configured threshold.
- [ ] Add user-FB tests in `compiler/codegen/tests/it/end_to_end_user_fb.rs`:
  - [ ] `end_to_end_when_user_fb_dot_access_write_then_input_set` — Mirror of the existing `DOUBLER` test (line 14): `fb.x := 7; fb(); result := fb.y;` — expect `result == 14`. Proves the write path round-trips through a subsequent FB call and dot-access read.
  - [ ] `end_to_end_when_user_fb_dot_access_write_unknown_field_then_diagnostic` — `fb.bogus := 1;` should produce a diagnostic mentioning the field name and FB instance name (compile-only test using `try_parse_and_compile`).
- [ ] Add codegen unit test for the unknown-field diagnostic (parses a stdlib FB program with `timer.NOPE := TRUE;` and asserts the error message). Place near the existing FB compile error tests (search `not a structure` for siblings).
- [ ] Verify the original issue's program compiles and runs:
  ```iec
  PROGRAM main
    VAR timer : TON; END_VAR
    timer.IN := TRUE;
    timer.PT := T#500ms;
    timer();
  END_PROGRAM
  ```
- [ ] Run `cd compiler && just` — full CI (compile + coverage ≥ 85% + clippy + fmt) must be green before opening the PR.
- [ ] Open the PR; reference and close issue [#949](https://github.com/ironplc/ironplc/issues/949).

## Verification

From `compiler/`:

1. `just test` — all new and existing codegen tests pass, in particular `end_to_end_fb_ton` and `end_to_end_user_fb`.
2. End-to-end CLI reproduction of the issue's program: `cargo run --bin ironplcc -- compile --output /tmp/ton.iplc /tmp/ton_test.st` exits 0 (currently emits P9999 and exits non-zero).
3. `just` — full CI green.

## Critical files

- `compiler/codegen/src/compile_stmt.rs` — single arm edit in the assignment handler.
- `compiler/codegen/tests/it/end_to_end_fb_ton.rs` — stdlib regression tests.
- `compiler/codegen/tests/it/end_to_end_user_fb.rs` — user FB tests.
- Reference-only (no changes):
  - `compiler/codegen/src/compile_expr.rs:624-655` — symmetric read path, model for the new write path.
  - `compiler/codegen/src/compile_stmt.rs:382-456` — `compile_fb_call`, `resolve_fb_field_op_type`, `fb_field_op_type`.
  - `compiler/codegen/src/compile.rs:645-672` — `FbInstanceInfo`, `UserFbTypeInfo`.
  - `compiler/codegen/src/emit.rs:670-697` — opcode helpers.
  - `compiler/vm/src/vm.rs:1761-1787` — VM semantics for `FB_LOAD_INSTANCE`, `FB_STORE_PARAM`, `FB_LOAD_PARAM`.
