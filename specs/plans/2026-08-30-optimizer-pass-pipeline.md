# Optimizer pass pipeline (prefactor)

Slice 1 of [#1509](https://github.com/ironplc/ironplc/issues/1509).

## Goal

Restructure `codegen/src/optimize.rs` from one function holding three
interleaved rewrites into an ordered pipeline of named passes over shared
rewrite machinery, modelled on `analyzer/src/stages.rs`.

Behaviour-preserving: no bytecode changes, no test assertions change.

## Why

Slice 2 adds a fourth rewrite — folding a `TRUNC_*` that follows a constant
load — which the current shape cannot express:

- `is_removable_pair(a, b, constants) -> bool` feeds a driver that marks *both*
  instructions of a matched pair as removed. The new rewrite removes *one*
  instruction and, when the constant is out of range, rewrites an *operand* of
  the other.
- The new rewrite is the only one that needs the constant pool mutably (to
  intern a truncated value); the self-assignment rewrite does not need the pool
  at all. One shared predicate signature means widening it for every pattern.
- `optimize.rs` is 833 lines (315 code, 517 tests) against the project's
  1000-line module limit. A fourth pattern plus its tests crosses it.

Three of the steering file's prefactoring signals at once.

### Ordering

Worth stating explicitly, because it was checked rather than assumed: **no pass
currently exposes work for another**, so any order produces the same bytecode
today.

A cascade would need an identity removal to leave a constant load adjacent to a
`TRUNC` — `LOAD_CONST a; LOAD_CONST 1; MUL; TRUNC`. That cannot arise:
`xform_fold_constant_expressions` folds literal-op-literal in the analyzer long
before codegen, so `x := 5 * 1` reaches the optimizer as `LOAD_CONST 5`; and a
named `VAR CONSTANT` compiles to a variable load, so `x := 5 * ONE` becomes
`LOAD_CONST 5; LOAD_VAR ONE; MUL_I32; TRUNC_I8` with nothing for the identity
pass to remove. In practice the identity passes fire on I64 index arithmetic
(`LOAD_CONST_I64 0; ADD_I64` from a zero struct-field offset), which never
precedes a `TRUNC`.

The value of the pipeline is that this becomes a *stated* property with one
obvious place to record ordering constraints — as `stages.rs` does for the
`xform_*` sequence — instead of an emergent property of a single interleaved
scan. No fixed-point loop is built, because nothing needs one.

## Architecture

```
optimize/mod.rs                 driver: pass sequence, offset-map composition,
                                remap_line_map, pub(crate) optimize
optimize/rewrite.rs             shared machinery: Instruction, decode,
                                jump-target collection, apply_peephole,
                                branch-offset rewriting
optimize/pass_self_assign.rs    LOAD_VAR x; STORE_VAR x
optimize/pass_arith_identity.rs LOAD_CONST 0; ADD|SUB  and  LOAD_CONST 1; MUL|DIV
optimize/tests.rs               the existing test suite, unchanged
```

`apply_peephole` takes the bytecode and a matcher over a two-instruction window,
and keeps everything the current driver does — decode, jump-target protection,
offset-map construction, `JMP`/`JMP_IF_NOT`/`CMP_BR` offset rewriting.

Each pass declares its own parameters, as the analyzer's transforms do
(`xform_resolve_adr::apply(library, options)` vs
`xform_int_to_bool_initializer::apply(library, &mut type_environment, options)`):
`pass_self_assign::apply(bytecode)` takes no pool, `pass_arith_identity::apply(
bytecode, constants)` takes it by shared reference.

### Offset-map composition

Each pass returns an old→new map covering every instruction boundary in its
input plus one-past-the-end, sending removed instructions forward to the next
surviving instruction. The driver folds pass maps together as
`composed[old] = pass_map[prev[old]]`.

The composition is total: `prev`'s values are accumulated from surviving
instruction sizes, so every one of them is an instruction boundary in the
intermediate stream (or its length), which is exactly the domain the next pass's
map covers. Composition also preserves the snap-forward property that
`remap_line_map` depends on.

### Deviation from the slice-2 plan

The `Action { Keep, Remove, RewriteOperand(u16) }` vocabulary is **not**
introduced here. All three existing passes remove both instructions, so `Keep`
and `RewriteOperand` would be unconstructed variants — dead code that fails
`just lint`, and speculative generality in a commit that adds no behaviour.
`apply_peephole`'s matcher returns `bool` in this slice; slice 2 widens it to
`Option<[Action; 2]>` where the variants are actually used.

### Tests

The existing tests all call `optimize(...)` — they pin driver-level observable
behaviour, which is precisely what must not change. They move verbatim into
`optimize/tests.rs` (only the `use` lines change, from `use super::*` to
explicit imports). Rewriting them to call individual passes would forfeit the
property that makes a prefactor reviewable.

Two tests are added for the one genuinely new mechanism, offset-map composition,
which no existing test covers because there was only ever one map.

## Prefactoring

This slice *is* the prefactoring.

## Design doc reference

None. The optimizer has no design document; `specs/design/vm-performance.md` §13
Layer 1 records the related `for_loop_trunc_can_be_elided` work and is updated in
slice 2, not here.

## File map

| File | Change |
|---|---|
| `compiler/codegen/src/optimize.rs` | Deleted; split into the files below |
| `compiler/codegen/src/optimize/mod.rs` | New — driver, composition, `remap_line_map` |
| `compiler/codegen/src/optimize/rewrite.rs` | New — `Instruction`, `decode`, `apply_peephole`, branch fixup |
| `compiler/codegen/src/optimize/pass_self_assign.rs` | New |
| `compiler/codegen/src/optimize/pass_arith_identity.rs` | New |
| `compiler/codegen/src/optimize/tests.rs` | New — existing suite, assertions unchanged |

## Tasks

- [ ] Create `optimize/rewrite.rs` with `Instruction`, `decode`, `is_cmp_br`,
      `CMP_BR_SIZE` and `apply_peephole`
- [ ] Create `pass_self_assign.rs` and `pass_arith_identity.rs`
- [ ] Create `optimize/mod.rs` with the pass sequence, composition and
      `remap_line_map`
- [ ] Move the existing tests to `optimize/tests.rs` with assertions unchanged
- [ ] Add composition tests: a `JMP` spanning instructions removed by two
      different passes remaps correctly; a line-map entry on an instruction
      removed by the first pass snaps forward past a second-pass removal
- [ ] `cd compiler && just` — full CI including the 85% coverage gate
- [ ] `git rm specs/plans/2026-08-30-optimizer-pass-pipeline.md`
