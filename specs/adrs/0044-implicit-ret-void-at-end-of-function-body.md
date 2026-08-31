# Implicit RET_VOID at the End of a Function Body, Enforced by the Verifier

status: accepted
date: 2026-08-31
supersedes: ADR-0011

## Context and Problem Statement

[ADR-0011](0011-bytecode-implicit-return-behavior.md) asked whether the program
counter reaching the end of a function body without an explicit `RET` / `RET_VOID`
should be a valid implicit return or a trap. It chose the trap: `execute()` was to
return `Err(Trap::MissingReturn)`, with a new `Trap::MissingReturn` variant.

That was never built, and the code has since entrenched the option ADR-0011
rejected. `Trap::MissingReturn` exists nowhere in `compiler/vm/`. The dispatch
loop in `vm.rs` handles `pc >= bytecode.len()` by calling `handle_frame_return`,
with a comment saying so:

> Fell off the end of a function body without an explicit `RET` — treat as `RET_VOID`.

The iterative-dispatch rewrite carried that behaviour forward without anyone
revisiting the ADR, and `specs/design/bytecode-instruction-set.md` documents the
implemented behaviour **while citing ADR-0011 as its source** — an ADR that says
the opposite. Because ADR-0011 never reached `accepted`, this is an unimplemented
decision rather than a violated one, but a reader consulting it is told the VM
traps where in fact it returns.

Two things changed since ADR-0011 that bear on the question:

1. **The bytecode verifier landed** ([ADR-0006](0006-bytecode-verification-requirement.md)),
   and it models the fall-off point explicitly rather than ignoring it.
   `container/src/verify.rs` treats offset `len` as a reachable program point and
   checks it as a return site: `return_check(function_id, pc, RET_VOID_DEPTH, depth)`.
   A body that falls off the end with anything left on the operand stack is
   rejected with `StackImbalance::UnbalancedReturn` before it can run.
2. **The VM became a single iterative dispatch loop over a frame stack.** There
   is no longer a per-function `while pc < bytecode.len()` whose exit is a natural
   place to hang a trap; `pc >= bytecode.len()` is now one arm of the outer loop,
   sitting alongside `RET` and `RET_VOID`, all three routed through
   `handle_frame_return`.

So the question is no longer the one ADR-0011 asked. It is: given a verifier that
already checks the fall-off point, does the VM still need a runtime trap there?

## Decision Drivers

* **The record must match the code** — a design document citing an ADR for the
  opposite of what the ADR says is worse than either behaviour on its own
* **ADR-0005 (Safety-first)** — silent success on malformed bytecode is still
  unacceptable; the question is *where* that is caught, not *whether*
* **Defense-in-depth** — the VM should not assume the verifier ran
* **One return path** — `RET`, `RET_VOID` and fall-off-end all pop a frame;
  three spellings of one operation should share one implementation
* **Cost of re-deciding** — real code now depends on the fall-off arm, including
  every hand-assembled test body that omits a trailing `RET_VOID`

## Considered Options

* **A — Implicit `RET_VOID`, verifier-enforced.** Falling off the end pops the
  frame exactly as `RET_VOID` does; the verifier checks the fall-off point as a
  return site and rejects an unbalanced one before execution. (What is built.)
* **B — Implement ADR-0011 as written.** Add `Trap::MissingReturn` and trap in
  the dispatch loop when `pc >= bytecode.len()`.
* **C — Trap in the VM *and* reject in the verifier.** Require an explicit
  terminator in verified bytecode and trap at runtime if one is missing anyway.

## Decision Outcome

Chosen option: **A — implicit `RET_VOID`, enforced by the verifier.**

Falling off the end of a function body is a well-defined implicit `RET_VOID`. The
safety property ADR-0011 was protecting — that a truncated or corrupt body cannot
silently "succeed" — is delivered statically instead of at runtime: the verifier
treats the end of the body as a return site and holds it to the same operand-stack
contract as an explicit `RET_VOID`, so the malformed bodies ADR-0011 worried about
are rejected at load time rather than trapped mid-scan. Rejecting bad bytecode
before it runs is strictly better for a PLC than trapping partway through a scan
that has already actuated outputs.

Truncation specifically is caught earlier still, when the container is parsed: the
header and the function directory declare every section's size, so a short file
fails to load with `SectionSizeMismatch` (or an unexpected end of input) rather
than producing a body that runs off its end.

The residual gap is a container executed without verification. That is a narrower
hole than ADR-0011 assumed, and closing it with a runtime trap would mean the VM
rejects bytecode the verifier accepts — a second, divergent definition of a
well-formed body, which is the kind of "verified behaviour differs from actual
behaviour" gap ADR-0011 itself argued against under its Verifier-Only option.

### Consequences

* Good, because the ADR record now matches `vm.rs` and
  `bytecode-instruction-set.md`, which previously cited ADR-0011 for the opposite
  of what it decided
* Good, because malformed bodies are rejected before execution rather than
  trapped mid-scan, which suits a runtime that drives physical outputs
* Good, because `RET`, `RET_VOID` and fall-off-end share one implementation
  (`handle_frame_return`), so frame teardown cannot diverge between them
* Good, because hand-assembled test bytecode stays readable without a trailing
  `RET_VOID` on every body
* Bad, because a container executed with verification skipped has no runtime
  backstop for a body that falls off the end — accepted, because such a container
  has no backstop for any other verifier-checked property either, and running
  unverified bytecode is the thing to prevent, not to make partially safe
* Neutral, because codegen always emits an explicit terminator, so the implicit
  path is reached only by hand-written or generated test bodies

### Confirmation

1. `verify_stack_balance_when_falls_off_end_unbalanced_then_unbalanced_return`
   in `container/src/verify.rs` pins the verifier's treatment of the fall-off
   point as a `RET_VOID`-depth return site.
2. `bytecode-instruction-set.md` cites this ADR, not ADR-0011, for the
   fall-off-end behaviour.

## Pros and Cons of the Options

### Option A: Implicit `RET_VOID`, verifier-enforced (chosen)

* Good, because it is what is built, tested, and documented — no code change, no
  re-litigating behaviour the iterative-dispatch rewrite already depends on
* Good, because the verifier's check is stronger than the proposed trap: it
  catches an *unbalanced* fall-off, whereas `Trap::MissingReturn` fires on any
  fall-off, balanced or not, and so says nothing about stack discipline
* Good, because bad bytecode is rejected before the first output is written
* Bad, because unverified bytecode has no runtime backstop

### Option B: Trap on fall-off-end (ADR-0011 as written)

* Good, because the VM never relies on the verifier having run
* Bad, because it makes the VM stricter than the verifier: bodies the verifier
  accepts would trap at runtime, so "verified" would no longer imply "runnable"
* Bad, because every hand-assembled test body must end in `RET_VOID`, for a
  check the verifier already performs more precisely
* Bad, because the trap fires mid-scan, after side effects, where the verifier's
  rejection happens before any execution

### Option C: Trap in the VM and require a terminator in the verifier

* Good, because it is genuinely defense-in-depth — two independent checks
* Bad, because it forbids a body shape the verifier can already prove safe, for
  no gain in the verified path
* Bad, because it costs a new `Trap` variant, a new verifier rule, and updates to
  every test body, to close a gap that only exists when verification is skipped

## More Information

### Relationship to ADR-0011

This ADR supersedes ADR-0011. ADR-0011's *analysis* stands — silent success on
malformed bytecode does violate ADR-0005, and the three options it framed are the
right three. What changed is that the verifier arrived and made the fall-off point
a checked return site, which is the "verifier-only enforcement" option ADR-0011
rejected on the grounds that it "assumes the verifier is always present and
correct." That objection is weaker than it looked: the same assumption underwrites
every other property the verifier checks (jump targets, stack depth against the
declared maximum, operand-stack balance at every `RET`), none of which the VM
re-checks at runtime either. Singling out the missing terminator for a runtime
trap would be inconsistent with how the rest of bytecode well-formedness is
handled.
