# Bytecode Verification as a Requirement

status: proposed
date: 2026-02-18

## Context and Problem Statement

The bytecode instruction set assumes well-formed bytecode: valid opcodes, in-bounds operand indices, consistent stack depths at branch merge points, and correct types on the operand stack. If any of these assumptions are violated — whether by a compiler bug, in-transit corruption, or a deliberate attack — the VM could crash, corrupt memory, or execute unintended operations.

Should the VM trust that bytecode is well-formed (as produced by a trusted compiler), or must it verify bytecode before execution?

## Decision Drivers

* **Attack precedents** — Stuxnet (2010) replaced PLC bytecode to cause physical damage; Rogue7 (2019) demonstrated downloading arbitrary bytecode to S7-1500 PLCs; CVE-2022-1161 (CVSS 10.0) showed bytecode/source decoupling on Rockwell controllers
* **VM precedents** — Lua does not verify bytecode, leading to trivial RCE from crafted bytecode ("Pwning Lua through load", saelo 2017); the Factorio game's custom Lua verifier had off-by-one bugs leading to RCE (CVE in Factorio < 1.1.101); eBPF's verifier has had multiple bypass vulnerabilities (CVE-2020-8835, CVE-2023-2163), but each was a localized bug, not a fundamental design failure
* **PLC deployment model** — bytecode travels from an engineering workstation to the PLC over a network; the channel may not be trusted; the PLC may run for years without human inspection
* **Resource constraints** — micro PLCs (Cortex-M0, 32 KB RAM) may not have sufficient resources to run a full verifier on-device
* **Defense-in-depth** — even with a signature (ADR-0007), a verifier catches compiler bugs and provides a second layer of assurance

## Considered Options

* Trust the compiler — no verification (Lua's approach)
* Verify off-device only — the engineering workstation verifies, the PLC trusts the signed result (Java Card's approach)
* Verify on-device — the PLC verifies bytecode at load time before execution
* Verify on-device with signature fallback — verify on-device when resources allow; accept signed-but-unverified bytecode on constrained targets

## Decision Outcome

Chosen option: "Verify on-device with signature fallback", because it provides the strongest guarantee on capable hardware while remaining deployable on micro PLCs.

The verification requirement is:

1. **Every PLC must either verify bytecode on-device at load time, or validate a cryptographic signature from a trusted verifier.** There is no mode where unverified, unsigned bytecode executes.
2. **The verifier is a separate pass** that runs before the interpreter. It produces a pass/fail result. The interpreter refuses to run bytecode that has not been verified or signature-validated.
3. **The verifier rules are normative** — they are defined in a separate specification (Bytecode Verifier Rules) that an implementer can test against.

### Consequences

* Good, because crafted bytecode cannot reach the interpreter — even a zero-day in the interpreter is unexploitable if the verifier rejects the crafted input
* Good, because compiler bugs are caught at the PLC, not silently executed for years
* Good, because the signature fallback makes this deployable on micro PLCs where on-device verification is too expensive
* Good, because the verifier spec is testable — it can be fuzzed and formally verified independently of the interpreter
* Bad, because the verifier adds flash size (~4-8 KB) and load-time latency (~milliseconds for typical programs)
* Bad, because the signature fallback trusts the engineering workstation's verifier — a compromised workstation can sign malicious bytecode that bypasses on-device verification
* Bad, because the verifier itself is a complex component that can have bugs (eBPF demonstrates this); however, a verifier bug is a localized issue, while no-verification is a systemic issue
* Neutral, because the verifier runs once at load time, not per-scan-cycle, so its performance cost is amortized

### Confirmation

Verify by:
1. Implementing the verifier as a standalone component with 100% branch coverage
2. Fuzzing the verifier with millions of random bytecode inputs — it must never crash, and must reject all malformed inputs
3. Confirming that the interpreter refuses to execute bytecode that has not passed verification
4. Testing the signature fallback path: signed bytecode from a trusted verifier is accepted without on-device verification

## Pros and Cons of the Options

### Trust the Compiler (No Verification)

The VM assumes all bytecode is well-formed. No verification pass, no signature check.

* Good, because load time is minimal — bytecode is executed immediately
* Good, because the VM implementation is simpler — no verifier component
* Bad, because this is the Lua approach, and it leads to trivial arbitrary code execution from crafted bytecode
* Bad, because compiler bugs silently produce malformed bytecode that corrupts VM state
* Bad, because any network-level attacker who can modify bytecode in transit achieves arbitrary code execution
* Bad, because this is the approach that enabled Stuxnet, Rogue7, and CVE-2022-1161

### Verify Off-Device Only

The engineering workstation runs the verifier. The PLC receives signed bytecode and trusts the signature.

* Good, because the PLC needs no verifier — saves flash and RAM on constrained targets
* Good, because verification runs on powerful hardware with no resource constraints
* Bad, because this is the Java Card approach, and Java Card researchers demonstrated that modifying bytecode after off-card verification defeats all guarantees (Mostowski & Poll, CARDIS 2008)
* Bad, because a compromised engineering workstation (Rogue7 attack model) can sign arbitrary bytecode
* Bad, because there is no defense-in-depth — if the signature is valid, malicious bytecode executes

### Verify On-Device

The PLC verifies bytecode at load time. No signature fallback.

* Good, because the PLC is self-sufficient — it does not trust any external system
* Good, because defense-in-depth is maximized — both the verifier and the interpreter must be compromised for exploitation
* Bad, because micro PLCs may not have sufficient RAM for the verifier's working set (~2 KB per function for type state tracking at merge points)
* Bad, because mandatory on-device verification excludes the smallest hardware targets

### Verify On-Device with Signature Fallback (chosen)

Verify on-device when resources allow. On constrained targets, accept bytecode signed by a trusted verifier.

* Good, because capable hardware gets full on-device verification — no trust in external systems
* Good, because constrained hardware can still run — it trusts signed bytecode from the engineering workstation
* Good, because the signature requirement means even constrained targets reject unsigned/tampered bytecode
* Good, because the two paths (verify vs. trust-signature) can be tested independently
* Bad, because the signature fallback path still trusts the engineering workstation — it is weaker than on-device verification
* Neutral, because the VM must implement both paths (verifier + signature validation), increasing total code size — but on a micro PLC, only the signature path is needed, so the verifier code is not included in the micro build

## More Information

### What the verifier must check

The full verifier rules are defined in a separate specification. The high-level requirements are:

1. **Opcode validity** — every opcode byte is a defined opcode; undefined bytes (0x00, 0x09, 0xFA, 0xFB, 0xFF, etc.) are rejected
2. **Operand bounds** — every u16 variable index, constant pool index, and function ID references a valid entry in the corresponding table
3. **Stack depth consistency** — at every branch merge point (jump targets, loop headers), the stack depth is identical on all incoming paths
4. **Stack type consistency** — at every merge point, the type of every stack slot is compatible on all incoming paths
5. **No stack underflow** — no instruction pops from an empty stack on any reachable path
6. **No stack overflow** — the maximum stack depth (declared in the container header) is not exceeded on any path
7. **Jump target validity** — every jump offset lands on a valid instruction boundary within the current function; no jumps into operand bytes or outside the function
8. **Call target validity** — every CALL and FB_CALL references a valid function or FB type
9. **Field index validity** — every FB_STORE_PARAM, FB_LOAD_PARAM, LOAD_FIELD, and STORE_FIELD field index is within bounds for the target type
10. **Array type consistency** — every LOAD_ARRAY/STORE_ARRAY type byte matches the declared element type of the array variable
11. **Return path completeness** — every function has a RET or RET_VOID on all paths
12. **Call depth** — the static call graph does not exceed the declared maximum call depth

### Verification timing and TOCTOU

The verifier runs at bytecode load time — after the bytecode is copied into the VM's memory but before execution begins. To prevent time-of-check-time-of-use (TOCTOU) attacks (where bytecode is modified after verification), the bytecode memory should be made read-only after verification completes. On platforms that support memory protection (Cortex-M with MPU, Linux with mprotect), this is straightforward. On platforms without memory protection, the signature serves as the integrity guarantee.

### Resource cost of on-device verification

| Resource | Estimated cost | Notes |
|---|---|---|
| Flash (verifier code) | 4–8 KB | Comparable to a soft-float library |
| RAM (verifier working set) | ~2 KB per function | Type state at merge points; freed after verification |
| Time (verification pass) | ~1 ms per 1 KB of bytecode | One-time cost at load time; not per-scan |

On a micro PLC with 32 KB RAM and 128 KB flash, the verifier is feasible but tight. The signature fallback exists for targets where it is not.
