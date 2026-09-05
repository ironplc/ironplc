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
and array-element assignments, and a `FOR` loop) and disassembling the resulting
`.iplc` via `ironplc_project::disassemble`: **16 of 17 `TRUNC_*` instructions are
immediately preceded by `LOAD_CONST_I32`.** The single survivor is `arr[i] := i`
inside the loop, where the value genuinely is computed at runtime.

Struct-field initialization alone emits `LOAD_CONST; TRUNC; LOAD_CONST idx;
STORE_ARRAY` for *every* narrow field of *every* structure, and the value there is
always the type default (`0`) or a literal initializer.

**All 16 are already in range**, i.e. pure deletions needing no new pool entry.
That is expected: a constant lands outside its target's narrow range only when
the source says something like `x : USINT := 300;`, which is arguably a program
bug (see Follow-ups). The value-rewriting half of this change is therefore the
rare case, not the common one — it is included because leaving it out would mean
`TRUNC` survives in exactly the situation where its result is least obvious to
the reader, but it is not where the win comes from.

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
fix debug line info. A `LOAD_CONST_I32; TRUNC_*` fold is the same shape of work
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

### The fold itself

For adjacent instructions `LOAD_CONST_I32 p` followed by `TRUNC_{I8,U8,I16,U16}`,
where neither is a jump target and pool entry `p` is a `PoolConstant::I32(v)`:

- compute `v' = trunc(op, v)` — `(v as i8) as i32`, `(v as u8) as i32`,
  `(v as i16) as i32`, `(v as u16) as i32`, matching `vm.rs` exactly;
- if `v' == v`, delete the `TRUNC_*` and leave the load alone;
- otherwise, intern `v'` in the constant pool and rewrite the load's operand to
  the new index, then delete the `TRUNC_*`.

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

**Turn the optimizer into an ordered pipeline of named passes, modelled on
`analyzer/src/stages.rs`.**

### Why the current shape does not fit

`optimize.rs` is one function holding three unrelated rewrites behind a single
`is_removable_pair(a, b, constants) -> bool` predicate, with a driver that marks
*both* instructions of a matched pair as removed. Adding the fold to that shape
means:

- a fourth `if` inside the shared predicate, with no way to say when it runs;
- a second marking mechanism next to the first, because this rewrite removes
  *one* instruction and, in the out-of-range case, *rewrites an operand* of the
  other — neither of which the `bool`/remove-both driver can express;
- crossing the 1000-line module limit (833 today: 315 code, 517 tests).

That is three of the steering file's prefactoring signals at once.

### Ordering: currently free, and worth making explicit rather than accidental

Being honest about this, because it was worth checking before designing around
it: **no pass currently exposes work for another.** Two things were verified
against the compiler rather than assumed:

- A cascade would need `LOAD_CONST a; LOAD_CONST 1; MUL; TRUNC` — an identity
  removal leaving a constant adjacent to a `TRUNC`. It cannot arise.
  `xform_fold_constant_expressions` folds literal-op-literal in the analyzer
  long before codegen, so `x := 5 * 1` reaches the optimizer as `LOAD_CONST 5`.
- A named `VAR CONSTANT` does not close the gap either: `x := 5 * ONE` compiles
  to `LOAD_CONST 5; LOAD_VAR ONE; MUL_I32; TRUNC_I8` — `ONE` is a variable load,
  so the identity pass never fires on it.

The identity passes in practice fire on index arithmetic (`LOAD_CONST_I64 0;
ADD_I64` from a zero struct-field offset), which is I64 and never precedes a
`TRUNC`. So the three existing patterns and the new one match on disjoint
opcode pairs, and any pass order produces the same bytecode today.

The pipeline is still the right shape, for reasons that do not depend on a
cascade existing:

- **The new rewrite cannot be expressed by the current driver at all.** It
  removes *one* instruction of the pair and, in the out-of-range case, rewrites
  an *operand* of the other. `is_removable_pair -> bool` plus a remove-both
  driver has no vocabulary for either.
- **Passes need different inputs.** The truncation fold is the only one wanting
  the constant pool mutably; the self-assignment pass does not want the pool at
  all. The analyzer's transforms already work this way — some take
  `&mut TypeEnvironment`, some do not, and `stages.rs` wires each up
  individually. One shared widened signature would give every pattern write
  access to the pool for the benefit of one.
- **Independence becomes a stated property instead of an emergent one.** Right
  now "can these rewrites interfere?" is answerable only by reading one
  interleaved scan. A named, ordered list with the reasoning in comments — again
  as `stages.rs` does — makes it reviewable, and gives one place to add a
  fixed-point loop if a future pass ever does re-enable an earlier one. Nothing
  today does, so the loop is not built.
- **Module size.** 833 lines against a 1000-line limit, before adding a pattern
  and its tests.

### The shape

`optimize/mod.rs` becomes the driver: it owns the ordered list of passes, threads
the bytecode through them, and composes their offset maps. Shared rewrite
machinery moves to `optimize/rewrite.rs`, so each pass supplies only its decision
about a two-instruction window:

```rust
pub(super) enum Action { Keep, Remove, RewriteOperand(u16) }

pub(super) fn apply_peephole(
    bytecode: &[u8],
    matcher: impl FnMut(&Instruction, &Instruction) -> Option<[Action; 2]>,
) -> (Vec<u8>, OffsetMap)
```

`apply_peephole` keeps everything the current driver does — decode, jump-target
protection, offset-map construction, `JMP`/`JMP_IF_NOT`/`CMP_BR` offset rewriting
— and is written once. The three existing patterns become passes that return
`[Remove, Remove]`; the new one returns `[Keep, Remove]` or
`[RewriteOperand(i), Remove]`.

Composing offset maps preserves the property `remap_line_map` depends on: each
map sends every instruction boundary — removed ones included — to the offset of
the next surviving instruction, and composing two such maps still does. That
needs an explicit test.

### Commits

1. **Split into passes.** Create `optimize/{mod,rewrite}.rs` plus
   `optimize/pass_self_assign.rs` (`LOAD_VAR x; STORE_VAR x`) and
   `optimize/pass_arith_identity.rs` (`LOAD_CONST 0; ADD|SUB` and
   `LOAD_CONST 1; MUL|DIV` — one module, one concern). Introduce `Action` and
   `apply_peephole`. Pass unit tests move to their pass module; driver and
   jump/offset tests stay with the driver and the rewrite engine. Signatures keep
   `&[PoolConstant]`. Existing tests pass unchanged apart from moving.
2. **Widen the pool to `&mut`.** `optimize` and `finalize_function` take
   `&mut Vec<PoolConstant>` / `&mut CompileContext`. Mechanical, no behaviour
   change; all five `finalize_function` call sites (`compile.rs:833`,
   `compile.rs:843`, `compile_fn.rs:408`, `compile_fn.rs:663`,
   `compile_method.rs:237`) already hold `ctx` mutably, and all run before the
   pool is written to the container builder (`compile.rs:809`).
3. **Add the feature** as `optimize/pass_const_trunc.rs`, registered last in the
   driver.

The three existing patterns match on disjoint opcode pairs, so splitting them
into separately-scanned passes cannot change their combined result; the 45
existing `optimize` tests assert exact output bytecode and are the check on that.

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

**Commit 1 — split into passes**

| File | Change |
|---|---|
| `compiler/codegen/src/optimize.rs` | Deleted; split into the files below |
| `compiler/codegen/src/optimize/mod.rs` | New — pass list, offset-map composition, `remap_line_map`, driver tests |
| `compiler/codegen/src/optimize/rewrite.rs` | New — `Instruction`, `decode`, `Action`, `apply_peephole`, branch fixup, and their tests |
| `compiler/codegen/src/optimize/pass_self_assign.rs` | New — `LOAD_VAR x; STORE_VAR x` + its tests |
| `compiler/codegen/src/optimize/pass_arith_identity.rs` | New — additive and multiplicative identities + their tests |

**Commit 2 — mutable pool**

| File | Change |
|---|---|
| `compiler/codegen/src/optimize/mod.rs` | `optimize` takes `&mut Vec<PoolConstant>` |
| `compiler/codegen/src/compile.rs` | `finalize_function` takes `&mut CompileContext` |
| `compiler/codegen/src/compile_fn.rs`, `compile_method.rs` | Pass `ctx` mutably |

**Commit 3 — the fold**

| File | Change |
|---|---|
| `compiler/codegen/src/optimize/pass_const_trunc.rs` | New — `trunc_fold_value`, the matcher, and their tests |
| `compiler/codegen/src/optimize/mod.rs` | Register the pass last, with the ordering comment |
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

- [ ] **Commit 1** — create `optimize/{mod,rewrite}.rs`, `pass_self_assign.rs`,
      `pass_arith_identity.rs`; introduce `Action` + `apply_peephole`; move the
      existing tests to their owning modules
- [ ] Add driver tests for map composition: a `JMP` spanning instructions removed
      by two different passes remaps correctly; a line-map entry on an
      instruction removed in pass 1 snaps forward past a pass-2 removal
- [ ] `cd compiler && just test` — passes with no test assertions changed
- [ ] **Commit 2** — widen `optimize` / `finalize_function` to `&mut`; update the
      five call sites; `just test` still green
- [ ] **Commit 3** — add `trunc_fold_value(op: u8, v: i32) -> Option<i32>` in
      `pass_const_trunc.rs`, mirroring the four `TRUNC_*` arms of `vm.rs`
- [ ] Add the pass matcher: `[Keep, Remove]` when in range,
      `[RewriteOperand(interned), Remove]` otherwise; register it last in the
      driver with the ordering comment
- [ ] Pass unit tests: in-range fold for each of the four opcodes; out-of-range
      fold for each; non-I32 pool entry not folded; out-of-bounds pool index not
      folded; `TRUNC` that is a jump target not folded; `DUP; TRUNC` not folded
- [ ] Driver test pinning pass independence: a stream containing all four
      patterns optimizes to the same bytecode whichever order the passes run in
- [ ] Differential test (proptest; `codegen` dev-deps already include
      `ironplc-vm` and `proptest`): for arbitrary `i32` and each `TRUNC_*`,
      `trunc_fold_value` equals what the VM produces for
      `LOAD_CONST_I32 v; TRUNC_op`
- [ ] `compile_const_trunc.rs` — structural: `x : SINT; x := 42;` emits
      `LOAD_CONST_I32, STORE_VAR_I32, RET_VOID` with no `TRUNC`; struct-field
      init emits no `TRUNC`; array-element constant store emits no `TRUNC`;
      `count := count + inc` (`INT`) still emits `TRUNC_I16`
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
