# Safety-First Resolution of Design Trade-Offs

status: proposed
date: 2026-02-18

## Context and Problem Statement

The bytecode instruction set design repeatedly encounters trade-offs between opcode economy (fewer opcodes, simpler interpreter) and safety (stronger type checking, more invariants enforced by the VM). These trade-offs recur across different areas: type representation, array access, control flow, string handling, and arithmetic. Rather than resolving each trade-off independently, should the project adopt an explicit standing principle for how to resolve them?

## Decision Drivers

* **PLC programs control physical processes** — a VM bug that corrupts memory or silently produces wrong values can cause physical damage (equipment destruction, safety hazards)
* **PLC programs run unattended for years** — a latent bug triggered by a rare input combination has years of opportunity to manifest, unlike desktop software that is frequently restarted
* **Opcode budget has room** — at 157 of 256 opcodes used (after ADR-0008 consolidation), there are 99 slots available; economy is not a binding constraint
* **Interpreter complexity is manageable** — the interpreter runs in Rust, so additional dispatch handlers add flash size but not memory safety risk
* **Verification is planned** — a bytecode verifier (ADR-0006) relies on the instruction set encoding enough type information to verify statically; safety-oriented design directly reduces verifier complexity

## Considered Options

* No standing principle — resolve each trade-off on its own merits
* Economy-first — minimize opcode count, push safety to the verifier and runtime
* Safety-first — when in doubt, encode type information and invariants in the opcode, even at the cost of more opcodes

## Decision Outcome

Chosen option: "Safety-first", because the cost of a VM bug in a PLC context is physical damage, while the cost of extra opcodes is a slightly larger interpreter binary.

The principle is: **when a design choice improves safety (verification, type checking, bounds enforcement) at the cost of opcode count or interpreter complexity, take the safety option.** This is a standing policy, not a one-time decision. It applies to future instruction set extensions.

### How this principle was applied

| Trade-off | Safety choice | Alternative | Opcodes spent |
|---|---|---|---|
| STRING vs WSTRING | Separate type families (ADR-0004), now via distinct BUILTIN func_id ranges (ADR-0008) | Polymorphic dispatch with runtime tag | +1 (BUILTIN opcode; distinct func_id ranges preserve type safety) |
| Array access | Dedicated LOAD_ARRAY/STORE_ARRAY with mandatory bounds checking | Computed offsets via arithmetic | +2 |
| TIME arithmetic | Dedicated TIME_ADD/TIME_SUB with type enforcement | Raw I64 arithmetic | +2 |
| CASE compilation | JMP_IF chains (no TABLE_SWITCH) | TABLE_SWITCH opcode | +0 (avoided complexity) |
| Exponentiation | Library call (not an opcode) | Dedicated EXPT opcode | +0 (avoided complexity) |

### Consequences

* Good, because every type distinction is statically verifiable from the opcode stream — the verifier is simpler and more trustworthy
* Good, because the VM enforces invariants (bounds, types) even if the verifier has a bug — defense-in-depth
* Good, because the principle provides a clear, repeatable decision framework for future extensions — contributors don't need to re-derive the reasoning each time
* Good, because the opcode budget (99 remaining slots after ADR-0008) can absorb many more safety-oriented additions before becoming a constraint
* Bad, because the interpreter binary is larger than a minimal design — estimated ~10 KB additional flash for the WSTRING family and array/TIME handlers
* Bad, because the principle occasionally rejects designs that are safe in practice but not provably safe by the verifier (e.g., polymorphic string ops with correct runtime tags are safe, but we reject them because "correct runtime tags" is an assumption, not a proof)
* Neutral, because this principle can be overridden for specific decisions when the safety cost is genuinely minimal and the economy benefit is large — it is a default, not an absolute rule

### Confirmation

For any future instruction set change, apply this checklist:
1. Does the proposed change encode a type or invariant that was previously implicit?
   - If yes, prefer the change (safety-first).
2. Does the proposed change rely on the compiler always producing correct output?
   - If yes, prefer an alternative that the VM can verify independently (defense-in-depth).
3. Does the proposed change add runtime complexity that the verifier cannot check statically?
   - If yes, prefer a design where the verifier handles it statically (simpler runtime).

## Pros and Cons of the Options

### No Standing Principle

Each trade-off resolved individually based on the specific context.

* Good, because each decision is optimized for its own context
* Bad, because decisions become inconsistent — some areas favor economy, others favor safety, with no clear reason for the difference
* Bad, because contributors must re-derive the reasoning for every new trade-off
* Bad, because inconsistency in type encoding creates gaps that a verifier must work around

### Economy-First

Minimize opcode count. Push type safety to the verifier (static analysis before execution) and runtime checks (defense-in-depth after verification).

* Good, because the instruction set is compact and the interpreter is small — better for flash-constrained micro PLCs
* Good, because a powerful verifier can compensate for a simpler instruction set
* Bad, because verifier bugs become critical — the eBPF verifier (CVE-2020-8835, CVE-2023-2163) and JVM verifier (CVE-2012-1723, CVE-2017-3289) demonstrate that sophisticated verifiers have exploitable bugs
* Bad, because runtime checks (type tags, bounds assertions) add per-instruction overhead that static type encoding avoids
* Bad, because the verifier must track more state (mutable type tags, value ranges) making it more complex and harder to audit

### Safety-First (chosen)

When in doubt, encode type information and invariants in the opcode. Accept more opcodes in exchange for stronger static guarantees.

* Good, because the verifier is simpler — it checks opcode-level properties, not inferred properties
* Good, because the VM enforces invariants even if the verifier is bypassed (defense-in-depth)
* Good, because the principle scales — it applies equally to current and future decisions
* Bad, because the interpreter is larger (more opcodes, more handlers)
* Bad, because some safe-in-practice designs are rejected in favor of provably-safe designs

## More Information

### Why PLC context makes safety-first the right default

In web/desktop VM design (JVM, V8, Wasm), economy-first is common because:
- The runtime is large anyway (GC, JIT, standard library)
- Security bugs can be patched quickly (auto-update)
- The worst case is data breach, not physical damage

In PLC VM design, these assumptions are inverted:
- The runtime should be small (embedded targets)
- Firmware updates are infrequent and require maintenance windows
- The worst case is physical damage to equipment or harm to people

This makes the cost of a VM bug much higher and the cost of extra opcodes much lower in relative terms. Safety-first is the correct default for this domain.
