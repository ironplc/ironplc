# Experiment: Cold-path-split via `#[inline(never)]` (option #1)

**Date:** 2026-05-02
**Branch:** `claude/bytecode-cells-optimization-LNlzN`
**Driver:** [bytecode-dispatch-bounds-check measurement](2026-05-02-bytecode-dispatch-bounds-check-measurement.md)
**Status:** Hypothesis falsified. No code change recommended.

## Hypothesis

Earlier callgrind data showed `vm::execute_with_hook` is 7634 asm lines with a 2200-byte stack frame, and pc lives at `[rsp+64]` (stack memory) every dispatch — strongly suggesting the function is too register-pressured for LLVM to keep pc in a register. The hypothesis: outlining cold opcode handlers as `#[inline(never)]` functions would shrink the hot dispatch enough to relieve register pressure, letting pc move to a register and saving ~2 memory ops per dispatch.

The user's prior two-level-dispatch attempt failed; this experiment was meant to test whether the *register-pressure* angle (rather than dispatch-table-size angle) was the right diagnosis.

## Method

Two outlining sizes, both as `#[inline(never)]` functions called from a single fall-through arm in the main match:

1. **Small outline**: the BUILTIN arm body (numeric ↔ STRING conversions + CMP_STR), ~118 source lines.
2. **Larger outline**: the contiguous string opcode section (STR_INIT through STR_STORE_ARRAY_ELEM), ~602 source lines.

Measured for each:
- Asm size of `execute_with_hook` after release-mode build
- Callgrind instruction count on `examples/vm_vs_native vm 1000` (counter-loop workload, 1M total iterations)
- Wall-clock ns/iter on the same workload, median of 5 back-to-back runs

Each variant was compared back-to-back with a re-measured baseline on the same machine to neutralise environmental drift (which had previously measured ~24% in this environment).

## Results

| Variant | `execute_with_hook` asm | `panic_bounds_check` sites | Callgrind Ir (1M iters) | Median wall-clock |
|---|---|---|---|---|
| Baseline | 7634 lines | 29 | 499,710,708 | 31.90 ns/iter |
| BUILTIN outlined (118 lines) | 6960 (-9%) | 22 (-24%) | 510,723,786 (+2.2%) | 33.16 ns/iter (+4.0%) |
| String section outlined (602 lines) | 4975 (-35%) | 14 (-52%) | 514,679,047 (+3.0%) | 33.32 ns/iter (+4.5%) |

**The dispatch function shrank dramatically, but the experiment got *slower* in both retired-instruction count and wall-clock.**

PC remained at `[rsp+48]` (stack memory) in the outlined version — the 35% function-size reduction did not relieve register pressure to the point where LLVM could keep pc in a register. The bottleneck registers are not being held by cold opcodes.

## Diagnosis

The hypothesis was wrong. The register pressure in the dispatch loop is dominated by the **hot opcodes themselves**, not the cold ones LLVM already separates into late basic blocks. Each of the ~25 hot opcode handlers (LOAD/STORE_VAR_*, ADD/SUB/MUL/DIV_*, comparison, control flow) needs its own state — bytecode operand decode, stack push/pop, var table access, container indirection. The cumulative working set is already too large for pc to fit in a register, even with 35% of cold code outlined.

LLVM's existing layout already handles cold-vs-hot separation effectively — the explicit `#[inline(never)]` adds function-call overhead (register save/restore at the call boundary, plus the cold function's own prologue/epilogue) without freeing useful registers in the hot path.

Two-level dispatch failing was consistent with this: smaller dispatch *tables* weren't the problem either; per-opcode work is.

## Implication

Architectural changes that reduce **per-opcode work** or **per-iteration opcode count** are the only paths forward for the dispatch loop. Cold-path split is a dead end.

The other two options from the earlier discussion remain in play:

- **Function-per-opcode dispatch table** — *might* help because each opcode handler becomes its own function with independent register allocation. But comes with call/ret overhead per dispatch (~10 instr) that may offset gains. Hard to predict; would require a substantial refactor to test (roughly 25-40 hot handlers each becoming a fn-pointer table entry).
- **Superinstructions / fused opcodes** — attacks per-iteration opcode count directly. The counter-loop workload runs ~12 opcodes per iteration; fusing the common `LOAD_VAR + LOAD_CONST + BINOP + STORE_VAR` pattern halves that. Codegen change, not VM-architecture change. Already cataloged in `specs/design/vm-performance.md` Tier 1 §4.
- **Register-based VM** (vm-performance.md Tier 2) — eliminates stack push/pop entirely; operands are register indices baked into bytecode. Big refactor but fundamentally addresses the per-opcode work problem.

## Recommendations

### Don't do
- Cold-path split via `#[inline(never)]`. Falsified at two scales.
- Two-level dispatch (already tried). Same root cause.

### Most actionable next step
- **Superinstructions for the hot pattern** in `specs/design/vm-performance.md` Tier 1 §4. This is the cheapest architectural win that attacks the right axis (per-iter opcode count). It's a codegen change, doesn't disturb the VM dispatch architecture, and the design is already specified.

### Worth considering after that
- If the superinstruction win isn't enough, **register-based VM** (Tier 2 §5). The major lever still on the table; substantial work but the design is documented.

## Critical files referenced

- `compiler/vm/src/vm.rs:665` — `execute_with_hook`
- `compiler/vm/src/vm.rs:693-2067` — the dispatch match
- `compiler/benchmarks/examples/vm_vs_native.rs` — measurement harness
- `specs/design/vm-performance.md` — existing optimization catalog (Tier 1 §4 superinstructions, Tier 2 §5 register-based)

## Out of scope

- Function-per-opcode dispatch table (would require 25-40 handler refactors; a separate experiment).
- Wider outline including FB_CALL / array ops (would require generic-on-H signature for FB_CALL recursion).
- Stable bench environment (orthogonal; mentioned in the parent measurement plan).
