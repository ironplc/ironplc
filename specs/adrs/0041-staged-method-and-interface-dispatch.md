# ADR-0041: Staged Method/Property Dispatch and Interface Values

status: proposed
date: 2026-07-27

## Context and Problem Statement

The CODESYS/TwinCAT OOP vendor extensions (`EXTENDS`, `IMPLEMENTS`,
`METHOD`, `PROPERTY`, `THIS^`/`SUPER^`) parse fully and are checked for
static shape (field inheritance, `IMPLEMENTS` conformance against an
interface's required members) in the in-flight OOP-foundation work
(PR #1247 and siblings), which is **not yet merged to `main`**. None of
this has runtime semantics yet -- every `FUNCTION_BLOCK` using any of
these constructs is still uniformly flagged as an unsupported vendor
extension by that foundation work, and no codegen exists for calling a
`METHOD`, reading/writing a `PROPERTY`, or resolving `THIS^`/`SUPER^`.

Verified directly against the current codegen before proposing
anything here, not assumed:

- `compile_fb_call` (`compiler/codegen/src/compile_stmt.rs`) resolves
  a function block call's target **statically**: `ctx.fb_instances`
  looks up a `type_id` (`u16`) from the *declared* variable, baked
  directly into the emitted call instruction (`emitter.emit_fb_call
  (type_id)`, `compiler/codegen/src/emit.rs`). There is no per-instance
  runtime type information anywhere.
- `FbInstanceInfo`/`FbTypeId` (`compiler/codegen/src/compile.rs`) are
  compile-time-only bookkeeping: one `type_id` per function block
  *type*, assigned once during compilation, never stored in the
  instance's own memory.
- `iec_type_tag` (`compiler/codegen/src/compile_setup.rs`) is unrelated
  debug-info metadata used for naming variables in tooling output --
  not a real type tag usable for dispatch.
- Function block instances already live in the shared 8-byte-slot data
  region (ADR-0026), addressed via field offsets resolved entirely at
  compile time (ADR-0027).

"OOP dispatch" is actually two structurally different problems:

1. **Calling a method/property on a value whose concrete type is known
   at compile time** -- a direct instance (`instance.Method()`), including
   a method the instance's concrete type overrides from a base via
   `EXTENDS`. The compiler can determine exactly which `MethodDeclaration`
   body to invoke without any runtime information, the same way
   `compile_fb_call` already resolves a plain FB call today.
2. **Calling a method/property through a value whose concrete type is
   *not* known at compile time** -- a `REFERENCE TO`/`POINTER TO` a
   *base* function block type used polymorphically, or a variable of
   *interface* type. The same reference could point at any
   derived/implementing type at runtime; the correct method body can
   only be selected once the actual object is known, which requires
   genuine runtime dispatch.

## Decision Drivers

- Minimize new runtime machinery for the common case -- direct,
  statically-resolvable method calls are expected to be the majority
  of real usage (methods overriding a base's behavior on a concretely
  typed instance is the dominant OOP pattern actually observed in
  survey work on this dialect so far), and don't need any of the
  machinery case 2 requires.
- Safety -- the VM is `no_std` and all memory layout must be statically
  determined (ADR-0005); any new indirection must preserve that
  invariant, not introduce open-ended runtime allocation or unbounded
  lookup.
- Consistency -- reuse the existing slot-based data region model
  (ADR-0017, ADR-0026) rather than inventing a second, parallel memory
  model just for OOP types.
- Incremental delivery -- the two problems above are independently
  valuable; shipping case 1 does not require having solved case 2, and
  a single big-bang PR covering both is a much larger review surface
  and a much larger risk of getting the memory-layout decision wrong
  before real usage data exists.
- Avoid speculative complexity -- most function blocks never
  participate in `EXTENDS`/`IMPLEMENTS` at all; paying a
  memory/indirection cost for every instance regardless of whether it
  is ever used polymorphically is a cost this ADR prefers to avoid on
  the common case. (This "only polymorphic types pay" instinct is this
  ADR's own. ADR-0026 in fact chose a *uniform* 8-byte slot per
  structure field over a packed, pay-for-what-you-use layout, so it is
  not a precedent for selective per-instance cost.)

## Considered Options

- **Option A -- static-only, forever.** Resolve every method/property
  call at compile time by declared type. Reject (with a diagnostic,
  not silently) any call through a base-typed reference/pointer or an
  interface-typed variable that cannot be statically resolved to a
  single concrete type. Simplest to implement; permanently incomplete
  relative to real IEC 61131-3 OOP semantics -- interface-typed
  parameters/variables are a normal, intentional pattern (that is the
  entire point `IMPLEMENTS` exists for), so permanently rejecting them
  is not a real destination, only a possible first step.
- **Option B -- universal vtable.** Give every function block instance
  a runtime type tag / vtable pointer unconditionally, and always
  dispatch indirectly, even for a plain, non-polymorphic instance call.
  Uniform mechanism, no static/dynamic split to maintain -- but pays a
  memory and indirection cost on every instance of every function
  block in every program, including the (expected to be large) fraction
  that never participates in inheritance at all.
- **Option C -- staged/hybrid.** Keep today's static resolution
  mechanism unchanged for anything the compiler can prove the concrete
  type of (direct instance calls, including calls that resolve to an
  overridden method via a statically-known derived type). Add a new,
  opt-in runtime type tag and per-type dispatch table **only** for
  function block types that ever participate in a polymorphic call
  site (implement an interface, are extended and referenced
  polymorphically, or are the base type of a `REFERENCE TO`/`POINTER
  TO` used polymorphically). Deliver the static case as its own,
  smaller, independently-useful first phase; add the dynamic case as a
  second phase once a concrete need and design for the dispatch table
  encoding exists.

## Decision Outcome

Chosen option: **C, staged -- but only Phase 1 is decided by this ADR.**

Phase 1 (static dispatch) is adopted. Phase 2 (dynamic dispatch through
references, pointers, and interfaces) is **not** decided here: it is
recorded as the anticipated direction but deferred to its own ADR,
because as sketched it conflicts with the safety-first principle
(ADR-0005) and the static-verification requirement (ADR-0006) in ways
this ADR does not resolve. See "Relationship to ADR-0004/0005/0006"
below.

### Phase 1 -- static dispatch (including overridden methods)

- Given an instance's *static* declared type and a method/property
  name, resolve the concrete `MethodDeclaration`/`PropertyDeclaration`
  to call by walking the static type's own methods first, then its
  `EXTENDS` chain base-to-derived -- reusing the same traversal built
  for `IMPLEMENTS` conformance checking in the OOP-foundation PRs
  (`collect_all_fb_members`, not yet in `main`), which produces "this
  type's own + everything inherited" in the right precedence order.
- `THIS^` resolves to a fixed self-reference plumbed through method
  codegen (new codegen support; no runtime polymorphism -- the "self"
  instance is always the one already executing).
- `SUPER^` resolves by calling the immediate base type's own method
  body directly, bypassing the derived type entirely -- also
  compile-time-resolvable without any dispatch table, since "the
  base's implementation" is unambiguous regardless of what the actual
  runtime type turns out to be elsewhere.
- `PROPERTY` reads/writes rewrite to `GET`/`SET` accessor calls at
  compile time (new codegen rewrite step; still no runtime
  polymorphism).
- This phase delivers real execution semantics for direct
  method/property use and `THIS^`/`SUPER^`, without any function block
  memory-layout change and without introducing any new runtime
  indirection at all.

### Phase 2 (deferred -- not decided by this ADR) -- dynamic dispatch through references, pointers, and interfaces

Phase 2 is described here only as the anticipated shape of a future
change, **not** as an approved decision. It must not be implemented
until a dedicated ADR resolves its conflict with ADR-0005 and ADR-0006
(see "Relationship to ADR-0004/0005/0006" below). The sketch that
follows is the starting point for that future ADR, not a settled design.

- A per-instance runtime type tag: one additional 8-byte slot (same
  per-field cost model ADR-0026 already established), added **only**
  to function block types that are ever the target of `IMPLEMENTS`,
  ever extended and referenced polymorphically, or ever the base type
  of a polymorphically-used `REFERENCE TO`/`POINTER TO` -- not
  universally on every instance (Option B rejected specifically to
  avoid this cost on the common case).
- A per-(concrete-type, member-name) dispatch table built entirely at
  compile time (every concrete type and its members are statically
  known; only the *call site's* target is dynamic) and addressed by
  the runtime type tag.
- One new indirect-call mechanism (opcode or opcode variant) that loads
  the instance's own type tag, indexes into the table, and dispatches
  -- the only new runtime indirection Phase 2 would introduce.
- An interface-typed value needs a representation carrying both the
  underlying instance's data reference and its runtime type tag (a
  "fat" reference), reusing the same tag/table mechanism as
  `EXTENDS`-based dispatch -- interface method resolution is "does
  this concrete type provide a compatible member," already checked
  statically for shape (name + return type) by the existing
  `IMPLEMENTS` conformance rule.
- The exact table layout and opcode encoding are intentionally **not**
  fixed by this ADR -- they are deferred to Phase 2's own
  implementation plan, once real polymorphic call-site patterns from
  actual usage inform the design, the same way ADR-0026 deferred its
  own packed-layout migration details to a future change rather than
  over-specifying them up front.

### Non-goals

- Access-modifier (`PUBLIC`/`PRIVATE`/`PROTECTED`/`INTERNAL`)
  enforcement. Currently metadata-only; whether/when to add real
  enforcement is a separate decision, orthogonal to dispatch mechanism.
- `ABSTRACT` non-instantiation enforcement is orthogonal to dispatch and
  unaffected by this ADR either way. (No specific problem code is cited
  here: the OOP-foundation PRs own that diagnostic, and it is not yet in
  `main`. `P4040` in `main` is `ConstantExpressionOverflow`, unrelated to
  `ABSTRACT`.)
- A specific Phase 2 bytecode container-format change. Only the
  general shape (one new tag slot per polymorphic-participating
  instance, a compile-time-built table, one new indirect-call
  mechanism) is sketched here, and even that is deferred to Phase 2's
  own ADR rather than decided.

## Consequences

- Good, because Phase 1 delivers real, useful semantics for the
  expected-majority static-dispatch case without any risky
  architecture change or memory-layout impact.
- Good, because *if* Phase 2 is later approved, its cost (extra slot,
  indirect call) is paid only by function block types that are genuinely
  used polymorphically, not by every instance in every program.
- Good, because both phases stay consistent with the existing
  slot-based data region model (ADR-0017/0026) rather than
  introducing a second, parallel memory model for OOP types alone.
- Bad, because "Full OOP" is not delivered in a single PR -- accepted
  given the size and risk difference already established between the
  two cases.
- Bad/open, because Phase 2 as sketched conflicts with ADR-0005's
  explicit rejection of runtime-tag polymorphic dispatch and with
  ADR-0006's static call-target verification. This ADR does not resolve
  that conflict; it records the direction and defers the decision to a
  dedicated Phase 2 ADR that must supply the safety and verification
  analysis first. It is an open question whether Phase 2 should be
  implemented at all, or whether interface-typed values should instead
  be supported by a more verifiable mechanism.
- Neutral, because Phase 2's table/opcode encoding *and* its safety
  analysis are deliberately left to that future ADR, so this ADR alone
  does not decide Phase 2 -- only that it is the anticipated direction
  and roughly what it would cost.

## More Information

### Relationship to prior ADRs

Phase 1 is uncontroversial against the safety ADRs -- it adds no runtime
indirection and no memory-layout change. The relationships below are
therefore mostly about Phase 2, and this is where the tension lives.

- **ADR-0004** (separate type families over polymorphic opcodes) and
  **ADR-0005** (safety-first): these establish a standing preference for
  statically-encoded type distinctions over runtime-tag dispatch.
  ADR-0005 explicitly *rejected* "polymorphic dispatch with a runtime
  tag" (its STRING/WSTRING worked example) on the grounds that a correct
  runtime tag is "an assumption, not a proof." Phase 2's core mechanism
  -- load an instance's runtime type tag, index a table, dispatch -- is
  exactly that rejected shape. Phase 2 preserves the *memory* invariant
  ADR-0005 cares about (the tag and table are static in size and
  content, no dynamic allocation), but it does **not**, on its own,
  satisfy ADR-0005's *verification* concern. That tension -- not memory
  layout -- is the reason Phase 2 is deferred rather than decided here.
  ADR-0005 does allow its default to be overridden "when the safety cost
  is genuinely minimal and the economy benefit is large"; making that
  case (or finding a more verifiable alternative) is the first task of
  Phase 2's own ADR.
- **ADR-0006** (bytecode verification): the verifier must statically
  check call-target validity and stack-type consistency at every call
  site. A call target selected from a runtime tag is data-dependent, so
  Phase 2's new indirect-call opcode needs an explicit verification
  story -- for example, the verifier proving each per-type dispatch
  table is well-formed and that every candidate target shares a
  compatible signature, so stack typing holds no matter which entry is
  selected. This ADR does not provide that story; supplying it is a
  precondition for Phase 2, not an afterthought.
- **ADR-0017** (unified data region), **ADR-0026** (structure memory
  layout), **ADR-0027** (compile-time field offset resolution): Phase
  2's per-instance type tag is one more slot in the same data region
  model already used for every other function block field, not a new
  memory model.
- The parsing and static-shape-checking work for
  `METHOD`/`PROPERTY`/`EXTENDS`/`IMPLEMENTS`/`THIS^`/`SUPER^` lands in
  the in-flight OOP-foundation PRs (PR #1247 and siblings) and is **not
  yet merged to `main`**; it is tracked separately and is not part of
  this repository's ADR series. That work keeps every OOP-using
  `FUNCTION_BLOCK` gated as an unsupported vendor extension. This ADR
  governs the mechanism for eventually lifting that gate, not the gate
  itself.

### Verification

Grounded directly in the current codegen rather than assumed:
`compile_fb_call` (`compiler/codegen/src/compile_stmt.rs`), confirmed
to resolve call targets statically via `ctx.fb_instances`;
`FbInstanceInfo`/`FbTypeId` (`compiler/codegen/src/compile.rs`),
confirmed to be compile-time-only with no per-instance runtime
storage; `iec_type_tag` (`compiler/codegen/src/compile_setup.rs`),
confirmed to be unrelated debug-info metadata, not a real type tag.
