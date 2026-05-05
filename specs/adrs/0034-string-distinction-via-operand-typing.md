# STRING/WSTRING Distinction via Operand Typing and Runtime Encoding Tags

status: proposed
date: 2026-05-05
supersedes: ADR-0004

## Context and Problem Statement

ADR-0004 chose three distinct type families with separate opcodes (STR_* and WSTR_*) for STRING and WSTRING variable access, and distinct func_id ranges within BUILTIN for string functions. Two architectural changes since that decision require revisiting the mechanism while preserving the safety principle:

1. **ADR-0017 dissolved the buffer table.** ADR-0004's defense-in-depth mechanism — a one-byte encoding tag on each entry of the buffer table — has no place to live now that string storage is in the unified data region addressed by `data_offset` rather than `buf_idx`.

2. **The opcode budget is tighter than ADR-0004 anticipated.** Adding a parallel WSTR_* opcode family for variable access and string functions consumes ~16 opcode slots out of the ~85 free. Combined with dispatch-table size impact on icache for embedded targets, this is more cost than ADR-0004's analysis estimated, particularly for resource-constrained micro PLCs.

The principle of ADR-0004 — STRING and WSTRING values must never be silently confused, with safety enforced statically and reinforced at runtime — must be preserved. Only the mechanism is at issue.

## Decision Drivers

* **Preserve ADR-0004's safety principle** — type confusion between STRING and WSTRING corrupts data silently; PLC programs control physical processes, so silent corruption can cause physical damage (ADR-0005)
* **Defense in depth** — even if the bytecode verifier has a bug or is bypassed, runtime checks should detect type confusion before incorrect data is propagated
* **Opcode budget** — the bytecode instruction set has ~85 free slots; doubling the string opcode family for WSTRING is a meaningful fraction
* **Dispatch-table footprint** — embedded targets are sensitive to icache pressure from large interpreter dispatch tables
* **Source-code deduplication** — STRING and WSTRING operations differ only in character width; a design that requires duplicating ~16 handlers is a maintenance hazard, even if helpers can share the body

## Considered Options

* Three Distinct Type Families with separate opcodes (ADR-0004 original)
* Single Generic `ref` with Runtime Type Tags (ADR-0004's rejected option)
* Single STR_* Opcode Family with Operand Typing and Runtime Tags

## Decision Outcome

**Chosen option: Single STR_\* opcode family with operand typing and runtime encoding tags.**

Width information lives with the data, not with the opcode. Type safety is enforced through three layers:

### Layer 1: Static Verification at the Operand Level

Each STR_* opcode operand resolves to a specific data location with a known encoding:

- A `data_offset` operand identifies a variable in the slot table; the slot's declared type (STRING or WSTRING) is fixed at compile time and recorded in the slot table.
- A `pool_index` operand identifies a constant pool entry; the entry's encoding is fixed at compile time and stored alongside the bytes.
- A `buf_idx` value popped from the stack identifies a temp buffer slot; the slot's encoding was set when the buffer was allocated by an opcode that wrote a typed entry.

The bytecode verifier proves that every consumer's expected encoding matches the producer's actual encoding by tracing operands to their typed source. This is **local reasoning per opcode** — not flow-sensitive abstract interpretation — well within a tractable verifier.

### Layer 2: Runtime Encoding Tags at Three Sites

- **String header in the data region** carries `char_width: u16` (see ADR-0035, which supersedes ADR-0015).
- **Constant pool entries** carry an encoding tag (e.g., `PoolConstant::Str(Vec<u8>, char_width)`).
- **Temp buffer slots** carry a one-byte encoding tag, set at allocation, asserted on every operation that consumes the buffer.

On every string operation, the VM compares the source's actual encoding to the destination's expected encoding; mismatch traps. The runtime check cannot be optimized away — it is a mandatory safety invariant.

### Layer 3: Single Set of Opcode Handlers, Parameterized by Width

Each STR_* opcode handler dispatches to a shared helper function with `char_width` derived from the data's encoding tag. Source code has one body per logical operation; behavior differs only in scaling byte offsets by `char_width`. STR_* and WSTR_* paths cannot drift apart through independent edits, because there is only one path.

### Why This Is Not the Single-Generic-Ref Option ADR-0004 Rejected

ADR-0004 rejected polymorphic dispatch with runtime tags because the verifier would need to track type tags through *abstract interpretation* — the eBPF verifier failure mode (CVE-2020-8835, CVE-2023-2163). The new design avoids that failure mode in three ways:

1. **The verifier does not track tags through arbitrary computation.** Each operand's type comes from a typed source: a slot table entry, a constant pool entry, or a temp buffer slot. The lookup is O(1) per operand.

2. **Width determination is local to each opcode**, not flow-sensitive. There is no need to track abstract values along execution paths.

3. **The runtime defense-in-depth check makes verifier mistakes detectable at execution time**, not silently corrupting. Even if a verifier bug allows a typed-mismatch operation through, the runtime trap catches it.

This is closer in spirit to ADR-0004's middle option ("Two Reference Types with Polymorphic String Dispatch") than to either extreme — but with the runtime check elevated from defense-in-depth-only to an enforced invariant.

### Consequences

* Good, because the opcode budget is preserved — no new STR_*/WSTR_* opcode pairs
* Good, because the dispatch table does not grow — same icache footprint as STRING-only support
* Good, because each string operation has one source-code body — STR_* and WSTR_* paths cannot drift apart
* Good, because the runtime encoding check provides defense-in-depth that ADR-0004 wanted, in a location consistent with ADR-0017's data region architecture
* Good, because the verification chain is local-per-opcode rather than flow-sensitive — avoiding the eBPF verifier failure mode
* Neutral, because the runtime check costs one comparison per string operation entry — well below the cost of the operation itself
* Bad, because the verifier must be implemented to track operand types from their typed sources; the verification logic is more involved than "look at the opcode"
* Bad, because the static type safety guarantee depends on the verifier being correct; mitigated by the runtime tag check

### Confirmation

Verify by:

1. **Verifier tests** that reject bytecode consuming a STRING-tagged constant pool entry with operations expecting a WSTRING source (and vice versa), without relying on runtime trapping.
2. **Runtime tests** that confirm encoding-mismatch trapping works when verifier checks are deliberately bypassed (synthetic mismatched bytecode loaded directly).
3. **End-to-end execution tests** demonstrating WSTRING declarations, assignments, comparisons, and string-builtin calls (LEN, CONCAT) work correctly.
4. **Source review** that each STR_* opcode handler delegates to a shared, width-parameterized helper rather than duplicating logic.

## Pros and Cons of the Options

### Three Distinct Type Families (ADR-0004 original)

Separate STR_* and WSTR_* opcodes for variable access; func_id ranges for string functions.

* Good, because type safety is enforced by the opcode itself — verifier work is minimal
* Good, because the dispatch site encodes width as a compile-time constant — no per-op width lookup
* Bad, because it consumes ~16 opcode slots — a meaningful fraction of the ~85 free
* Bad, because the dispatch table grows for embedded targets
* Bad, because each operation has two source bodies (STR_* and WSTR_*) that can drift apart through independent edits — mitigated only by careful refactoring into shared helpers
* Bad, because the defense-in-depth tag was specified for buffer table entries which no longer exist post-ADR-0017; relocation would be ad-hoc

### Single Generic `ref` with Runtime Type Tags (ADR-0004's rejected option)

A single `ref` type on the stack with type tags that operations check at runtime, without any static verification.

* Bad, because the verifier must track tags through abstract interpretation — the eBPF verifier failure mode
* Bad, because every operation is polymorphic; even when the program statically uses only STRING, the dispatch is the wide path
* Bad, because tag corruption silently misinterprets data with no defense-in-depth

### Single STR_* Opcode Family with Operand Typing and Runtime Tags (chosen)

See "Decision Outcome" above. The verifier proves operand-source-typing statically; the runtime tag enforces the invariant; opcode count and source surface stay constant.

## More Information

### Relationship to ADR-0017

ADR-0017 places all string storage in the unified data region. This ADR is consistent: STR_* opcodes operate on offsets into that region. The encoding of the bytes at each offset is described by the string header (see ADR-0035).

### Relationship to ADR-0035

ADR-0035 (which supersedes ADR-0015) specifies the per-string memory layout including the `char_width` field. This ADR specifies how the VM uses that field — both as an enforced invariant (runtime trap on mismatch) and as the data on which width-parameterized helpers operate.

### Relationship to ADR-0016

ADR-0016 specifies UTF-16LE for WSTRING and Latin-1 for STRING. This ADR is consistent. The encoding tag on a string distinguishes these cases; the byte layout for each encoding is unchanged.

### Verifier Implementation Note

The verifier must, for each STR_* opcode, look up the encoding of every typed source operand and confirm it matches the operation's expectations. The lookups are O(1) and the checks are local — no symbolic execution, no value tracking across opcodes. This keeps the verifier simple enough to audit, in contrast to the eBPF-style verifier that ADR-0004 was right to reject.
