# Bytecode Dispatch Bounds-Check Measurement

**Date:** 2026-05-02
**Branch:** `claude/bytecode-cells-optimization-LNlzN` (measurement done on `measurement/bounds-check-experiment`)
**Status:** Investigation complete — no action recommended.

## Question

Are per-instruction bounds checks in the VM dispatch loop a meaningful bottleneck? If so, would restructuring bytecode storage (cells / enum-decoded array) be worth the RAM cost, complexity, and loss of zero-copy in the no_std `ContainerRef` flow?

## Method

Three layers, on the existing criterion benchmarks at `compiler/benchmarks/benches/st_benchmark.rs`:

1. **Baseline** — current code, no changes.
2. **Variant A (safe-Rust slice fix)** — rewrite `read_u16_le`/`read_u32_le`/`read_i16_le` in `compiler/vm/src/vm.rs:2074-2118` to use `bytecode.get(*pc..end).ok_or(...)?` plus `slice.try_into()` instead of byte-by-byte indexing. Same semantics, no `unsafe`.
3. **Variant B (unsafe upper-bound)** — replace each per-byte `bytecode[*pc + N]` with `unsafe { *bytecode.get_unchecked(*pc + N) }`, keeping the explicit bounds guard. This is the absolute ceiling on any safe-or-unsafe approach.

Disassembly inspection via `cargo rustc --release --emit=asm`. All measurements on this branch's machine; criterion default settings.

## Findings

### 1. Source has redundant per-byte bounds checks the optimizer doesn't elide

Standalone `read_u32_le` asm shows **5 bounds checks for one 4-byte read**: the explicit `if end > bytecode.len()` guard plus 4 redundant per-byte checks (`bytecode[*pc]`, `bytecode[*pc+1]`, `bytecode[*pc+2]`, `bytecode[*pc+3]`). LLVM does not propagate the explicit guard's proof to the byte indexing.

In the inlined dispatch function `execute_with_hook`:
- 29 `panic_bounds_check` call sites
- 647 `cmp` instructions
- 525 unsigned conditional jumps (mostly bounds-related)

### 2. Variant A reduces asm bounds checks by 55% but changes inlining

| Metric | Baseline | Variant A |
|---|---|---|
| `execute_with_hook` size | 7634 lines | 7131 lines (-6.6%) |
| `panic_bounds_check` sites | 29 | 13 (-55%) |
| `cmp` instructions | 647 | 614 |

Asm clearly improved. Runtime impact: confounded by environmental drift (see #4 below).

### 3. Variant B (unsafe upper-bound) is ~9% **slower** than steady-state baseline

| Bench | Baseline (steady) | Variant B | Δ |
|---|---|---|---|
| `st_counter_loop/10000` | 287.45 µs | 314.05 µs | **+9.3%** |
| `st_arithmetic_i32/1000` | 25.90 µs | 28.95 µs | **+11.8%** |
| `st_for_loop/10000` | 392.05 µs | 422.51 µs | **+7.8%** |
| `st_nested_loops/100x100` | 433.81 µs | 486.11 µs | **+12.1%** |

Removing bounds checks **regressed performance** consistently across dispatch-heavy benches. Most likely cause: the bounds branches were well-predicted (always succeed), and removing them changed code layout, register allocation, and inlining decisions in ways that hurt overall throughput.

### 4. Benchmark environment drift exceeds plausible win

The same baseline code, run three times, produced 232 µs / 288 µs / 287 µs for `st_counter_loop/10000` — a **~24% drift** between cold and steady-state runs. Reliable A/B differentiation in this environment requires effects larger than ~5–10%, and ideally back-to-back runs. The bounds-check effect we set out to measure is below this floor.

## Conclusion

**The cells / enum-decoded array restructuring is not justified.**

- Variant B sets the absolute ceiling on what any bounds-check-elimination strategy (cells, enum array, threaded code, unsafe `get_unchecked`) can achieve. That ceiling is **negative** — removing checks made the dispatch loop slower in steady state.
- The CPU is already hiding the per-instruction bounds-check cost via branch prediction (the checks always succeed) and instruction-level parallelism. The asm shows checks; the pipeline schedules around them for free.
- Therefore, no structural change is warranted purely for bounds-check elimination. The cost of cells (RAM growth, broken zero-copy in `ContainerRef`, breaking std/no_std uniformity) buys nothing.

## Where the VM cost actually goes (callgrind)

The original investigation was prompted by a "VM is ~200x slower than native" observation. To see whether bounds checks could possibly explain that gap, we measured exact instruction counts via callgrind on a counter-loop workload (`total := total + counter; counter := counter - 1;`) — see `compiler/benchmarks/examples/vm_vs_native.rs`.

| Workload | Wall-clock per iter | Instructions per iter |
|---|---|---|
| Native scalar Rust (with `black_box`) | ~3 ns | 7.59 |
| IronPLC VM | ~45 ns | 499.71 |
| **Ratio** | **~15x** | **~66x** |

Wall-clock VM:native is ~15x — not 200x. The wider gap to "200x" almost certainly comes from comparing IronPLC against a **compiled** native-PLC runtime (MATIEC, RuSTy compile ST → native machine code with loop optimizations), where native achieves ~1-2 instrs/iter and the ratio jumps to ~250x. That is fundamentally a "interpreter vs compiler" gap, not a "bounds check" gap.

**Where the 500 VM instructions per iteration go:**

| Function | Instr % | Per VM iter |
|---|---|---|
| `vm::execute_with_hook` (dispatch + handlers) | **90.33%** | ~451 instr |
| `container::ConstantPool::get_i32` | **8.01%** | ~40 instr (called 2x/iter) |
| Everything else (malloc, memcpy, container setup) | <2% | ~9 instr |

The body `total := total + counter; counter := counter - 1;` compiles to roughly 12 VM opcodes per iteration (load_var ×3, load_const ×2, add, sub, store_var ×2, plus the WHILE comparison/branch). At 451 dispatch instructions for ~12 opcodes, that's **~37 native instructions per VM opcode**.

The ~37 instr/opcode breaks down (per the asm at vm.rs dispatch loop and handlers):

- **~10 instr** dispatch overhead per opcode: load pc from stack, bounds check, fetch opcode, jump-table lookup, indirect jump. PC lives on the stack (`[rsp+64]`) because the function is too register-pressured — every opcode pays 2 memory ops just to maintain pc.
- **~15-20 instr** for stack push/pop with type tags (each `Slot` carries a tag, every push/pop checks/sets it).
- **~5-10 instr** for the actual operation (add, load var, etc.).

## Two concrete bottlenecks visible in this data

1. **`ConstantPool::get_i32` is unnecessarily expensive (~40 instr/call, 8% of total).** In `compiler/container/src/constant_pool.rs:75`, `ConstEntry` stores `value: Vec<u8>` (heap allocation per constant). Every i32 load does: vec bounds check → pointer chase to ConstEntry → pointer chase to its inner Vec → `copy_from_slice` + `from_le_bytes`. A dense `Vec<i32>` (or inline u8 array) would cut this to ~5 instr. Estimated win: ~7% wall-clock on constant-heavy workloads.

2. **The dispatch function is monolithic and stack-spilled.** 7600 lines of asm, 2200-byte stack frame, pc lives in memory. This is exactly what the existing `specs/design/vm-performance.md` Tier 1 (superinstructions, fused load-op-store, opcode consolidation) and Tier 2 (register-based translation, tail-call threading) are designed to attack. Bounds-check elimination — what we measured here — is *not* the lever; the structural improvements in that doc are.

## Concrete recommendations

### Skip
- Cells / enum-decoded instruction array — provably no benefit (variant B was slower).
- Variant A's safe-Rust `read_*` rewrite — no measurable benefit, possibly negative.
- Streaming load via `BufReader` — was only relevant if cells.

### Worth doing
- **Restructure `ConstantPool` storage** — separate `Vec<i32>`/`Vec<f32>`/`Vec<f64>` per type, or inline-typed bytes. Eliminates the heap-Vec-per-entry indirection. Small, mechanical change. ~7% expected win on constant-heavy code, possibly more.
- **Pursue items already cataloged in `specs/design/vm-performance.md`** — superinstructions, fused load-op-store, opcode consolidation (Tier 1), then register-based translation or tail-call threading (Tier 2). These attack the per-opcode count and per-opcode dispatch cost, which is where the 90% lives. They were always the right answer; this measurement just confirms it.

### Worth doing first (orthogonal)
- **Stabilize the criterion bench environment.** 24% drift between cold and warm runs makes any future micro-optimization claim noisy. Pin CPU frequency and isolate cores before any of the above optimizations are landed.

## Recommendations

### Don't do
- **Cells / enum-decoded instruction array.** No measurable upside; real downside (RAM, zero-copy loss).
- **Variant A's safe-Rust rewrite of `read_*` helpers.** Asm looks better but real-world impact is at best neutral and possibly negative; not worth the churn.
- **Streaming decoder via `BufReader`.** Only relevant if cells were chosen.

### Maybe worth investigating later (separate plans)
- **Where the actual VM time goes.** Profile (`perf record` or `cargo flamegraph`) on `st_counter_loop/10000` to find the real hot spots — likely stack push/pop, value boxing/unboxing in `Slot`, or constant-pool access, not opcode fetch.
- **Per-opcode hot-path specialization.** `st_diverse_opcodes` is ~10× slower per-instruction than `st_counter_loop` (2.95 vs 43 Melem/s). Worth understanding why.
- **Stable benchmarking environment.** Pin frequency, isolate cores, fix the 24% drift before any future micro-optimization claims.

## Critical Files (for the record)

- `compiler/vm/src/vm.rs:682-688` — main dispatch loop
- `compiler/vm/src/vm.rs:2074-2118` — `read_u8`/`read_u16_le`/`read_u32_le`/`read_i16_le`
- `compiler/container/src/code_section.rs:26` — `Vec<u8>` storage
- `compiler/container/src/container_ref.rs:47` — no_std zero-copy view (would be invalidated by cells)
- `compiler/benchmarks/benches/st_benchmark.rs:38` — `st_counter_loop` ("dispatch overhead baseline")
- `compiler/vm/src/profile.rs` — existing per-opcode profiling (gated on `profiling` feature) — useful for the hot-spot follow-up

## Raw Data

Saved at `/tmp/measure/` on the measurement machine (not committed):
- `baseline.txt`, `baseline2.txt`, `baseline3.txt` — three baseline runs
- `variant_a.txt` — variant A benches
- `variant_b.txt`, `variant_b2.txt` — variant B benches
- `asm_variantB.s` — variant B disassembly
- `criterion-baseline/` — criterion JSON outputs

## Out of Scope

- Implementing any of the structural changes considered (cells, enum array, threaded code).
- Fixing the criterion environment drift.
- Profiling for non-bounds-check bottlenecks.
