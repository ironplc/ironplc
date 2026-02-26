# Trap on Missing RET_VOID at End of Bytecode

status: proposed
date: 2026-02-26

## Context and Problem Statement

The VM's `execute()` function runs a bytecode dispatch loop: `while pc < bytecode.len() { match op { ... } }`. When the loop condition becomes false — i.e., the program counter reaches the end of the bytecode without encountering a `RET_VOID` instruction — the function currently returns `Ok(())`, treating it as a successful completion.

This behavior was not an explicit design choice; it fell out of the loop structure. The question is: should running off the end of bytecode be a valid implicit return, or should it be a trap?

This question surfaced during the [VM testing design](../design/vm-testing.md) analysis, where it was identified as an undocumented behavior that needs a deliberate decision.

## Decision Drivers

* **ADR-0005 (Safety-first)** — PLC programs control physical processes; ambiguous success is dangerous
* **ADR-0006 (Bytecode verification)** — the verifier can enforce `RET_VOID` termination before execution, making the runtime check redundant for verified bytecode
* **Defense-in-depth** — even with a verifier, the VM should not assume bytecode is verified (a corrupted container, a bug in the verifier, or a container loaded without verification)
* **Embedded simplicity** — the VM should be simple; adding a post-loop trap is one extra comparison and branch, trivial in cost
* **Instruction set clarity** — every function must have a well-defined termination; implicit fallthrough is a source of bugs in bytecode systems

## Considered Options

* Implicit return — running off the end returns `Ok(())` (current behavior)
* Explicit trap — running off the end returns `Err(Trap::MissingReturn)`
* Verifier-only enforcement — the verifier rejects bytecode without `RET_VOID`, but the VM allows implicit return

## Decision Outcome

Chosen option: "Explicit trap", because silent success on malformed bytecode violates the safety-first principle, and the cost (one comparison after the loop) is negligible.

The `execute()` function should return `Err(Trap::MissingReturn)` after the while loop exits without hitting `RET_VOID`. A new `Trap::MissingReturn` variant is added to the `Trap` enum.

### Consequences

* Good, because the VM never silently succeeds on bytecode that might be corrupt or truncated — a missing `RET_VOID` is always surfaced
* Good, because the behavior is explicit and documented — no ambiguity about what "falling off the end" means
* Good, because it enforces defense-in-depth — even if the verifier misses a case or is bypassed, the VM catches it
* Good, because the performance cost is one branch per function return, which is negligible compared to the dispatch loop overhead
* Bad, because hand-assembled test bytecode that omits `RET_VOID` will now fail — existing tests that rely on implicit return must be updated (this is actually good — it makes the tests more explicit)
* Neutral, because well-formed bytecode always ends with `RET_VOID`, so this trap never fires in correct operation — it is purely a safety net

### Confirmation

1. Add `Trap::MissingReturn` to the `Trap` enum with a `Display` implementation
2. Change `execute()` to return `Err(Trap::MissingReturn)` after the while loop
3. Add a test: `execute_when_bytecode_ends_without_ret_void_then_missing_return_trap`
4. Update any existing tests that rely on implicit return to include `RET_VOID`

## Pros and Cons of the Options

### Implicit Return (current behavior)

Running off the end of bytecode returns `Ok(())`.

* Good, because it is the simplest implementation — no additional code needed
* Good, because it is lenient with hand-assembled bytecode in tests
* Bad, because truncated or corrupt bytecode silently "succeeds" — the VM reports no error, but the program did not complete its intended logic
* Bad, because it contradicts ADR-0005 (safety-first) — silent success on ambiguous input is the opposite of fail-fast
* Bad, because it makes debugging harder — a missing `RET_VOID` due to a codegen bug produces wrong results with no error, rather than an immediate trap

### Explicit Trap (chosen)

Running off the end returns `Err(Trap::MissingReturn)`.

* Good, because corrupt or truncated bytecode is immediately detected
* Good, because codegen bugs that forget `RET_VOID` are caught at runtime
* Good, because the cost is negligible (one branch after the loop)
* Good, because it aligns with ADR-0005 (safety-first) — fail loud on ambiguous input
* Bad, because hand-assembled test bytecode must always include `RET_VOID` (minor inconvenience)

### Verifier-Only Enforcement

The verifier rejects bytecode without `RET_VOID`, but the VM allows implicit return.

* Good, because verified bytecode never hits the implicit return path
* Bad, because unverified bytecode (testing, development, corrupted containers) silently succeeds
* Bad, because it creates a gap between "verified behavior" and "actual behavior" — the VM behaves differently depending on whether the verifier ran, which is confusing and error-prone
* Bad, because it assumes the verifier is always present and correct, which violates defense-in-depth

## More Information

### Precedent in Other VMs

- **JVM**: Requires explicit `return` instructions; falling off the end of a method is a `VerifyError` at class loading time, and the interpreter assumes it cannot happen
- **WebAssembly**: Every function body has an implicit return of its result type at the end of the block; there is no "fall off the end" concept because the function body is a structured block, not a flat bytecode stream
- **CPython**: The compiler always emits `RETURN_VALUE` at the end of a function; the interpreter assumes it is present (undefined behavior if missing)
- **Lua**: The compiler always emits `OP_RETURN`; the interpreter does not check for end-of-bytecode

Most VMs handle this at the compiler/verifier level and trust the invariant at runtime. The explicit trap option is more conservative (defense-in-depth), which is appropriate for a PLC runtime where silent misbehavior can have physical consequences.
