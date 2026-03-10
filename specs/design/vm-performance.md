# VM Performance Optimization

## Overview

This spec catalogs performance optimization opportunities for the IronPLC bytecode VM. The VM is currently ~100x slower than native-code PLC runtimes (MATIEC, RuSTy). While some gap is inherent to interpretation, significant overhead comes from per-instruction safety checks, dispatch overhead, and unoptimized string/memory operations.

The core insight is that IronPLC's ADR-0006 already mandates bytecode verification at load time. If the verifier proves safety properties statically, the interpreter can skip runtime checks — achieving safety without per-instruction overhead.

This spec builds on:

- **[ADR-0000: Stack-Based Bytecode VM](../adrs/0000-stack-based-bytecode-vm.md)**: The VM architecture decision
- **[ADR-0005: Safety-First Design Principle](../adrs/0005-safety-first-design-principle.md)**: The standing policy for safety vs. performance trade-offs
- **[ADR-0006: Bytecode Verification Requirement](../adrs/0006-bytecode-verification-requirement.md)**: The verifier that enables safe unchecked execution
- **[ADR-0010: no_std VM for Embedded Targets](../adrs/0010-no-std-vm-for-embedded-targets.md)**: The `no_std` constraint
- **[Bytecode Verifier Rules](bytecode-verifier-rules.md)**: The specific properties the verifier checks

## Constraints

- **Safety-first** (ADR-0005): No optimization may weaken safety guarantees. The verifier is the trust boundary.
- **`no_std` compatible**: Core optimizations must work without heap allocation or `std`. Platform-specific acceleration tiers (`std`-only) are acceptable as optional enhancements.
- **Deterministic timing**: Optimizations should improve WCET predictability, not worsen it. Fewer branches = more predictable timing.

## Current Overhead Sources

1. **Per-instruction safety checks**: Every `push()?`, `pop()?`, `load(index)?`, `store(index)?`, `scope.check_access(index)?`, and `constant_pool.get_*(index)?` returns `Result<_, Trap>` with bounds checking. A simple `ADD_I32` involves ~8 conditional branches.
2. **Dispatch overhead**: A `match` statement with ~150 arms. Each iteration requires: load opcode → branch to handler → execute → jump back to loop top → load next opcode.
3. **String operations**: Byte-by-byte copy loops instead of `memcpy`/`copy_from_slice`. Each string opcode has 2-4 additional bounds checks.
4. **Heap allocation in hot path**: MUX operations allocate `vec![0; n]` on every call (also incompatible with `no_std`).
5. **Variable access indirection**: Two levels of checking per access (scope validation + bounds check).

## Tier 1: High Impact, Moderate Effort

### 1. Verification-Gated Unchecked Operations

**The single biggest opportunity.** ADR-0006 already requires a bytecode verifier. If the verifier proves:

- Stack depth never underflows/overflows on any path → eliminate stack bounds checks (use `get_unchecked`)
- All variable indices are within bounds → eliminate variable table bounds checks
- All variable indices pass scope validation → eliminate per-access scope checks
- All constant pool indices are valid → eliminate constant pool bounds checks
- All jump targets land on instruction boundaries → eliminate PC bounds checks

Add a `verify()` pass that runs once after `load()`, before the first `execute()`. It walks the bytecode using abstract interpretation — tracking stack depth, validating all offsets against region sizes, checking temp buffer counts. If verified, `execute()` uses unsafe unchecked indexing. If verification fails, reject the bytecode at load time. This is the same pattern as the JVM bytecode verifier and WebAssembly validation. For a PLC runtime where the same bytecode runs millions of scan cycles, the one-time verification cost is negligible.

The verifier is the trust boundary, not per-instruction checks. Defense-in-depth is maintained: the verifier catches bad bytecode at load time.

#### Three categories of runtime checks

*Category 1: Structurally verifiable at load time (eliminate entirely)*

- `data_offset + STRING_HEADER_BYTES > data_region.len()` — verifier checks all data_offsets
- `data_offset + STRING_HEADER_BYTES + max_length > data_region.len()` — same
- `buf_end > temp_buf.len()` — verifier tracks temp buffer allocation per function
- `max_temp_buf_bytes == 0` — verifier checks string functions have allocated temp buffers
- Stack overflow/underflow on every `push()?` / `pop()?` — verifier tracks depth on all control-flow paths
- Constant pool bounds on `get_str(index)`, `get_i32(index)`, etc. — all pool indices are in bytecode
- Variable table bounds on `load(index)` / `store(index)` — all var indices are in bytecode
- Scope access checks on every variable access — all indices are static

*Category 2: Data-dependent but invariant-based (can also eliminate)*

- `src_cur_len.min(src_max_len)` defensive clamp in `STR_LOAD_VAR` — assumes `cur_length` might be corrupted. If the verifier proves only VM opcodes write to the data region, the invariant `cur_length <= max_length` is maintained by construction (`STR_STORE_VAR` enforces it).

*Category 3: Genuine runtime semantics (must keep)*

- Truncation logic `src_cur_len.min(dest_max_len)` in `STR_STORE_VAR` — this isn't error checking, it's IEC 61131-3 assignment semantics (string truncation on assignment to a shorter STRING variable).

#### Implementation approach

- Add `push_unchecked` / `pop_unchecked` to `OperandStack` that skip the bounds check
- Add `load_unchecked` / `store_unchecked` to `VariableTable`
- For string opcodes, skip all Category 1 & 2 checks, use direct slice indexing
- Rust's own slice bounds checks are still present even after removing the explicit `Trap` checks — true elimination requires `get_unchecked()` / `get_unchecked_mut()` which are `unsafe`
- Either duplicate the dispatch loop (verified vs unverified) or use a compile-time generic parameter to select checked/unchecked behavior
- The verifier must guarantee all the properties listed above — this is already part of its specification in ADR-0006

**WCET benefit**: Fewer branches = more predictable timing. This actually improves determinism, which matters for PLC scan cycle WCET analysis.

**Expected improvement**: For scalar opcodes, eliminates the `?` on `push`/`pop`/`load`/`store` (3-5 branches per instruction). For a simple `ADD_I32`, this cuts from ~8 branches to ~1 (the dispatch). For string opcodes, the impact is much larger — each string opcode currently has 2-4 bounds checks that would become zero. Research suggests 10-30% overall speedup for scalar code; string-heavy code could see significantly more.

**Files**: `vm/src/stack.rs`, `vm/src/variable_table.rs`, `vm/src/vm.rs`

### 2. Replace Byte-by-Byte String Copies with `copy_from_slice`

Multiple string opcodes (`STR_STORE_VAR`, `STR_LOAD_VAR`, `REPLACE_STR`, `INSERT_STR`, `DELETE_STR`, `CONCAT_STR`) use `for i in 0..len` byte-by-byte copy loops instead of `copy_from_slice()`.

Current:
```rust
for i in 0..copy_len {
    data_region[data_offset + STRING_HEADER_BYTES + i] =
        temp_buf[buf_start + STRING_HEADER_BYTES + i];
}
```

Should be:
```rust
data_region[dest_start..dest_start + copy_len]
    .copy_from_slice(&temp_buf[src_start..src_start + copy_len]);
```

`copy_from_slice` compiles to `memcpy` which uses SIMD/word-aligned copies. This is a straightforward fix that could be 4-16x faster for string operations depending on string length.

When source and destination overlap within the same buffer, use `copy_within` instead. This only applies to data_region-to-data_region copies (which don't currently happen in the opcode set).

**Files**: `vm/src/vm.rs` (string opcode handlers)

### 3. Eliminate MUX Heap Allocation

MUX operations (`builtin.rs`) allocate `vec![0; n]` on every call. This is a heap allocation in the hot path, and incompatible with `no_std`.

MUX can be implemented by peeking into the stack without copying. Since the stack already holds all inputs, index directly into the stack:

```rust
fn dispatch_mux_i32(n: usize, stack: &mut OperandStack) -> Result<(), Trap> {
    let k_slot = stack.peek_at(n)?;  // peek at K below the n inputs
    let k = k_slot.as_i32();
    let idx = (k.max(0) as usize).min(n - 1);
    let result = stack.peek_at(n - 1 - idx)?;  // peek at the selected input
    stack.drop_n(n + 1)?;  // drop all inputs + K
    stack.push(result)?;
    Ok(())
}
```

This requires adding `peek_at(depth)` and `drop_n(count)` to `OperandStack`.

**Files**: `vm/src/builtin.rs`, `vm/src/stack.rs`

### 4. Fused Load-Op-Store Superinstructions

PLC programs are dominated by the pattern: load variable, operate, store result. In the current VM, `x := x + 1` becomes:
```
LOAD_VAR_I32 idx    (3 bytes, scope check + bounds check + push)
LOAD_CONST_I32 1    (3 bytes, pool lookup + push)
ADD_I32             (1 byte, pop + pop + push)
STORE_VAR_I32 idx   (3 bytes, scope check + bounds check + pop)
```

That's 4 dispatches, 10 bytes, ~8 stack operations, ~6 bounds checks.

A fused `INC_VAR_I32 idx` superinstruction:
```
INC_VAR_I32 idx     (3 bytes, 1 dispatch, 0 stack ops, 1 bounds check)
```

Candidate superinstructions (profile-guided selection):
- `INC_VAR_I32` / `DEC_VAR_I32` — increment/decrement variable by 1
- `LOAD_ADD_STORE_I32` — load var, add top-of-stack, store back
- `LOAD_CMP_JMPNOT_I32` — load var, compare with const, conditional jump (common in IF/CASE)
- `LOAD_CONST_SMALL_I32` — load small integer (-128..127) with 1-byte immediate instead of pool lookup

These save dispatch overhead (the biggest per-instruction cost) and eliminate intermediate stack traffic. Research shows 1.5-3x speedup from superinstructions in switch-based interpreters, particularly effective on embedded CPUs with simpler branch predictors.

ADR-0005 notes 99 opcode slots remain, and the emitter/verifier can be extended mechanically.

**Files**: `container/src/opcode.rs`, `vm/src/vm.rs`, `codegen/src/emit.rs`, verifier

### 4b. Opcode Consolidation to Reduce Instruction Cache Pressure

**The dispatch loop compiles to ~18KB of machine code** (measured in release builds). A typical L1 instruction cache is 32KB, so the dispatch loop alone occupies over half of L1i. This creates two performance problems:

1. **L1i cache thrashing**: With ~96 dispatch targets, the instruction footprint exceeds what fits comfortably in L1i. Cold opcode paths evict hot ones.
2. **Branch target buffer (BTB) pressure**: CPUs have limited BTB entries for indirect branches. With ~96 targets in the dispatch `match`, the indirect branch at the top of the loop misses frequently, causing pipeline stalls.

The root cause is type specialization at the opcode level. Every arithmetic and comparison operation has 4 separate opcodes per type family (i32, i64, f32, f64), plus unsigned variants. For example, ADD alone has 4 opcodes (ADD_I32, ADD_I64, ADD_F32, ADD_F64). Each expands to nearly identical machine code — pop two slots, apply one CPU instruction, push result — but the compiler emits separate code for every dispatch target. Source-level macro deduplication does not help; the compiled output is the same size.

**Key insight**: `Slot` is already type-erased — it's a `u64` wrapper. The LOAD_VAR/STORE_VAR consolidation proved this: all four type variants had identical code and collapsed into a single arm via or-patterns. Arithmetic ops can't fully type-erase (the CPU instruction genuinely differs for integer vs float), but the dispatch structure can be reorganized so the CPU sees far fewer unique code paths.

#### Option A: Two-Level Dispatch (Recommended)

Restructure the opcode byte so that bits encode both operation class and type:

```
opcode byte layout:  [operation:5][type:3]
```

The outer dispatch matches on operation class (~25-30 targets instead of ~96), and each arm contains a small inner switch on type. The inner switch is:
- **Small**: 2-4 arms, fitting in a few cache lines
- **Predictable**: IEC 61131-3 programs tend to operate on the same type within a code section, so the branch predictor learns quickly

Cost: one extra mask/shift per dispatch plus a predictable inner branch (~1-3 cycles). Benefit: avoiding icache misses that cost ~100+ cycles each.

#### Option B: Function Pointer Tables

Store typed operation implementations as function pointers indexed by type tag:

```rust
type BinOp = fn(Slot, Slot) -> Result<Slot, Trap>;
const ADD_TABLE: [BinOp; 4] = [add_i32, add_i64, add_f32, add_f64];

// In dispatch:
OpClass::ADD => {
    let type_tag = opcode & 0x07;
    let b = stack.pop()?;
    let a = stack.pop()?;
    stack.push(ADD_TABLE[type_tag as usize](a, b)?)?;
}
```

Makes the dispatch loop very compact but trades for an indirect function call per operation. The call overhead (call/ret, possible icache miss on the callee) may offset gains.

#### Option C: Collapse Only Identical Implementations

The least invasive approach. Use or-patterns to merge arms that have truly identical machine code (as already done for LOAD_VAR/STORE_VAR). Leave type-specific operations as separate arms but group them for spatial locality.

Lowest risk, but smallest icache benefit.

#### Estimated Impact

| Metric | Current | After Option A |
|--------|---------|----------------|
| `execute` function size | ~18KB | ~6-8KB |
| Dispatch targets in main match | ~96 | ~25-30 |
| L1i budget consumed | ~56% of 32KB | ~19-25% |

This is a bytecode format change, so it should be coordinated with other opcode encoding changes (superinstructions, inline constants) to avoid multiple breaking changes.

**Files**: `container/src/opcode.rs`, `vm/src/vm.rs`, `codegen/src/emit.rs`, verifier

## Tier 2: Medium Impact, Significant Effort

### 5. Internal Register-Based Translation (wasmi-style)

wasmi (the Wasm interpreter) achieved up to 5x execution speedup per version by translating stack-based Wasm bytecode to an internal register-based IR at load time. The key insight: stack-based bytecode is easy to generate but slow to execute; register-based is hard to generate but fast to execute. Do both — keep the stack-based format for the wire/compiler, translate to registers at load time.

How it works:
1. At load time, after verification, translate each function's stack bytecode to a register-based internal representation
2. The register IR uses virtual registers (array indices) instead of push/pop
3. `ADD r0, r1, r2` replaces `LOAD, LOAD, ADD` — fewer instructions, no stack traffic
4. The internal IR is never serialized — it exists only in memory during execution

What wasmi did specifically:
- Instructions packed into 8-byte cache-aligned words
- Specialized common cases (`CallSingle`, `ReturnSingle`) with shorter encodings
- Caller-register reuse for host calls (no parameter copying)
- Lazy compilation (defer translation until first call)
- Eliminated ~47% of dispatched instructions (the push/pop traffic)
- They plan to add tail-call dispatch next, calling it "clearly superior"

Research shows 25-45% fewer executed instructions for register vs stack (Shi et al., 2008). Eliminates all stack push/pop overhead. The stack bounds checks are gone entirely because there is no stack.

Could be a `std`-only optimization — micro PLCs use the stack interpreter, desktop/RPi use the register interpreter.

**Files**: New crate or module for IR translation; `vm/src/vm.rs` gets a second execution engine.

### 6. Dispatch Optimization via Tail-Call Threading

Rust's `match` compiles to a jump table, which is decent. But each iteration creates an indirect branch that the CPU branch predictor struggles with — especially on Cortex-A53's simpler predictor.

Tail-call threading: each handler directly dispatches to the next handler without returning to the loop:
```
fn handle_add(pc, stack, ...) { ...; let next_op = bytecode[pc]; DISPATCH[next_op](pc+1, stack, ...) }
```

In Rust:
- **Nightly**: `#[feature(explicit_tail_calls)]` enables `become` for guaranteed tail calls
- **Stable**: Function pointer table with `#[inline(never)]` handlers. Rust/LLVM reliably produces TCO on aarch64 in release builds — verify by checking assembly for `b`/`br` (tail call) vs `bl` (call with link register save)

PLC programs are cyclic (same code runs every scan), so branch prediction warms up after the first cycle. This reduces the benefit somewhat, but superinstructions still help by reducing total instruction count.

Expected: 10-30% on dispatch-heavy code. 1.5-2x on dispatch overhead specifically.

**Files**: `vm/src/vm.rs`

### 7. Block Memory Operations for Data Copying

Add opcodes for bulk memory operations:

- `MEMCOPY region_src region_dst len` — copy a contiguous block of variables (e.g., copying a struct/FB instance)
- `MEMZERO region len` — zero a block (variable initialization)
- `STR_MEMCOPY src_offset dst_offset` — string variable-to-variable copy without temp buffer intermediate

These avoid the overhead of N individual LOAD/STORE instructions when copying structured data. Function blocks often copy entire instances. A bulk copy opcode turns O(n) dispatches into O(1).

**Files**: `container/src/opcode.rs`, `vm/src/vm.rs`, `codegen/src/emit.rs`

### 7b. PLC-Specific Fused Operations

PLC programs have highly predictable patterns:

- **Boolean ladder logic**: Long chains of `LOAD_VAR + BOOL_AND/OR + STORE_VAR`. A fused `LOAD_AND_STORE` cuts 3 dispatches to 1.
- **Timer/counter function blocks**: TON, TOF, CTU, CTD are the most called function blocks. Specialized opcodes that operate directly on the variable table (e.g., `TON_TICK var_offset, preset_offset`) eliminate all stack traffic.
- **Input-compute-output**: The fundamental PLC pattern. Fused `LOAD_OP_STORE` instructions directly target this.

### 7c. Raw Pointer PC

Replace the `usize` PC with a raw `*const u8` pointer into the bytecode slice. This eliminates per-byte bounds checks on bytecode access (currently `bytecode[pc]` does a bounds check). The verifier already guarantees all PCs are valid instruction boundaries.

Expected: ~5-11% speedup by eliminating bounds checks on the most frequently executed code path (opcode fetch).

**Files**: `vm/src/vm.rs`

## Tier 3: Lower Impact or Longer Term

### 8. Inline Small Constants

Currently, even loading the value `0` or `1` requires a constant pool lookup. Add `LOAD_CONST_SMALL` with an inline i8 operand (2 bytes total, no pool lookup). `LOAD_TRUE` and `LOAD_FALSE` already do this for 1/0. Extending to small constants (-128..127) covers most loop counters, comparison values, and bit masks.

**Files**: `container/src/opcode.rs`, `vm/src/vm.rs`, `codegen/src/emit.rs`

### 9. Profile-Guided Opcode Ordering

Add bytecode-level profiling (count how many times each opcode executes in typical PLC programs). Reorder opcode constants so the most frequent opcodes have the lowest values (best jump table locality). Consider grouping related opcodes (all i32 arithmetic together) for cache line efficiency.

**Files**: `container/src/opcode.rs`

### 10. Precomputed Jump Targets

Currently, jumps use relative i16 offsets that require signed arithmetic at runtime. At load time (after verification), convert all jump offsets to absolute addresses stored in a side table or rewritten in-place.

**Files**: `vm/src/vm.rs`, loader

### 11. Specialized Comparison-Branch Opcodes

The common pattern `if x > 10` compiles to 4 instructions (LOAD_VAR, LOAD_CONST, GT, JMP_IF_NOT). A fused `CMP_GT_JMP_I32 var_idx, const_pool_idx, target` does the same in one dispatch. PLC programs are dominated by comparisons-then-branches (IF, CASE, WHILE). This could reduce instruction count for control flow by 75%.

**Files**: `container/src/opcode.rs`, `vm/src/vm.rs`, `codegen/src/emit.rs`

### 12. Copy-and-Patch Compilation

A technique from Haas et al. (2021) used in CPython 3.13+ (PEP 744): pre-compile each opcode handler to native code as a "stencil", then at load time, stitch stencils together by patching operand holes. Near-JIT performance (~39-63% faster than V8 Liftoff baseline compiler output) with near-interpreter implementation complexity. Code generation is 4.9-6.5x faster than V8 Liftoff.

Requires platform-specific stencils (ARM, x86), writable+executable memory (`mmap` with `PROT_EXEC`), and `std`. Not compatible with `no_std` or strict WCET analysis. Best suited as an optional desktop-only acceleration tier.

Status: evaluate after interpreter optimizations are exhausted.

## 13. Formal Methods to Eliminate Runtime Checks

The bytecode verifier (Item 1) is itself a formal method — abstract interpretation over the bytecode. More rigorous formal methods could eliminate checks that abstract interpretation alone cannot, and could provide the confidence needed to use `unsafe` unchecked operations in a safety-critical PLC runtime.

### Layer 1: Abstract Interpretation with Richer Domains

The basic verifier tracks stack depth (an integer) and validates static indices. Extending it with richer abstract domains enables eliminating more checks:

- **Interval analysis**: Track value ranges for stack slots (e.g., "this slot is in [0, 99]"). This can prove:
  - Array indices are in-bounds without runtime checks (the compiler knows array sizes)
  - Division by zero cannot occur (divisor range excludes 0)
  - Integer overflow cannot occur for specific operations (value ranges fit)
  - MUX selector `K` is in-range (eliminating the `min/max` clamp)
- **Type-state tracking**: Track which variables have been initialized, which string slots have valid headers. This eliminates the `cur_length <= max_length` defensive clamps.
- **Control-flow abstract interpretation**: Walk all paths through the bytecode, merging abstract states at join points. This is what the JVM verifier and WASM validator do.

This is the most practical formal method — it directly feeds into removing specific runtime checks. Implement directly in Rust, no external dependencies. Compatible with `no_std`.

### Layer 2: Verified Verifier via Bounded Model Checking

The critical question with Item 1 is: *how do we know the verifier is correct?* A bug in the verifier that accepts invalid bytecode, combined with unchecked execution, equals memory corruption in a PLC runtime. This is exactly the class of bugs that has plagued the eBPF verifier (CVE-2020-8835, CVE-2023-2163) and JVM verifier (CVE-2012-1723).

- **Kani** (Rust bounded model checker by AWS): Can formally verify that the verifier correctly rejects all bytecode that would cause out-of-bounds access, stack overflow, or scope violation. Kani exhaustively explores all possible bytecode inputs up to a bound. This is particularly feasible because the instruction set is small (~150 opcodes) and the verification properties are simple.
- **Prusti** (Rust verifier based on Viper): Can annotate the verifier with pre/postconditions that are formally proven.

This doesn't directly improve performance — it provides the *safety justification* for using `unsafe` unchecked operations. In a safety-first project (ADR-0005), this may be the key to unlocking Item 1.

### Layer 3: Proof-Carrying Bytecode

The compiler produces proofs alongside bytecode. The VM checks the proof (which is cheaper than re-deriving it) and then executes unchecked.

Each function's bytecode includes a "proof certificate" — a compact encoding of the abstract state at each instruction (stack depth, value ranges, type assignments). The verifier just checks that the certificate is consistent with the bytecode, rather than computing it from scratch. Verification becomes O(n) instead of O(n × join-complexity). On constrained hardware, this makes verification fast enough to run on every load.

**Precedent**: Java's StackMapTable (added in Java 6) is exactly this — the compiler provides stack maps at branch targets, and the verifier checks them instead of computing them. This simplified the JVM verifier from a complex fixed-point computation to a single linear pass.

The IronPLC compiler already tracks `current_stack_depth` and `max_stack_depth` during codegen (see `emit.rs:22-23`). Extending it to emit a compact proof certificate is natural.

### Layer 4: Property-Based Testing as a Lightweight Alternative

If full formal verification is too heavy, property-based testing (`proptest` or `quickcheck`) and fuzzing (`cargo-fuzz`) can provide high confidence:

- Generate random valid bytecode, verify it, execute it, confirm no panics
- Generate random invalid bytecode, confirm the verifier rejects it

This is not a formal method in the mathematical sense, but combined with `unsafe` code review, it may be sufficient for the current project stage.

### Formal methods recommendation

| Method | Eliminates Checks? | Provides Safety Justification? | Effort | no_std |
|--------|-------------------|-------------------------------|--------|--------|
| Abstract interpretation (Layer 1) | Yes — structural + some data-dependent | Partially | Medium | Yes |
| Kani model checking (Layer 2) | No (indirectly) | Yes — proves verifier correct | Medium | Build-time only |
| Proof-carrying bytecode (Layer 3) | No (makes verification faster) | Yes | High | Yes |
| Property-based testing (Layer 4) | No | Partially | Low | Yes |

Start with Layer 1 (richer abstract interpretation) + Layer 4 (property-based testing). Add Layer 2 (Kani) once the verifier exists to build confidence for `unsafe`. Add Layer 3 (proof-carrying) only if verification latency on constrained hardware becomes an issue.

## Priority Ordering

| # | Idea | Impact | Effort | no_std | Safety |
|---|------|--------|--------|--------|--------|
| 1 | Verification-gated unchecked ops | Very High | Medium | Yes | Maintained via verifier |
| 2 | String copy_from_slice | High (for string code) | Low | Yes | Equivalent |
| 3 | MUX stack peek (no alloc) | Medium | Low | Yes (fixes no_std bug) | Equivalent |
| 4 | Fused superinstructions | High | Medium | Yes | Maintained via verifier |
| 4b | Opcode consolidation (icache/BTB) | High | Medium | Yes | Equivalent (encoding change) |
| 7c | Raw pointer PC | Medium | Low | Yes | Requires verifier |
| 8 | Inline small constants | Medium | Low | Yes | Equivalent |
| 10 | Precomputed jump targets | Low-Medium | Low | Yes | Equivalent |
| 6 | Tail-call threading | Medium | Medium | Yes | Equivalent |
| 7 | Block memory opcodes | Medium | Medium | Yes | New verifier rules |
| 7b | PLC-specific fused ops | Medium | Medium | Yes | New verifier rules |
| 11 | Comparison-branch fusion | Medium | Medium | Yes | New verifier rules |
| 5 | Register-based translation (wasmi) | Very High | High | No (std only) | Maintained |
| 9 | Opcode ordering | Low | Low | Yes | N/A |
| 12 | Copy-and-patch | Very High | Very High | No | Maintained |
| 13a | Interval analysis in verifier | High | Medium | Yes | Eliminates data-dependent checks |
| 13b | Kani model checking of verifier | N/A (confidence) | Medium | Build-time | Proves verifier correct |
| 13c | Proof-carrying bytecode | Low (speed) | High | Yes | Faster verification on constrained HW |
| 13d | Property-based testing / fuzzing | N/A (confidence) | Low | Yes | Finds verifier bugs |

## Recommended First Steps

1. **Add benchmarks first** — without measurement, we're guessing. Create a benchmark harness with representative PLC programs (counter loop, arithmetic-heavy, string-heavy, branching).
2. **Items 2 & 3** — quick fixes, immediate improvement, fix no_std compatibility.
3. **Item 1** — the biggest win, but requires the verifier. If the verifier is partially built, start with the properties it already checks.
4. **Items 4 & 8** — superinstructions and inline constants, moderate effort, clear benefit.
