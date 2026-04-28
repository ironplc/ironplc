# Opcode Encoding by Operation Class and Type Tag

status: proposed
date: 2026-04-28

## Context and Problem Statement

The bytecode VM dispatches instructions through a flat `match` over 123 opcode constants in `compiler/vm/src/vm.rs`. After the or-pattern collapses already done for `LOAD_VAR` / `STORE_VAR`, the dispatch has roughly 96 distinct outer arms. Each arm is its own indirect-branch target.

Profiling data (`compiler/benchmarks/tests/profile_for_loop.rs`) shows the user's reference FOR loop dispatches 1516 ops per scan with the top 5 opcodes covering 80%. Per-dispatch cost on this workload is multiplied by:

- BTB pressure from ~96 distinct outer-match targets
- `execute()` machine-code size (per-arm prologue/epilogue replicated across many type-specific arms)
- L1i pressure during the dispatch loop

The flat encoding also makes future additions structurally awkward. Adding a fused superinstruction like `INC_VAR` requires four new opcodes (one per integer width). Adding a new type extension like WSTRING requires duplicating ~15 string opcodes (one per existing string operation × narrow/wide). Both extensions consume opcode bytes by type-cross-product, not by structural unit.

## Decision

Restructure the opcode byte to encode operation class in the high 6 bits and type variant in the low 2 bits:

```
opcode byte:  [op_class:6][type:2]
```

The Rust dispatch becomes a two-level match: outer on op-class, inner on type-tag. The container's `FORMAT_VERSION` bumps from 1 → 2; old `.iplc` files reject cleanly.

The encoding bakes in three rules that govern future capacity:

- **Op class encodes "what operation."** 64 slots total. Scarce, used for distinct top-level operations.
- **Type tag encodes "what kind of data."** 4 slots per op class. Plentiful in aggregate; used for genuine data-shape variation (width, signedness, int/float, narrow/wide).
- **Sub-opcode (in operand bytes) encodes "which family member."** Used when a family of structurally similar operations shares an op class — currently the string family (15 ops behind one `STRING_OP` slot).

Family consolidation under sub-opcode dispatch is mandatory, not optional: without folding `BOOL_OP`, `STACK_OP`, `FB_OP`, `ARRAY_OP`, and `STRING_OP` families, the op-class count is ~66 and exceeds the 64-slot cap.

After the change, ~41 of the 64 op-class slots are used. The remaining 23 are headroom for fused superinstructions (`INC_VAR`, `LOAD_ADD_STORE`, `BR_*_VAR_IMM`) and any future top-level operations. WSTRING-style type extensions cost zero op-class slots: WSTRING becomes `STRING_OP` with `type_tag = 1`.

## Considered Alternatives

### Keep the flat encoding

Simplest. No format change. But the ~96 outer-arm BTB load doesn't shrink, and every future fusion or type extension consumes opcode bytes by type-cross-product. With 133 free byte values today, that ceiling looks generous, but it's structurally hostile: claiming one byte for `INC_VAR_I32_TRUNC` does not also reserve `INC_VAR_I64`/`INC_VAR_DINT`/etc. The "free 133" is unstructured space.

### 5+3 encoding (32 op classes × 8 type variants)

More type-tag headroom per class — `DIV` could hold all 6 variants (signed/unsigned int + float) in one class instead of splitting into `DIV_S`/`DIV_U`. But 32 op-class slots is too tight: even with mandatory family consolidation, the count is ~30-31, leaving ~1-2 slots for future fusion. Future extensions would have to land in an "extended" op-class with two-byte encoding, pushing common operations behind a third dispatch layer.

### Function-pointer table

Replace the `match` with `DISPATCH[opcode](state)`. Compact source. But the indirect call per dispatch (call/ret, possible icache miss on the callee) likely costs more than a `match` jump-table on modern Rust/LLVM, especially after the encoding reorganization shrinks the outer match. Not measured; deferred unless `match`-based dispatch shows a hard ceiling.

### Two-byte opcode

Lifts the 256-byte ceiling entirely. Permanently doubles the bytecode-fetch cost for every instruction. The 1-byte opcode is a hard requirement for embedded targets (Cortex-M, etc.) where bytecode-stream throughput dominates.

## Consequences

**Guaranteed by the encoding alone:**

- Outer-match arm count drops from ~96 to ~41. BTB working set on the dispatch loop's hot indirect branch shrinks proportionally.
- 23 op-class slots open up for future top-level operations.
- Future type-variant extensions (WSTRING, additional integer widths, fixed-point types) cost zero op-class slots.

**Speculative — measured, not asserted:**

- Total `execute()` machine-code size reduction. The `vm-performance.md` §4b table predicts ~18KB → ~6-8KB, which depends on LLVM deduplicating per-arm prologue/epilogue code. Whether that deduplication actually happens is a question for the disassembler.
- Wall-clock improvement on real workloads. Comes from BTB and possibly L1i; relative contribution is platform-specific.

The implementation plan (`specs/plans/2026-04-28-opcode-encoding-reorganization.md`) requires baseline measurement before changes and post-change re-measurement to validate the size prediction. If the size reduction is small (<20%), the BTB win and structural headroom still justify the change but the L1i story is muted; we should not assume further BTB-only optimizations will compound usefully.

**Costs:**

- `DIV` / `MOD` / `LT` / `LE` / `GT` / `GE` split into signed/unsigned op-class pairs because they need 6 type variants and only 4 fit per class. Adds 6 op-class slots to the count, but the split is semantically clean (signed and unsigned int division genuinely use different CPU instructions).
- `STRING_OP`, `FB_OP`, `ARRAY_OP`, `BOOL_OP`, `STACK_OP` family consolidations add an inner sub-opcode dispatch. These op classes are not on any FOR-loop hot path; the string family is the only one that's hot in string-heavy programs, and string handlers are already large enough that the sub-opcode dispatch is a small relative cost.
- Bytecode format break. The user has explicitly waived backwards compatibility for this work.
- Test migration: ~445 raw-hex bytecode literals across ~30 VM test files need conversion to use named constants. The plan defers this work behind a cargo feature gate (`legacy_bytecode_tests`) until after the post-change measurement validates the encoding is worth keeping.

## References

- Plan: `specs/plans/2026-04-28-opcode-encoding-reorganization.md`
- Design: `specs/design/vm-performance.md` §4b "Opcode Consolidation to Reduce Instruction Cache Pressure" (Option A is what this ADR adopts).
- Measurement instrument: `compiler/benchmarks/tests/profile_for_loop.rs`.
- ADR-0006 (verification requirement) — not implemented here, but the encoding's structural validity check (valid op-class, zero type bits on untyped ops, valid sub-opcode for family ops) is a partial form of what the verifier will eventually do.
- ADR-0005 (safety-first) — encoding-only change, no `unsafe`, no safety implications.
- ADR-0010 (no_std VM for embedded targets) — the 1-byte opcode is preserved; this change is no_std-compatible.
