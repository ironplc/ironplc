# Fold `TRUNC_*` that follows a constant load

## Goal

Stop emitting a runtime `TRUNC_*` instruction when the value being truncated is
a constant known at code generation time. Either the constant already fits the
narrow type — in which case the `TRUNC_*` is a pure no-op and is deleted — or it
does not, in which case codegen applies the truncation itself and rewrites the
`LOAD_CONST_I32` to reference the already-truncated value.

Net effect: one fewer instruction executed per scan cycle for every narrow-typed
constant store, and one fewer byte of bytecode. Semantics are unchanged in every
case.

## Background: why the wasted instruction exists

Per [ADR-0001](../adrs/0001-bytecode-integer-arithmetic-type-strategy.md), all
sub-32-bit integer types (`SINT`, `INT`, `USINT`, `UINT`, `BYTE`, `WORD`) are
promoted to 32-bit on load, and the compiler emits an explicit `TRUNC_*` before
any store back to a narrow slot. `emit_truncation` (`codegen/src/compile_expr.rs`)
is called at roughly a dozen sites — scalar assignment, array element, struct
field, deref, initializers, function-call return values — and it always emits the
opcode, because at that point it only knows the *target's* type, not whether the
*value* on the stack is a compile-time constant.

`compile_constant` cannot compensate: it is handed an `OpType`
(`(OpWidth, Signedness)`) which collapses `SINT`/`INT`/`DINT` all to
`(W32, Signed)`. The `storage_bits` that decides the truncation width lives in
`VarTypeInfo`, one level up. So the constant is emitted at 32 bits and narrowed
afterwards.

The result is visible in the existing tests, which document the exact sequence:

```
// compiler/codegen/tests/it/compile_types.rs:22
// x : SINT;  x := 42;
// LOAD_CONST_I32 pool:0, TRUNC_I8, STORE_VAR_I32 var:0, RET_VOID
```

```
// compiler/codegen/tests/it/compile_shift.rs:24
// x := BYTE#16#0F: LOAD_CONST_I32 pool:0, TRUNC_U8, STORE_VAR_I32 var:0
```

### How common is it

Compiling a representative program (structure with `INT`/`BYTE`/`SINT` fields, a
function block with narrow locals, a `PROGRAM` with narrow scalar, struct-field
and array-element assignments, and a `FOR` loop) and scanning the resulting
`.iplc` for the four `TRUNC_*` opcodes: **18 of 19 occurrences are immediately
preceded by `LOAD_CONST_I32`.** The one survivor is `count := count + inc`, where
the value genuinely is computed at runtime.

That count is a raw byte scan over the whole container rather than a section-aware
disassembly, so treat it as indicative rather than exact. The direction is not in
doubt: struct-field initialization alone emits `LOAD_CONST; TRUNC; LOAD_CONST idx;
STORE_ARRAY` for *every* narrow field of *every* structure, and the value there is
always the type default (`0`) or a literal initializer.

### Prior art in this codebase

`for_loop_trunc_can_be_elided` (`codegen/src/compile_stmt.rs:1112`) already elides
the two `TRUNC` sites in a `FOR` loop header when the constant bounds provably stay
inside the control variable's range. It is documented in
[vm-performance.md §13, Layer 1](../design/vm-performance.md), which explicitly
names the remaining gap:

> The scope is the loop's own init and increment only; narrow stores in the loop
> body ... still truncate.

This change closes part of that gap — the constant-valued part — and is documented
in the same place.

## Architecture

### Where the fold goes: the peephole optimizer

`codegen/src/optimize.rs` already runs a post-emission pass over each function's
bytecode with access to the constant pool, and already removes
`LOAD_CONST(0); ADD`, `LOAD_CONST(1); MUL` and `LOAD_VAR x; STORE_VAR x`. It
decodes instructions, protects jump targets, rebuilds the stream and rewrites
branch offsets, and hands back an old→new offset map that `remap_line_map` uses to
fix debug line info. A `LOAD_CONST_I32; TRUNC_*` fold is the same shape of pattern
against the same machinery.

Doing it there rather than at the ~12 `emit_truncation` call sites means:

- **One implementation, every path.** Scalar stores, array elements, struct
  fields, deref stores, initializers, `emit_zero_const` defaults, subrange
  minimums, enum values and analyzer-folded constant expressions are all covered
  without touching any of them.
- **No duplicated constant extraction.** A source-level version would have to
  re-derive the value from `ConstantKind` for every literal kind that
  `compile_constant` handles, next to `compile_constant` but not inside it.
- **Correctness falls out of what is already there.** Jump-target protection,
  offset remapping and line-map snapping already exist and already have tests.
  `TRUNC_*` is 1-in/1-out on the stack, so `max_stack_depth` is untouched.

Two alternatives considered and rejected:

- *In `Emitter`* (alongside the existing `last_load`/`last_store` DUP peepholes).
  The emitter sees pool *indices*, not values; the pool lives in
  `CompileContext`. Wiring values through would mean changing the signature of
  `emit_load_const_i32` at ~30 call sites.
- *In `compile_constant`*, by widening `OpType` to carry `storage_bits`. `OpType`
  is threaded through every expression-compiling function in the crate; this is a
  large, high-risk refactor for a narrow benefit, and it still would not catch
  constants that reach `TRUNC` from `emit_zero_const` or subrange defaults.

### The pattern

For adjacent instructions `LOAD_CONST_I32 p` followed by `TRUNC_{I8,U8,I16,U16}`,
where neither is a jump target and pool entry `p` is a `PoolConstant::I32(v)`:

- compute `v' = trunc(op, v)` — `(v as i8) as i32`, `(v as u8) as i32`,
  `(v as i16) as i32`, `(v as u16) as i32`, matching `vm.rs` exactly;
- if `v' == v`, delete the `TRUNC_*` and leave the load alone;
- otherwise, intern `v'` in the constant pool and rewrite the load's operand to
  the new index, then delete the `TRUNC_*`.

The second case requires `optimize` to take `&mut Vec<PoolConstant>` instead of
`&[PoolConstant]`, which means `finalize_function` takes `&mut CompileContext`.
All five call sites (`compile.rs:833`, `compile.rs:843`, `compile_fn.rs:408`,
`compile_fn.rs:663`, `compile_method.rs:237`) already hold `ctx` mutably, and all
run before the pool is written to the container builder (`compile.rs:809`).

Notes on scope and known limits, all deliberate:

- **Only `LOAD_CONST_I32`.** `TRUNC_*` only accepts I32 operands, so no other
  load opcode can precede it with a foldable value. `LOAD_TRUE`/`LOAD_FALSE`
  never precede a `TRUNC` because `BOOL` has `storage_bits: 1`, for which
  `emit_truncation` is already a no-op.
- **`DUP` blocks the fold.** If the emitter's consecutive-load peephole already
  replaced the load with a `DUP`, the pattern does not match and the `TRUNC`
  stays. Correct, just not optimal; not worth chasing.
- **A rewritten constant can orphan its old pool entry.** `LOAD_CONST 200;
  TRUNC_I8` becomes `LOAD_CONST -56`, and `200` may no longer be referenced. The
  pool has no liveness pass today; this adds at most a few dead entries to the
  container. Not worth a pool-GC pass for this.
- **No cascade re-scan.** Removing a `TRUNC` could in principle expose a new
  adjacent pair for the existing identity patterns. It cannot in practice —
  `emit_truncation` only ever emits `TRUNC` immediately before a store or as the
  last step of a narrowing unary op, never before an arithmetic opcode — so the
  pass stays single-pass.

### Semantics

The VM is unconditionally wrapping (`bytecode-instruction-set.md`: "`TRUNC_*`
truncates by discarding high bits"; ADR-0002's configurable overflow policy is
not implemented). Folding therefore preserves behaviour exactly, including for
out-of-range constants such as `x : USINT := 300;`, which stores `44` before and
after this change.

Whether the *analyzer* should reject an out-of-range constant assigned to a
narrow type rather than silently wrapping it is a real question, but it is a
separate, user-visible change with its own problem code. Out of scope here; see
Follow-ups.

## Prefactoring

`optimize.rs` is 833 lines (315 of code, 517 of tests), and the module limit is
1000. Adding a fourth pattern plus its tests would cross it. Beyond size, the
current shape does not fit the new pattern: `is_removable_pair` returns `bool`,
and the driver marks *both* instructions of a matched pair as removed. This
change needs to remove *one* instruction and, in the out-of-range case, *rewrite
an operand* of the other.

The prefactoring, landed in its own commit with existing tests unchanged:

1. Split `optimize.rs` into `optimize/mod.rs` (decode, jump-target collection,
   rebuild, offset map, `remap_line_map`) and `optimize/patterns.rs` (pattern
   recognition and its unit tests). Each lands well under the limit and the
   feature then adds to one focused module.
2. Replace `is_removable_pair(a, b, constants) -> bool` with
   `match_pattern(a, b, constants) -> Option<[Action; 2]>` where
   `Action` is `Keep`, `Remove`, or `RewriteOperand(u16)`. The three existing
   patterns return `[Remove, Remove]`; the driver applies actions instead of
   assuming both are removed. Behaviour is identical and no test changes.

Step 2 is what turns the feature diff into "one more arm returning
`[Keep, Remove]` or `[RewriteOperand(i), Remove]`" instead of a second parallel
marking mechanism bolted next to the first.

## Design doc reference

[specs/design/vm-performance.md](../design/vm-performance.md) §13, Layer 1
("Abstract Interpretation with Richer Domains") holds the existing status note
for `for_loop_trunc_can_be_elided`. This change extends that note. No new REQ IDs
are needed — `vm-performance.md` is not in `codegen/build.rs`'s spec-listed docs,
and the `FOR`-loop elision precedent updated prose only.

[bytecode-instruction-set.md](../design/bytecode-instruction-set.md) documents
`TRUNC_*` semantics; nothing there changes, and no opcode is added, removed or
renumbered, so `wire_format.rs` is untouched.

## File map

**Prefactoring commit**

| File | Change |
|---|---|
| `compiler/codegen/src/optimize.rs` | Deleted; split into the two files below |
| `compiler/codegen/src/optimize/mod.rs` | New — decode, driver, offset map, `remap_line_map`, driver tests |
| `compiler/codegen/src/optimize/patterns.rs` | New — `Action`, `match_pattern`, constant predicates, opcode tables, pattern unit tests |

**Feature commit**

| File | Change |
|---|---|
| `compiler/codegen/src/optimize/patterns.rs` | Add `trunc_fold_value`; add the `LOAD_CONST_I32 + TRUNC_*` arm to `match_pattern` |
| `compiler/codegen/src/optimize/mod.rs` | `optimize` takes `&mut Vec<PoolConstant>`; apply `RewriteOperand` when rebuilding |
| `compiler/codegen/src/compile.rs` | `finalize_function` takes `&mut CompileContext` |
| `compiler/codegen/src/compile_fn.rs`, `compile_method.rs` | Pass `ctx` mutably (already `&mut` in scope) |
| `compiler/codegen/tests/it/compile_const_trunc.rs` | New — structural peephole tests |
| `compiler/codegen/tests/it/main.rs` | Register the new test module |
| `compiler/codegen/tests/it/compile_types.rs` | Repoint the two `TRUNC` tests at non-constant RHS (see below) |
| `compiler/codegen/tests/it/compile_shift.rs` | Update expected sequences |
| `compiler/codegen/tests/it/compile_array.rs` | Update `compile_when_array_sint_store_then_truncates` |
| `specs/design/vm-performance.md` | Extend the §13 Layer 1 status note |

### Existing tests that change, and why that is not a coverage loss

`compile_when_sint_then_produces_trunc_i8` and
`compile_when_uint_then_produces_trunc_u16` (`compile_types.rs`) exist to prove
that a store to a narrow variable narrows. Their bodies use a literal RHS, which
after this change no longer emits `TRUNC`. They will be rewritten to use a
runtime RHS (`x := y + 1;` with `y : SINT`), which still proves the original
claim, and the constant-folded form gets its own new tests in
`compile_const_trunc.rs`. Same for the `compile_shift.rs` and `compile_array.rs`
sequences. No assertion is deleted without a replacement covering the same claim.

## Tasks

- [ ] **Prefactor 1** — split `optimize.rs` into `optimize/{mod,patterns}.rs`;
      `cd compiler && just test` passes with no test edits
- [ ] **Prefactor 2** — introduce `Action` and `match_pattern`; driver applies
      actions; `cd compiler && just test` passes with no test edits
- [ ] Add `trunc_fold_value(op: u8, v: i32) -> Option<i32>` in `patterns.rs`,
      mirroring the four `TRUNC_*` arms of `vm.rs`
- [ ] Widen `optimize` / `finalize_function` to `&mut` pool and update the five
      call sites
- [ ] Add the `LOAD_CONST_I32 + TRUNC_*` arm: `[Keep, Remove]` when in range,
      `[RewriteOperand(interned), Remove]` otherwise
- [ ] Unit tests in `patterns.rs`: in-range fold for each of the four opcodes;
      out-of-range fold for each; non-I32 pool entry not folded; `TRUNC` that is
      a jump target not folded; `DUP; TRUNC` not folded
- [ ] Unit tests in `optimize/mod.rs`: offset map and a `JMP` across a folded
      pair remap correctly; line-map entry on a removed `TRUNC` snaps forward
- [ ] `compile_const_trunc.rs` — structural: `x : SINT; x := 42;` emits
      `LOAD_CONST_I32, STORE_VAR_I32, RET_VOID` with no `TRUNC`; struct-field
      init emits no `TRUNC`; array-element constant store emits no `TRUNC`;
      `count := count + inc` (`INT`) still emits `TRUNC_I16`
- [ ] Differential test (proptest, `codegen` dev-deps already include
      `ironplc-vm` and `proptest`): for arbitrary `i32` and each `TRUNC_*`,
      `trunc_fold_value` equals what the VM produces for
      `LOAD_CONST_I32 v; TRUNC_op`
- [ ] End-to-end tests: in-range and out-of-range constant stores to `SINT`,
      `USINT`, `INT`, `UINT`, `BYTE`, `WORD` produce the same variable values as
      before (`x : USINT := 300;` still reads back `44`)
- [ ] Update the existing `compile_types.rs` / `compile_shift.rs` /
      `compile_array.rs` assertions per the table above
- [ ] Update `specs/design/vm-performance.md` §13 Layer 1
- [ ] `cd compiler && just` — full CI, including the 85% coverage gate
- [ ] `git rm specs/plans/2026-08-30-constant-truncation-folding.md`

## Follow-ups to file as issues before the plan is deleted

1. **Out-of-range constant assigned to a narrow type is silently wrapped.**
   `x : USINT := 300;` compiles and stores `44` with no diagnostic. This change
   preserves that behaviour; whether the analyzer should reject it (a new `P####`
   with docs) is a separate user-visible decision.
2. **Pool liveness.** Operand rewriting can orphan a constant. A liveness pass
   over the pool before the container is built would shrink `.iplc` files
   slightly; worth doing only if pool size ever matters.
