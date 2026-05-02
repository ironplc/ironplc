# Experiment: Noinline-everything dispatch (subroutine threading lite)

**Date:** 2026-05-02
**Branch:** `claude/noinline-handlers-experiment`
**Status:** **Negative on this machine** — pending verification on better-controlled hardware.

## Hypothesis

The dispatch survey concluded that "splitting the function is the dominant win" for VM dispatch performance, regardless of which threading variant (subroutine, direct, computed-goto) is layered on top. This experiment tests the cheapest version of that idea: keep the existing `match` dispatch loop, but extract every opcode arm body into its own `#[inline(never)] fn` so LLVM allocates registers per-handler instead of one giant function.

If it helps, full subroutine threading (function-pointer table replacing the `match`) is the next step. If it doesn't, the function-call overhead per opcode dominates and we need different leverage (tail-call dispatch, register-based VM).

## Method

89 opcode handlers extracted to module-level `#[inline(never)] fn`s in `compiler/vm/src/vm.rs`. Five categories handled by handler-generating macros (`gen_handler_binop!`, `gen_handler_cmpop!`, `gen_handler_unaryop!`, `gen_handler_checked_divop!`, `gen_handler_load_const!`); the rest hand-extracted. Match arms reduced to one-line calls: `opcode::ADD_I32 => op_add_i32(stack)?`.

Kept inline (need hook generic, exit-loop signal, or temp_alloc / data_region indirection):
- `BUILTIN` (inner `func_id` match with conversions and `temp_alloc`)
- `CALL`, `FB_CALL` (recursive into `execute_with_hook` with `H: DebugHook`)
- `RET`, `RET_VOID` (exit dispatch loop)
- All `STR_*`, `*_STR`, `STR_*_ARRAY_*`, `FB_*`, `LOAD_ARRAY`, `STORE_ARRAY`, `*_ARRAY_DEREF` (cold for the test workload, complex bodies)

So 89 hot/medium handlers extracted, ~25 cold-and-complex arms kept inline.

`execute_with_hook` shrank from **7634 → 5199 asm lines (-32%)**.

Measured back-to-back on the same machine via `compiler/benchmarks/examples/vm_vs_native compare 10000` (10 runs each) and callgrind on `vm 1000` (1M iterations of `total := total + counter; counter := counter - 1;`).

## Results

| Variant | Median wall-clock VM | Range | Callgrind Ir (1M iters) |
|---|---|---|---|
| Baseline (main, post-const-pool) | 33.65 ns/iter | 33.5 – 34.0 (±1%) | 397,625,961 |
| Noinline-all | 43.50 ns/iter | 43.3 – 46.9 (±7%) | 521,607,169 |
| **Δ** | **+29% slower** | | **+31% (+124M instructions)** |

The ~124M extra instructions over 1M iterations work out to **~124 extra native instructions per dispatched opcode** — which is approximately the function-call/return prologue+epilogue overhead summed across the ~12 opcodes that fire per loop iteration. The math:

- Counter loop dispatches ~12 opcodes per iteration.
- A function call in System V ABI on x86_64 costs roughly 8–12 instructions of overhead (call, save callee-saved regs touched by the callee, ret, restore).
- 12 opcodes × 10 instr/call = ~120 extra instr/iter. Matches the observed 124.

Per-handler register allocation helps some (each handler is small enough that its locals can stay in registers), but **the call-overhead loss exceeds the register-allocation gain by ~30%**.

## Why it didn't work

Subroutine threading via the standard Rust calling convention pays full call/ret overhead per dispatch. The handlers are small (typically 5–15 instructions), so the call/ret pair is a meaningful fraction of total work.

The literature (and the agent's research summary) was clear that the *direct-threaded* variant (tail-call dispatch) is what avoids this — each handler ends with `become next_handler(state)` which compiles to a tail jump rather than call+ret. Subroutine threading was always second-best, and on small-bodied handlers like ours the call overhead dominates.

This is consistent with Pulley's data, which showed that even *with* tail calls (when LLVM cooperates), the speedup vs match dispatch is only a few percent and sometimes negative.

## What this rules in/out

- **Subroutine threading (function-pointer table) — likely also a regression.** The fn-ptr indirect call has the same call/ret overhead as the noinline-direct-call here, plus a memory load for the fn pointer. If `match`-with-noinline is +29% slower, fn-ptr-table will be similar or slightly worse. Not worth implementing on top of stable Rust.
- **Tail-call threaded code — promising in theory, unavailable in practice.** `become` is still nightly (target 2027 stabilization per Trifecta). `extern "C" fn` + LLVM TCO is too fragile to commit to.
- **Register-based VM — the only structural change still on the table.** It attacks the per-iteration *opcode count* (each "operation" becomes one register-based instruction instead of 3–4 stack-based ones), which amortises everything: dispatch, stack metadata, operand decode. Wasmi 0.32's 5× speedup is the existence proof.
- **Superinstructions — keep going.** Same axis as register-based: reduce per-iteration opcode count. Each fused superinstruction directly cuts dispatch cost.

## Caveats / things to verify on better hardware

- This test ran on a noisy shared environment; ~7% variance on the experiment runs is higher than baseline's ±1%. The signal is large enough (~30%) that noise doesn't change the conclusion, but a clean machine would let us confirm the exact magnitude.
- Counter-loop is one workload. A workload with more cold opcodes or smaller hot-loop fraction might see different ratios — though the call overhead is per-opcode, so workloads with more dispatches would see larger absolute (and similar relative) loss.
- I did not try `extern "C"` for the handlers. Worth one try if you want to be thorough, but the call/ret instructions themselves are the same — `extern "C"` mainly changes which registers are caller-saved vs callee-saved. Unlikely to flip the sign of the result.

## Recommendation

Do not land this experiment. Confirm on better hardware if you want, but the signal is unambiguous on this machine: noinline-everything is **+29% slower wall-clock, +31% callgrind instructions**.

Path forward, in order:
1. **Continue superinstructions** — the only stable-Rust win that attacks per-iteration opcode count without paying call/ret overhead. CMP_BR is the model; identify the next 3–5 fusable patterns.
2. **Plan register-based IR translation** as the larger structural win. Wasmi's 5× is hard to ignore, and it composes with whatever dispatch style we end up with.
3. **Revisit dispatch architecture only when `become` stabilises** (~2027 per Trifecta). At that point, mechanically convert the existing match to tail-call threaded.

## Critical files

- `compiler/vm/src/vm.rs` — match dispatch and 89 outlined `op_*` handlers
- `compiler/benchmarks/examples/vm_vs_native.rs` — measurement harness
- `specs/plans/2026-05-02-stack-sp-hoist-experiment.md` — earlier sp-hoist experiment, same conclusion shape (negative for stable-Rust dispatch tweaks)

## Reproducing on different hardware

```bash
git checkout claude/noinline-handlers-experiment
cd compiler
cargo build --release --example vm_vs_native -p ironplc-benchmarks

# Wall-clock A/B, ten runs each (back-to-back to neutralise drift):
for i in $(seq 1 10); do target/release/examples/vm_vs_native compare 10000; done
git stash; cargo build --release --example vm_vs_native -p ironplc-benchmarks
for i in $(seq 1 10); do target/release/examples/vm_vs_native compare 10000; done
git stash pop

# Callgrind for instruction counts (immune to wall-clock noise):
valgrind --tool=callgrind --callgrind-out-file=/tmp/exp.cg \
  target/release/examples/vm_vs_native vm 1000
git stash
cargo build --release --example vm_vs_native -p ironplc-benchmarks
valgrind --tool=callgrind --callgrind-out-file=/tmp/base.cg \
  target/release/examples/vm_vs_native vm 1000
git stash pop
```
