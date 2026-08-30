# Run the peephole optimizer before jump patching

Implements [issue #1530](https://github.com/ironplc/ironplc/issues/1530).

## Goal

Run the peephole optimizer on *un-patched* bytecode, so it never sees an
encoded branch offset. Today the emitter resolves every jump to a relative
`i16`, and the optimizer immediately runs that arithmetic backwards to
recover the symbolic target it was handed. Removing that round trip drops
all knowledge of which opcodes carry a branch offset — and where in the
instruction it sits — from `codegen/src/optimize/`.

Adding a new branch opcode then needs no optimizer change at all.

## Architecture

The emitter already holds the symbolic form: `labels` (bound positions) and
`patches` (which instruction operand targets which label). Instead of
patching and then decoding, the emitter hands the optimizer the raw bytes
plus the set of positions its jumps will land on, and remaps its own
bookkeeping afterwards.

```
finalize_function:
    emitter.unpatched_code()       -> UnpatchedCode { bytecode, jump_targets }
    optimize(...)                  -> (bytecode, offset_map)
    emitter.apply_optimized(...)   -- swap in bytes, remap labels + patches
    emitter.bytecode()             -- patch_jumps() over the new positions
```

Two consequences, both called out in the issue:

- **`patch_offset` is not an instruction boundary.** It is `start + 1` for
  `JMP`/`JMP_IF_NOT` and `start + 6` for `CMP_BR_*`, while the offset map
  only covers boundaries. `PendingPatch` therefore carries the instruction
  start plus the operand's delta within it, and derives `patch_offset` from
  the two. That moves "where does the branch operand sit" into the emitter,
  which is the code that writes those bytes — the right home for it.

- **The protected set comes from `patches`, not `labels`.** Resolving each
  `PendingPatch` to its label's position yields exactly today's set by
  construction. Protecting every *bound* label instead would newly protect a
  bound-but-unreferenced label and change what the optimizer may remove.

## Prefactoring

`apply_peephole` currently derives the protected set inside `decode()` and
consumes it in the same function. The first commit makes that set an
explicit input threaded through `optimize` -> `Pipeline` -> each pass ->
`apply_peephole`, still derived from the encoded offsets by a `decode`-local
helper. No behaviour change. The second commit then only has to swap where
the set comes from and delete the derivation, rather than doing both at once.

## Design doc reference

None — `specs/design/vm-performance.md` describes the passes, not the
pipeline's position relative to patching. No design content changes.

## File map

- `compiler/codegen/src/optimize/rewrite.rs` — take the protected set as a
  parameter; drop `is_cmp_br`, `CMP_BR_SIZE`, the target-recovery arithmetic
  in `decode()`, and the two branch-rewriting arms of the rebuild loop
- `compiler/codegen/src/optimize/mod.rs` — thread and remap the protected set
  through `Pipeline`; `optimize` takes `UnpatchedCode`
- `compiler/codegen/src/optimize/pass_*.rs` — pass the set through
- `compiler/codegen/src/emit.rs` — `UnpatchedCode`, `unpatched_code()`,
  `apply_optimized()`; `PendingPatch` carries the instruction start
- `compiler/codegen/src/compile.rs` — `finalize_function` runs the optimizer
  before `bytecode()`
- `compiler/codegen/src/optimize/tests.rs` — protection tests supply the
  target set directly; offset-rewriting tests move to the emitter pipeline

## Tasks

- [ ] Prefactor: make the protected offset set an explicit input to
      `apply_peephole` and each pass, threaded and remapped by `Pipeline`
- [ ] `PendingPatch` carries the instruction start and the operand delta
- [ ] Add `UnpatchedCode` and `Emitter::unpatched_code()`
- [ ] Add `Emitter::apply_optimized()` to remap labels and patches
- [ ] `finalize_function` optimizes before patching
- [ ] Delete branch decoding and rewriting from `rewrite.rs`
- [ ] Rework the optimizer's jump tests; add pipeline tests that assert the
      final patched offsets
- [ ] `cd compiler && just`
