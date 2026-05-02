# Plan: `CMP_BR` Compare-and-Branch Superinstruction (with WHILE → do-while)

## Context

`vm-performance.md` §11 ("Specialized Comparison-Branch Opcodes")
identifies a single broad lever for control-flow performance: PLC
programs are dominated by `var <cmp> const` patterns feeding directly
into a conditional branch (FOR head test, WHILE/REPEAT conditions,
IF/ELSIF, CASE selectors). Today every such site costs four
dispatches:

```text
LOAD_VAR_I32   var
LOAD_CONST_I32 const
LE_I32                     ; or LT/EQ/NE/GT/GE
JMP_IF_NOT     target
```

This plan supersedes the FOR-only `FOR_NEXT_I32` proposal: instead of
spending an opcode slot on a FOR-specific increment-and-branch fusion,
we add a single op-class — `CMP_BR` — that fuses load-compare-branch
across **all** loop and branch constructs. Net coverage is far broader
(every `var <cmp> const` branch in the program), and the FOR-loop
tail's increment fusion can be added later as a second op-class
(`FOR_NEXT`) without conflict.

The recent waves (`2026-05-01-opcode-encoding-wave-2..8`) finished the
structured `[op_class:6][type:2]` encoding and the `FORMAT_VERSION`
bump. Op-class slots `0x3D`, `0x3E`, `0x3F` remain free
(`compiler/container/src/opcode.rs:177`). This plan consumes `0x3D`
and leaves two free.

## New opcode

```text
op-class:  OP_CLASS_CMP_BR = 0x3D
opcodes:   CMP_BR_I32 = encode_opcode(OP_CLASS_CMP_BR, T_I32)   // 0xF4
           CMP_BR_I64 = encode_opcode(OP_CLASS_CMP_BR, T_I64)   // 0xF5
           (T_F32 / T_F64 reserved; trap in v1)

operands:  cmp_op:u8  var_idx:u16  const_idx:u16  target:i16    // 7 bytes
total:     8 bytes per instruction

cmp_op encoding (mod opcode::cmp_op):
  EQ = 0,  NE = 1,  LT_S = 2,  LE_S = 3,  GT_S = 4,  GE_S = 5
  (6..=255 reserved; trap as InvalidCmpOp)
```

### Semantics

```text
let cur   = variables.load_<type>(var_idx);   // direct, no stack
let cnst  = const_pool.get_<type>(const_idx); // direct, no stack
let truth = compare(cur, cnst, cmp_op);
if truth { pc += target; }                    // "branch if true"
// stack effect: 0
```

### Polarity

The opcode has a single polarity ("branch if true"). The compiler
flips the comparison operator to express "branch if false":

```text
EQ ↔ NE     LT ↔ GE     LE ↔ GT
NE ↔ EQ     GE ↔ LT     GT ↔ LE
```

For `const <cmp> var` operand orderings (which compile_expr already
recognises), the compiler commutes operands by mirroring the operator:
`a < b  ⟺  b > a`, etc.

### Floats are deferred

`T_F32` / `T_F64` traps in v1. NaN-aware polarity inversion is unsafe
without an explicit NaN-handling rule, and float comparisons are rare
in PLC control flow. A follow-up plan can add the float variants with
deliberate NaN semantics.

## Codegen sites (v1)

For each site, codegen attempts `try_classify_cmp(expr)` (a new helper
returning `Some((cmp_op, var_idx, const_idx, op_width))` for the
recognised shape `var <cmp> const_literal` with I32/I64 types, else
`None`). If `Some`, emit one `CMP_BR_<t>`; else emit today's sequence
unchanged.

### 1. FOR loop head (`compile_for`, `compile_stmt.rs:894-993`)

Today's head, after `2026-04-30-elide-for-loop-exit-jmp.md`:
```text
LOAD_VAR_I32 control; LOAD_CONST_I32 to; LE_I32; JMP_IF_NOT END     ; pos step
                                       ; GE_I32                     ; neg step
```

`to` is compiled by `compile_expr`; when it is a constant integer
literal (the same precondition the `for_loop_trunc_can_be_elided`
gate already uses), pool it directly with `ctx.add_i32_constant` /
`add_i64_constant` and emit:

```text
CMP_BR_<t>  cmp_op=GT (or LT for neg step), control, to_const, END
```

— branching to `END` when the continuation predicate is **false**.
4 dispatches → 1 per iteration.

When `to` is non-constant, fall back unchanged.

### 2. WHILE → do-while (`compile_while`, `compile_stmt.rs:720-739`)

Restructure WHILE into do-while shape **only when the condition is
classifiable**. For complex conditions, emit today's shape unchanged
(restructuring would otherwise require a `JMP_IF` opcode, which we
deliberately don't spend a slot on).

When `try_classify_cmp(condition)` returns `Some((cmp, var, k))`:

```text
  CMP_BR_<t>  NEG(cmp), var, k, END            ; zero-trip: exit if !cond
BODY:
  ...body...
  CMP_BR_<t>  cmp,      var, k, BODY           ; back-edge: continue if cond
END:
```

Per iteration: 4 dispatches (LOAD_VAR + LOAD_CONST + CMP + JMP_IF_NOT
+ trailing JMP = 5) collapses to a single CMP_BR. Net **5 → 1** for
common `WHILE i < N` shapes.

Fallback (complex condition) keeps today's shape:
```text
LOOP: compile(cond); JMP_IF_NOT END; body; JMP LOOP; END:
```

### 3. REPEAT (`compile_repeat`, `compile_stmt.rs:749-767`)

Already do-while-shaped. Today emits `compile(cond); JMP_IF_NOT LOOP`
at the bottom (continue while cond is false; UNTIL exits when true).

When `try_classify_cmp(until)` returns `Some((cmp, var, k))`, replace
the per-iteration tail with:

```text
CMP_BR_<t>  NEG(cmp), var, k, LOOP             ; back-edge if !until
```

4 dispatches → 1.

### 4. IF / ELSIF (`compile_if`, `compile_stmt.rs:452-510`)

For the IF-condition and each ELSIF-condition, when classifiable,
replace `compile_expr + JMP_IF_NOT next` with one CMP_BR using the
negated comparison.

`compile_if` ELSE body and the trailing `JMP end` are unchanged.

Saves 3 dispatches per classifiable condition. IFs are pervasive in
PLC scan code; the cumulative effect is large.

### 5. CASE — out of scope for v1

CASE selectors compile to a chain of comparisons today. Fusion is
profitable but the CASE codegen is more complex (range cases,
enumeration values); fold it into a follow-up after v1 lands.

## File map

| File | Change |
|------|--------|
| `compiler/container/src/opcode.rs` | Add `OP_CLASS_CMP_BR = 0x3D`. Add `pub const CMP_BR_I32` / `CMP_BR_I64` (T_F32/T_F64 reserved). Add `pub mod cmp_op { EQ, NE, LT_S, LE_S, GT_S, GE_S }`. Note 0x3E/0x3F still free. |
| `compiler/codegen/src/emit.rs` | Add `pub fn emit_cmp_br_i32(&mut self, cmp_op: u8, var_index: VarIndex, const_idx: u16, target: Label)` and `_i64` variant. Stack effect: 0. Use existing `PendingPatch` for the trailing `i16` target — same path as `emit_jmp_if_not` (`emit.rs:463`). |
| `compiler/codegen/src/compile_expr.rs` | Add `pub(crate) fn try_classify_cmp(expr: &Expr, ctx: &CompileContext) -> Option<ClassifiedCmp>` where `ClassifiedCmp { cmp_op: u8, var_index: VarIndex, const_idx: u16, op_width: OpWidth }`. Recognise `Cmp(<op>, Var(v), Lit(k))` and the commuted `Cmp(<op>, Lit(k), Var(v))` forms for I32/I64. Float, struct-field, array-element, and enum LHS forms return `None` (defer). Use `ctx.add_i32_constant` / `add_i64_constant` to pool the literal. Helper `negate_cmp(cmp_op) -> cmp_op` and `commute_cmp(cmp_op) -> cmp_op` live alongside. |
| `compiler/codegen/src/compile_stmt.rs` | (a) `compile_for`: replace head emission with one CMP_BR when `to` is a constant literal of matching width. (b) `compile_while`: restructure to do-while when classifiable; otherwise unchanged. (c) `compile_repeat`: replace tail when classifiable. (d) `compile_if`: replace IF-cond and each ELSIF-cond emission when classifiable. Each site uses the same gating helper. |
| `compiler/vm/src/vm.rs` | Add a single dispatch arm covering `CMP_BR_I32 \| CMP_BR_I64` (the type tag selects `as_i32` vs `as_i64`). Read `cmp_op:u8 var_idx:u16 const_idx:u16 target:i16`. Direct `variables.load(var_idx)` + `container.constant_pool.get_iN(const_idx)`. Match `cmp_op` against the six valid values; `_ => Trap::InvalidCmpOp(cmp_op)`. On true, advance `pc` by `target`. |
| `compiler/vm/src/error.rs` | Add `Trap::InvalidCmpOp(u8)` variant (or extend an existing decoding-error trap if one fits). |
| `compiler/vm/src/profile.rs` | Histogram entries for `CMP_BR_I32` and `CMP_BR_I64`. |
| `compiler/container/src/spec_conformance.rs` | Pin the new mnemonic + encoding. |
| `compiler/codegen/src/spec_conformance.rs` | Pin the new mnemonic in any opcode-coverage table. |
| `specs/design/bytecode-instruction-set.md` | Document `CMP_BR_<type>`: encoding, operand layout, `cmp_op` enum, polarity, stack effect, applicability. Cross-reference `vm-performance.md` §11. |
| `specs/design/vm-performance.md` | Mark §11 as partially implemented (I32/I64; floats and var-var comparisons deferred). Note the WHILE → do-while restructure. |

### Out of file map (intentionally)

- **No bytecode-level test for the new opcode** — pending the in-flight
  rework of how byte-code-level tests are structured.
- **No bytecode verifier change** — verifier hasn't landed. When it
  does, a `CMP_BR` rule needs: stack effect 0; `var_idx` in scope and
  width-matching the type tag; `const_idx` valid for the type; `cmp_op
  in 0..=5`; `target` lands on an instruction boundary. Capture in the
  verifier plan.

## Reused infrastructure

- Constant-pool integer lookup: `container.constant_pool.get_i32` /
  `get_i64`, exercised by `LOAD_CONST_I32` / `LOAD_CONST_I64`
  (`vm.rs:706-711`).
- Variable read with scope check: `scope.check_access` +
  `variables.load` (`vm.rs:725-742`).
- Label patching: `PendingPatch` mechanism (`emit.rs:451-472`) — the
  new emitter pushes one entry pointing at the trailing 2-byte target
  slot.
- AST classification of literal-vs-var in comparison expressions:
  `signed_integer_to_i64` (`compile_expr.rs:1559`), the same `Var`-vs-
  literal pattern matching `try_constant_sign` already does
  (`compile_stmt.rs:777`). The new `try_classify_cmp` factors this
  into a single call that returns the comparison operator alongside.
- Constant pooling: `ctx.add_i32_constant` / `add_i64_constant`
  (`compile_stmt.rs:963`) — already deduplicates.

## Verification

From `compiler/`:

1. `just compile` — clean build.
2. `cargo test -p ironplc-codegen --test compile_loops` and
   `cargo test -p ironplc-codegen --test end_to_end_loops` — existing
   FOR/WHILE/REPEAT tests must pass unchanged. End-to-end coverage is
   the correctness guarantee while bytecode-level tests are paused.
3. `cargo test -p ironplc-codegen` (full crate suite) — IF/ELSIF
   end-to-end tests must pass; in particular boolean conditions, mixed
   signed/unsigned widths, and complex-condition fallbacks (function
   calls, AND/OR chains) must continue to emit today's shape.
4. `cargo test -p ironplc-benchmarks --features profiling -- --nocapture --test-threads=1 profile_for_loop` —
   for `FOR i := 1 TO 100`, the histogram should show:
   - `CMP_BR_I32` count = N+1 (zero-trip + N per-iteration head tests)
     — wait, FOR-head is one CMP_BR per iter that includes the entry
     check, so count = N+1 across the loop.
   - `LOAD_VAR_I32`, `LOAD_CONST_I32`, `LE_I32`, `JMP_IF_NOT` for the
     head all = 0 (replaced).
   - Tail counts (`LOAD_VAR_I32`, `LOAD_CONST_I32`, `ADD_I32`,
     `STORE_VAR_I32`, `JMP`) unchanged from current baseline (FOR_NEXT
     is a follow-up).
5. Add a profiling smoke for WHILE (`profile_while_simple_cmp`): for a
   `WHILE i < 100 DO i := i + 1; END_WHILE` body, expect `CMP_BR_I32`
   = N+1, no per-iter `JMP`. (If a profile_while file does not exist,
   create one alongside `profile_for_loop`.)
6. `cargo bench -p ironplc-benchmarks st_for_loop st_nested_loops
   st_if_chain` (add `st_if_chain` if missing) — capture before/after
   in the PR.
7. `just` — full CI pipeline (clippy + fmt + coverage ≥ 85 %).

## Risks and mitigations

- **Wrong polarity (negation/commutation bugs).** The compiler does
  the cmp-op flips; getting EQ/NE swaps right matters. Mitigation:
  centralise `negate_cmp` / `commute_cmp` as `const fn`s in
  `compile_expr.rs` with exhaustive unit tests over the 6 ops × 2
  directions. End-to-end tests catch any residual mismatch by
  observing wrong loop iteration counts.
- **WHILE restructure changing observable behaviour.** Restructuring
  to do-while preserves trip count exactly (zero-trip pre-check +
  back-edge), so observable state across iterations is identical. The
  restructure only fires when the condition is `var <cmp> const` —
  side-effect-free by construction (variable read + constant
  comparison) — so duplicating the test cannot change behaviour.
- **Constant pool index width.** Pool entries can exceed `u16::MAX`
  in pathological programs; matches the existing `LOAD_CONST_I32`
  constraint. If pool overflow is ever a concern, it is already a
  concern for `LOAD_CONST_I32` and is gated at constant pool
  construction time.
- **`cmp_op` operand integrity.** Bytecode could be hand-crafted with
  an out-of-range `cmp_op`. Trap as `InvalidCmpOp(u8)` rather than
  silently treating unknown values as a fixed comparison. The verifier
  (when it lands) can statically reject these.
- **Span misattribution.** Today's emission attributes problems on the
  comparison to the comparison's own span; the new fused emitter must
  preserve span propagation through the gating helper so diagnostics
  still point at the right token.

## Out of scope (follow-ups)

- `CMP_BR_F32` / `CMP_BR_F64` (NaN-aware polarity).
- Var-var comparison fusion (`var <cmp> var`) — needs a wider operand
  layout or a second op-class slot.
- `FOR_NEXT_<type>` — fuse the FOR tail's increment + bound test +
  back-edge. Composes cleanly on top of CMP_BR; `FOR_NEXT` would
  replace the tail while CMP_BR keeps owning everything else. Worth
  measuring after CMP_BR lands to see whether the additional
  per-iteration win justifies a second op-class.
- WHILE restructure for **complex** conditions — would require a
  `JMP_IF` opcode (one of the two remaining op-class slots). Deferred
  until profiling shows complex-condition WHILEs are hot.
- CASE-selector fusion.
- Bytecode-level tests for `CMP_BR_<type>` — blocked on the byte-code
  test rework.
- Bytecode verifier rule for `CMP_BR_<type>` — blocked on the
  verifier itself.

## Sequencing

Independent of any in-flight wave. Uses op-class slot `0x3D` (free
post-wave-8) and the structured encoding, so no `FORMAT_VERSION`
churn.
