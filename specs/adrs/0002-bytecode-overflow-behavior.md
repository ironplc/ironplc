# Configurable Overflow Behavior at Narrowing Points

status: proposed
date: 2026-02-17

## Context and Problem Statement

When the bytecode VM narrows a 32-bit arithmetic result back to a sub-32-bit type (e.g., I32 to SINT), the result may not fit in the target range. IEC 61131-3 does not mandate a specific overflow behavior — it states that exceeding a type's range is an "error" without specifying whether the runtime should wrap, saturate, or fault. Different PLC runtimes handle this differently, and programs ported between runtimes may depend on a specific behavior.

How should the IronPLC bytecode VM handle integer overflow, and should it be configurable for cross-runtime compatibility?

## Decision Drivers

* **IEC 61131-3 does not mandate overflow behavior** — the standard leaves this implementation-defined, so any choice is compliant
* **Real PLC programs depend on specific overflow behavior** — programs written for CODESYS (wrapping) will produce wrong results on a saturating runtime, and vice versa
* **Cross-runtime compatibility is a user need** — users porting programs from CODESYS, Siemens S7, or other runtimes expect matching behavior
* **Safety-critical applications require fault-on-overflow** — SIL-rated PLC applications must detect overflow rather than silently producing wrong values
* **Performance** — overflow checking adds cost; wrapping is essentially free on two's complement hardware

## Considered Options

* Always wrap (two's complement modular arithmetic)
* Always fault (runtime error on overflow)
* Configurable overflow policy per VM instance

## Decision Outcome

Chosen option: "Configurable overflow policy per VM instance", because it enables cross-runtime compatibility while keeping the instruction set fixed. The NARROW instructions (NARROW_I8, NARROW_I16, NARROW_U8, NARROW_U16) apply the VM's configured overflow policy. The policy is set once at VM startup and applies uniformly to all narrowing operations.

The default policy is **wrap** (two's complement modular arithmetic for signed, modular reduction for unsigned), matching the most common PLC runtime behavior (CODESYS, TwinCAT, Allen-Bradley).

### Consequences

* Good, because programs ported from CODESYS/TwinCAT work correctly with the default (wrap) policy
* Good, because programs ported from Siemens S7 can use saturate mode for compatible behavior
* Good, because safety-critical applications can use fault mode to detect all overflows
* Good, because the instruction set is fixed — the policy is a VM configuration parameter, not encoded in bytecode, so the same compiled program can run under different policies
* Bad, because programs that depend on wrapping behavior may silently produce different results if someone switches the VM to saturate mode without testing
* Bad, because the configurable policy adds a conditional branch to every NARROW execution (check which policy is active), though this can be mitigated with function pointers or compile-time monomorphization

### Confirmation

Verify with tests that the same NARROW_I8 instruction:
1. Under wrap mode: `NARROW_I8(150)` produces `-106` (150 - 256)
2. Under saturate mode: `NARROW_I8(150)` produces `127` (clamped to SINT max)
3. Under fault mode: `NARROW_I8(150)` produces a runtime error

Verify with a multi-step expression test that intermediate overflow behavior is correct:
- Expression: `SINT: 100 + 50 - 30`
- Under wrap mode: `100 + 50 = 150` (I32, no overflow) → NARROW_I8 → `-106` → promoted back to I32 → `-106 - 30 = -136` → NARROW_I8 → `120`
- This matches native SINT wrapping behavior step-by-step only if NARROW is emitted after each sub-expression that stores to a SINT, which the compiler must ensure

## Pros and Cons of the Options

### Always Wrap

All integer overflow uses two's complement wrapping for signed types and modular reduction for unsigned types. NARROW instructions unconditionally truncate to the target width.

* Good, because it is the fastest option — truncation is a single bitwise AND or cast, no conditional logic
* Good, because it matches the most common PLC runtimes (CODESYS, TwinCAT, Allen-Bradley)
* Good, because two's complement wrapping is mathematically well-defined and predictable
* Bad, because programs from runtimes with different overflow behavior (Siemens S7 in saturating mode) will produce wrong results
* Bad, because safety-critical applications cannot detect overflow — wrapping silently produces a "valid" but wrong value

### Always Fault

Any narrowing conversion that produces a value outside the target type's range raises a runtime error and halts the PLC program.

* Good, because all overflows are detected — no silent data corruption
* Good, because it is the safest option for SIL-rated applications
* Bad, because many correct PLC programs intentionally use wrapping behavior (e.g., free-running counters, cyclic timers)
* Bad, because it makes the runtime incompatible with the majority of existing PLC programs
* Bad, because a runtime error in a PLC can mean uncontrolled shutdown of a physical process, which may itself be unsafe

### Configurable Overflow Policy Per VM Instance

The VM accepts an overflow mode at startup: `wrap`, `saturate`, or `fault`. All NARROW instructions use this policy. The policy is uniform across the entire VM instance.

* Good, because it handles all use cases — compatibility, safety, and default behavior
* Good, because the bytecode is policy-agnostic — the same compiled program works under any policy
* Good, because the default (wrap) is the fastest and most common, so typical usage pays no extra cost if the VM implementation specializes
* Neutral, because per-VM granularity means you cannot mix policies within a single program (e.g., wrapping for a timer counter but faulting for a safety-critical calculation); this is a reasonable limitation for a first implementation
* Bad, because adding configurability increases testing surface — each NARROW instruction must be tested under all three policies

## More Information

### Overflow behavior of real PLC runtimes

| Runtime | Default overflow behavior | Notes |
|---------|--------------------------|-------|
| CODESYS / TwinCAT | Wrap (two's complement) | No overflow detection by default |
| Siemens S7-300/400 | Wrap, sets OV/OS status bits | Programs can check status bits to detect overflow |
| Siemens S7-1500 | Configurable: wrap or fault | Global setting in CPU configuration |
| Allen-Bradley (Logix) | Wrap, sets S:V overflow flag | Fault handler can be configured |
| B&R Automation Studio | Wrap | Similar to CODESYS |
| Safety PLCs (general) | Fault | Required by IEC 61508 (SIL) |

### Interaction with ADR-0001

ADR-0001 defines the two-width arithmetic model where sub-32-bit types are promoted to 32-bit for arithmetic. The NARROW instructions defined there are the exact point where this ADR's overflow policy is applied. The separation is clean: ADR-0001 decides *when* narrowing occurs (at every store to a sub-32-bit variable), and this ADR decides *how* narrowing handles out-of-range values.

### Promotion-width arithmetic and intermediate overflow

Because arithmetic happens at 32-bit width, intermediate values in a complex expression may have different values than they would in native-width arithmetic when the overflow policy is not wrap. For example:

```
(* x is SINT, range -128..127 *)
x := 100 + 50 - 30;
```

With two-width arithmetic and wrap-on-narrow:
1. `100 + 50 = 150` (I32, fits)
2. If the compiler emits NARROW_I8 here (because the intermediate is stored): `150` wraps to `-106`
3. `-106 - 30 = -136` (I32, fits)
4. NARROW_I8: `-136` wraps to `120`

With native SINT wrapping:
1. `100 + 50 = 150` wraps to `-106`
2. `-106 - 30 = -136` wraps to `120`

The results match. For wrapping, this is guaranteed by the properties of modular arithmetic — the low bits of the result are the same regardless of when truncation occurs. However, the compiler should still emit NARROW instructions at statement boundaries (not within sub-expressions) to minimize unnecessary narrowing, since for wrapping the final truncation is sufficient.

For saturating or fault policies, intermediate narrowing changes results (as analyzed in the design discussion). The compiler must emit NARROW after every operation where IEC 61131-3 semantics require the result to be in the target type's range — which is at every assignment, not within expressions.
