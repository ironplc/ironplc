# Experiment: Hoist `sp` into a local in the dispatch loop

**Date:** 2026-05-02
**Branch:** `claude/stack-metadata-experiment`
**Status:** Hypothesis falsified. No code change recommended.

## Hypothesis

Earlier asm dissection of `ADD_I32` showed **17 of 37 instructions (46%) are stack memory ops** — most of them loading/storing `OperandStack` metadata via `&mut OperandStack`:

```
mov rcx, [r13]            # load stack.len
mov rdx, [rsp + 16]       # load &stack.data slice header
mov rsi, [rdx]            # load stack.data.len
mov rdx, [r9]             # load stack.data.ptr
...
mov [r13], rdi            # store stack.len after pop
```

The hypothesis: hoisting `stack.len` into a register-allocated local `sp: usize` in the dispatch loop, with `OperandStack` methods that take `len: &mut usize` instead of going through `self.len`, would save the per-pop/push load+store of `stack.len` and let LLVM keep `sp` in a register.

## Method

Implemented on `claude/stack-metadata-experiment` (post-merge of [`Optimize ConstantPool primitive lookups with inline storage`](https://github.com/ironplc/ironplc/pull/1032), which dropped baseline VM cost from 500M to 398M instructions).

1. Added `OperandStack::push_local`/`pop_local`/`peek_local`/`peek_at_local`/`dup_local`/`swap_local`/`truncate_by_local` (all `#[inline(always)]`) that take `sp: &mut usize` parameter.
2. Added `OperandStack::len()` and `set_len()` accessors.
3. Updated the five dispatch macros (`binop!`, `cmpop!`, `unaryop!`, `checked_divop!`, `load_const!`) to take `$sp:expr` and call `*_local` methods.
4. In `execute_with_hook`, hoisted `let mut sp: usize = stack.len();` after parameter binding.
5. Replaced all 77 `stack.push/pop/...` call sites within the function with `*_local` variants using `sp`.
6. Synced `sp` to `stack.len` before each external call boundary (`builtin::dispatch`, `CALL`, `FB_CALL`) and reloaded after, plus before `RET` / `RET_VOID` / catch-all error returns.

Measured back-to-back on the same machine via `compiler/benchmarks/examples/vm_vs_native compare 10000` (10 runs each) and callgrind on `vm 1000` (1M total iterations of a counter loop with body `total := total + counter; counter := counter - 1;`).

## Results

| Variant | Median wall-clock | Range | Callgrind Ir (1M iters) |
|---|---|---|---|
| Baseline (post-const-pool) | 37.5 ns/iter | 31.9 – 47.8 | 397,625,336 |
| sp-hoist | 41.2 ns/iter | 36.3 – 50.5 | 388,629,059 |
| **Δ** | **+9.9% slower** | | **-2.3% fewer** |

Wall-clock variance is high (~30%) on this machine; the comparison is back-to-back on the same machine, same minute. The sp-hoist variant is *consistently* slower across all 10 runs.

## Why it didn't work

Asm inspection of the experiment shows `sp` ends up at `[rsp+8]` — **still in stack memory, not a register**. The hot dispatch loop of `ADD_I32` (.LBB3_583) opens with:

```
mov rcx, [rsp + 8]     # load sp from stack memory
test rcx, rcx
mov rax, [rsp + 2168]  # load saved state
...
mov [rsp + 8], rdi     # store sp back to stack memory
```

Same memory-traffic shape as the baseline (which had `stack.len` at `[r13]`). The hoist moved `sp`'s home address but didn't get it into a register.

**Why LLVM doesn't keep `sp` in a register across the dispatch loop:**

1. The dispatch loop is a single big match with ~25 hot opcode arms, each its own basic block. At every `match` arm exit, LLVM has to materialise live state at a known location so the next iteration's basic block can read from it — `sp` lives in the stack frame across these basic block boundaries.
2. Recursive boundaries (`stack.set_len(sp)` before `CALL`/`FB_CALL`/`builtin::dispatch`, then `sp = stack.len()` after) explicitly force `sp` to memory.
3. `&mut sp` is passed through `*_local` calls. Even with `#[inline(always)]`, LLVM treats address-taken locals more conservatively.

The same register-pressure pattern that prevented `pc` from living in a register also prevents `sp`. **The dispatch loop is too big and too varied for LLVM to keep all hot state in registers** — adding `sp` to the local pool just adds more state competing for the same scarce registers.

The `-2.3%` callgrind win comes from the cold paths (fewer overall instructions in `set_len` / `builtin::dispatch` boundary handling), not from hot-path savings. The hot path actually got slightly slower (more instructions per `ADD_I32` due to setup/teardown around the inlined helpers), which dominates wall-clock.

## Conclusion

The diagnostic was correct (~46% of `ADD_I32` is stack memory traffic) but the proposed fix is the wrong shape. The cost is *structural*: `OperandStack` is accessed via `&mut OperandStack` in a loop with too much state, so LLVM keeps stack metadata in memory regardless of which local hoisting we try.

Three options on the table for actually reducing stack memory traffic, in order of complexity:

1. **Raw-pointer dispatch (unsafe)**: cache `(stack_ptr: *mut Slot, stack_capacity: usize, sp: usize)` as plain values at the top of the dispatch loop. Without `&mut` aliasing, LLVM should keep them all in registers. Requires `unsafe`, but with a clear "validated bytecode" invariant from ADR-0006, it's a contained risk. Same approach as production bytecode interpreters.
2. **Restructure `execute_with_hook` signature**: replace `stack: &mut OperandStack` with separate `data: &mut [Slot], sp: &mut usize` parameters. Fewer aliasing layers; slightly less invasive than (1) but doesn't fully address LLVM's pessimism around `&mut`.
3. **Skip ahead to register-based VM** (vm-performance.md Tier 2 §5). Eliminates the operand stack entirely; operands are register indices baked into bytecode. Same target (no per-opcode stack metadata) but addressed at the codegen level.

Given that **superinstructions** (already underway, see CMP_BR fused opcode merged in [#1030](https://github.com/ironplc/ironplc/pull/1030)) reduce per-iteration opcode count and therefore amortise *all* per-opcode costs (dispatch + stack metadata + operand decode), they remain the highest-leverage next step regardless of what we do about stack metadata.

## Recommendation

Do not land this experiment. Continue with superinstructions. Revisit stack metadata only when superinstructions plateau, and at that point evaluate (1) raw-pointer dispatch with verifier-backed invariants — both because it's the only path that actually moves `sp` into a register, and because ADR-0006 already establishes the validation framework that would justify the `unsafe`.

## Critical files referenced

- `compiler/vm/src/stack.rs` — `OperandStack` struct, the `*_local` methods I added (now reverted)
- `compiler/vm/src/vm.rs:691-2125` — `execute_with_hook` dispatch loop
- `compiler/benchmarks/examples/vm_vs_native.rs` — measurement harness
- `specs/design/vm-performance.md` Tier 2 §5 — register-based translation, the eventual answer

## Out of scope

- Raw-pointer dispatch experiment (Option 1 above) — would require ADR-level discussion of unsafe in the dispatch loop.
- Function-signature refactor of `execute_with_hook` (Option 2) — large surface area, uncertain win.
- Register-based VM (Option 3) — already documented as Tier 2 in `vm-performance.md`.
