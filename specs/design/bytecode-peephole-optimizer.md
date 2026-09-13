# Design: Bytecode Peephole Optimizer

## Overview

This design specifies the post-emission peephole optimizer in
`compiler/codegen/src/optimize/`. It runs an ordered sequence of passes over
the raw bytecode of one function, each matching a family of no-op or
identity patterns across two adjacent instructions and rewriting them away.

The optimizer's reason to exist is that some redundancy is only visible once
the whole instruction stream exists. The emitter has its own in-line
peepholes (consecutive load → `DUP`, store-load → `DUP`+`STORE`), but those
decide at the moment an instruction is pushed, from one instruction of
lookbehind and no view of the constant pool's values. A `TRUNC_*` that
follows a constant load, or a multiply by a pool entry that happens to hold
`1`, needs the finished stream and the pool together.

The design builds on:

- **[ADR-0001](../adrs/0001-bytecode-integer-arithmetic-type-strategy.md)**:
  sub-32-bit integers are promoted to 32 bits on load and narrowed by an
  explicit `TRUNC_*` before every store back to a narrow slot. This is what
  makes the constant-truncation pass worth having.
- **[Bytecode Instruction Set](bytecode-instruction-set.md)**: opcode
  encodings, `TRUNC_*` semantics, and `opcode::instruction_size` — the single
  source of truth for instruction lengths that the optimizer decodes with.
- **[Debugger Support](debugger-support.md)**: the debug-info side of the
  offset-map contract below (breakpoint resolution, stepping).

## Design Goals

1. **Semantics preserved exactly.** A pass may only delete or rewrite what
   the VM would have computed to the same value. No pass is gated on an
   optimization level, so a rewrite that is wrong for any input is wrong
   everywhere.
2. **One rewrite engine.** Decoding, jump-target protection, stream rebuild
   and offset bookkeeping are written once; a pass contributes only its
   decision about a two-instruction window.
3. **Branch offsets stay the emitter's business.** The optimizer runs before
   backpatching, so it never reads or writes an encoded branch operand.
4. **Debug info survives.** Every pass reports where each instruction moved,
   in a form the line map can be remapped through without losing the
   instruction-boundary invariant the debugger depends on.

## Scope

**In scope:** adjacent-pair patterns within a single function's bytecode,
resolved during code generation.

**Out of scope:** anything needing control- or data-flow analysis (dead-store
elimination, common-subexpression elimination, range tracking across
expressions), any rewrite spanning more than two adjacent instructions, and
constant folding of literal-op-literal expressions — the analyzer's
`xform_fold_constant_expressions` has already done that before codegen sees
the program.

---

## 1. Position in the Pipeline

The optimizer has exactly one entry point, `optimize`, called from
`finalize_function` (`codegen/src/compile.rs`), which every path that emits a
function goes through — init, scan, user functions, function-block bodies and
methods.

```
emitter emits instructions          branch operands are placeholders
        │
        ├─ take_line_map()          raw per-statement source positions
        ↓
optimize(unpatched_code, &mut constants)
        │                           passes run in order; pool may grow
        ↓  (bytecode, offset_map)
emitter.apply_optimized(…)          jumps patched against the new offsets
        ↓
remap_line_map(raw, offset_map, …)  debug info moved onto surviving offsets
```

**REQ-PEEP-codegen-001** No pass removes or rewrites an instruction whose
start offset is one of the function's jump targets. Those offsets are
supplied by the emitter in `UnpatchedCode::jump_targets`; protecting them
preserves basic-block boundaries and guarantees every jump target still maps
to a valid instruction start afterwards.

Because the bytecode arrives unpatched, no pass has to know which opcodes
carry a branch offset or where in the instruction it sits. The optimizer is
told which offsets are targeted and reports where everything moved; the
emitter resolves each jump afterwards.

`TRUNC_*` and the identity pairs are stack-neutral or stack-shrinking, so
`max_stack_depth` — computed by the emitter before optimization — remains a
valid upper bound and is not recomputed.

## 2. The Offset Map

Every pass returns an old→new offset map alongside its rewritten bytes. This
map is the whole interface between the optimizer and everything that holds an
offset into the function: the emitter's labels and pending patches, and the
debug line map.

**REQ-PEEP-codegen-002** The offset map maps every instruction start offset
in the pass's input, plus the one-past-the-end offset, to an offset in its
output. The end position is included because a label may be bound past the
last instruction — the shape an `IF` with no `ELSE` produces — where there is
no instruction to look up.

**REQ-PEEP-codegen-003** A removed instruction's offset maps to the offset
the next surviving instruction occupies ("snap forward"), so an offset that
lands on a removed instruction advances rather than dangling.

**REQ-PEEP-codegen-004** Running several passes composes their maps into one
map from original offsets to final offsets, preserving both properties above.
Composition is total: every value in the accumulated map is an offset the
next pass's input has an instruction boundary at (or its length), because
both are accumulated from surviving instruction sizes.

Line-map remapping (`remap_line_map`) is the one consumer that can discard
entries, and both cases are legitimate rather than errors: an entry whose
remapped offset lands at or past the new end of the function has no
instruction left to attribute a source position to, and an entry landing on
the same offset as the previous kept entry collapses, because a line map
holds one position per offset.

**REQ-PEEP-codegen-005** A line-map entry whose offset is absent from the map
is reported as an internal error (`P9998`), not dropped. The map covers every
instruction boundary, and the emitter records an entry only immediately
before pushing an opcode, so a miss means an entry sits mid-instruction —
a compiler defect, not a property of the program being compiled.

## 3. The Rewrite Engine

`optimize/rewrite.rs` holds the machinery every pass shares. A pass supplies
one closure, asked about each adjacent instruction pair in turn, returning
either nothing or one action per instruction of the pair:

| Action | Meaning |
|---|---|
| `Keep` | Leave the instruction exactly as it is. |
| `Remove` | Drop the instruction from the output. |
| `RewriteOperand(u16)` | Keep the instruction, replace its `u16` operand. |

**REQ-PEEP-codegen-006** `RewriteOperand` is valid only on a three-byte
opcode-plus-`u16` instruction and leaves the encoded size unchanged, so
offsets after it are unaffected and only `Remove` moves anything.

Instructions are decoded with `opcode::instruction_size`, so an opcode's
length is never restated here. A matched pair advances the scan past both
instructions; an unmatched one advances by a single instruction, so the
second instruction of a non-match is still offered as the first of the next
pair.

### Why this shape

Three properties of the pattern set drove it:

- **A pass may rewrite rather than delete.** The constant-truncation fold
  removes *one* instruction of its pair and, in the out-of-range case,
  rewrites an *operand* of the other. A single "is this pair removable?"
  predicate with a remove-both driver has no vocabulary for either.
- **Passes need different inputs.** Only the truncation fold wants the
  constant pool mutably; the self-assignment pass does not want the pool at
  all. One shared widened signature would hand every pattern write access to
  the pool for the benefit of one.
- **Independence is a stated property, not an emergent one.** With one
  interleaved scan, "can these rewrites interfere?" is answerable only by
  reading the scan. A named, ordered list makes it reviewable.

## 4. Pass Order

**REQ-PEEP-codegen-007** The registered passes match on disjoint opcode
pairs, so the result does not depend on the order they run in.

The order is nonetheless fixed and named in one place, so that a pass whose
output feeds another has an obvious place to say so — and an obvious place to
add a fixed-point loop if one is ever needed. Nothing needs one today, and no
cascade is currently reachable:

- A cascade would need something like `LOAD_CONST a; LOAD_CONST 1; MUL;
  TRUNC` — an identity removal leaving a constant adjacent to a `TRUNC`. The
  analyzer folds literal-op-literal long before codegen, so `x := 5 * 1`
  reaches the optimizer as `LOAD_CONST 5`.
- A named `VAR CONSTANT` does not close the gap either: `x := 5 * ONE`
  compiles to `LOAD_CONST 5; LOAD_VAR ONE; MUL_I32; TRUNC_I8`, where `ONE` is
  a variable load, so the identity pass never fires.

The identity passes fire in practice on index arithmetic (`LOAD_CONST_I64 0;
ADD_I64` from a zero structure-field offset), which is I64 and never precedes
a `TRUNC_*`.

The constant-truncation pass runs last because it is the only pass that
appends to the constant pool, so nothing after it can read a stale pool.

## 5. Pass: Self-Assignment

`LOAD_VAR x; STORE_VAR x` loads a variable and immediately stores it back to
the same slot at the same width, leaving the variable table unchanged.

**REQ-PEEP-codegen-020** Both instructions are removed when the load and
store name the same variable index at the same width (`I32`, `I64`, `F32`,
`F64`); a differing index or a mismatched width leaves the pair in place.

The width must match because a load and store of different widths against one
slot is a reinterpretation, not a round trip.

## 6. Pass: Arithmetic Identity

A constant operand loaded immediately before an arithmetic opcode, where the
operation is the identity for that value.

**REQ-PEEP-codegen-030** Both instructions are removed for
`LOAD_CONST 0; ADD|SUB` on integers, `LOAD_CONST 0.0; SUB` on floats, and
`LOAD_CONST 1; MUL|DIV` on both, where the load's width matches the
operator's.

Only the *second* operand can be folded this way. A constant loaded first is
never adjacent to the operator (`LOAD_CONST; LOAD_VAR; SUB`), which is what
keeps `0 - x` — negation, not an identity — out of reach of the pass.

**REQ-PEEP-codegen-031** `LOAD_CONST 0.0; ADD` on `F32`/`F64` is *not*
removed. Under IEEE 754 §6.3 the sum of two zeros of opposite sign is `+0.0`,
so `(-0.0) + 0.0 = +0.0` and removing the add would leave `-0.0` on the
stack. The sign of zero is observable: `1.0 / y` is `+inf` for one and `-inf`
for the other. `x - 0.0` is safe — `(-0.0) - 0.0 = -0.0` — as are `x * 1.0`
and `x / 1.0`, which preserve sign, magnitude, NaN payload and infinities
alike.

A zero float constant is matched whether it is `+0.0` or `-0.0`, since the
only additive use of one is `SUB` and `x - (-0.0)` is `x` just as
`x - 0.0` is.

## 7. Pass: Constant Truncation

### Why the instruction is there to remove

Under ADR-0001, `emit_truncation` emits a `TRUNC_*` before every store to a
sub-32-bit slot. It cannot tell whether the value on the stack came from a
constant: it is handed an `OpType` — a width and a signedness, which collapses
`SINT`/`INT`/`DINT` alike to `(W32, Signed)` — while the `storage_bits` that
select the truncation width live one level up in `VarTypeInfo`. So a narrow
constant store emits the constant at 32 bits and narrows it with an
instruction the compiler could have evaluated itself.

That is the common case, not an edge case. On a representative program — a
structure with `INT`/`BYTE`/`SINT` fields, a function block with narrow
locals, and a `PROGRAM` with narrow scalar, structure-field and array-element
assignments plus a `FOR` loop — 16 of 17 `TRUNC_*` instructions were
immediately preceded by a constant load, and all 16 were already in range.
The survivor was `arr[i] := i` inside the loop, where the value genuinely is
computed at run time. Structure-field initialization alone emits a constant
load per narrow field of every structure, where the value is always the type
default or a literal initializer.

### The fold

For `LOAD_CONST_I32 p` immediately followed by a `TRUNC_*`, where pool entry
`p` holds a `PoolConstant::I32`:

**REQ-PEEP-codegen-040** When the constant already equals its truncation, the
`TRUNC_*` is removed and the load is left untouched.

**REQ-PEEP-codegen-041** Otherwise the truncated value is interned in the
constant pool — reusing an existing entry when one holds that value — the
load's operand is rewritten to that index, and the `TRUNC_*` is removed.

**REQ-PEEP-codegen-042** The folded value equals what the VM's `TRUNC_*`
would have produced: `TRUNC_I8` is `(v as i8) as i32`, `TRUNC_U8` is
`(v as u8) as i32`, `TRUNC_I16` is `(v as i16) as i32`, and `TRUNC_U16` is
`(v as u16) as i32`.

The VM is unconditionally wrapping, so folding preserves behaviour exactly,
including for a constant outside its target's range.

Which programs can still reach the out-of-range case is a separate matter
from whether the fold handles it. The analyzer rejects an out-of-range constant
assigned to a numeric narrow type — `x : USINT := 300` is `P2026` — so for
`SINT`/`USINT`/`INT`/`UINT` the out-of-range branch is unreachable from a
program that compiles. The bit-string types are the exception: `BYTE` and
`WORD` are bit patterns rather than numbers and are deliberately not
range-checked, so `x := BYTE#300` compiles and the fold interns `44`, which
is what the run-time `TRUNC_U8` produced before. The branch is therefore
still live, and it would be load-bearing again for any future type that
wraps by design.

Because the pass matches on the emitted instruction stream rather than on any
one syntactic form, it reaches every narrow store whose value is a constant —
scalar assignment, array element, structure field, dereference store,
initializers, zero defaults, subrange minimums, enumeration values and
analyzer-folded constant expressions — without any of the dozen
`emit_truncation` call sites knowing about it.

### Deliberate limits

**REQ-PEEP-codegen-043** A `TRUNC_*` not immediately preceded by
`LOAD_CONST_I32` is left in place. In particular a `DUP` left by the
emitter's consecutive-load peephole hides the value from this pass, so
`x := n + n` into a narrow slot keeps its `TRUNC_*`. Correct, just not
optimal.

Only `LOAD_CONST_I32` can precede a `TRUNC_*` with a foldable value:
`TRUNC_*` takes an I32 operand, and `BOOL` — the one type reached by
`LOAD_TRUE`/`LOAD_FALSE` — has `storage_bits: 1`, for which
`emit_truncation` already emits nothing.

**REQ-PEEP-codegen-044** A pool index that is out of bounds, or that names a
constant of another type, is left unfolded rather than diagnosed. This pass is
not the place to report a malformed pool reference.

Rewriting an operand can leave the original constant unreferenced —
for `x := BYTE#300`, `LOAD_CONST 300; TRUNC_U8` becomes `LOAD_CONST 44`, and
`300` may no longer be used by any instruction. There is no pool liveness pass, so those
dead entries reach the container; the cost is a handful of bytes on the
programs that can trigger it at all, tracked separately in
[#1529](https://github.com/ironplc/ironplc/issues/1529).

### Why the optimizer rather than the emission sites

Three other homes were considered:

- **At the `emit_truncation` call sites.** A dozen sites would each need the
  same check, and none of them would catch a constant that reaches a
  `TRUNC_*` from a zero default or a subrange minimum.
- **In `compile_constant`, by widening `OpType` to carry `storage_bits`.**
  `OpType` is threaded through every expression-compiling function in the
  crate, so this is a large, high-risk refactor for a narrow benefit — and it
  still would not catch the non-expression constants above.
- **In `Emitter`, alongside the existing `DUP` peepholes.** The emitter sees
  pool *indices*, not values; the pool lives in `CompileContext`.

## 8. What Still Truncates at Run Time

A narrow store of a *computed* value (`total := total + i` where
`total : INT`) still executes a `TRUNC_*`, because proving it unnecessary
needs range tracking across arbitrary expressions rather than a single
instruction pair. `for_loop_trunc_can_be_elided`
(`codegen/src/compile_stmt.rs`) covers one further case — a `FOR` loop whose
constant bounds provably keep the control variable inside its declared range
— for the loop's own init and increment only.

Both are narrow, local instances of the interval analysis sketched under
[VM Performance §13, Layer 1](vm-performance.md), which is where the general
case is tracked.
