# OOP Dispatch Design

Date: 2026-09-27
Status: draft. Sections marked **Proposal** are suggestions for review.
Sections marked **Needs decision** are open for the maintainer.

## Goal

Describe how a method or property call reaches the body that runs, both
without dynamic dispatch (tier 1) and with it (tier 2). The result is the
input for the Phase 2 ADR that [ADR-0041](../adrs/0041-staged-method-and-interface-dispatch.md)
defers.

## Scope

In scope:

- `METHOD`, `PROPERTY` (`GET`/`SET`), `EXTENDS`, `IMPLEMENTS`, `THIS^`, `SUPER^`
- Interface-typed variables, inputs and outputs
- `REFERENCE TO` / `POINTER TO` a function block type, and `VAR_IN_OUT` of a
  function block type
- Interface null (`= 0`, `<> 0`), `__ISVALIDREF`, calling through null
- `__QUERYINTERFACE`, `__QUERYPOINTER`

Out of scope:

- Parsing of the constructs (syntax PRs, no design doc needed)
- Access modifiers and `ABSTRACT` enforcement (ADR-0041 non-goals)
- `__NEW` / `__DELETE`. No dynamic allocation, in either tier.
- `FB_init`. See [Adjacent: FB_init](#adjacent-fb_init).

## Terminology

- Separate the two axes that are easy to conflate: *dispatch* (is the call
  target known at compile time?) and *allocation* (is memory reserved at
  run time?). Dynamic dispatch over statically allocated instances is the
  normal TwinCAT case.
- Define: static dispatch, dynamic dispatch, closed world, dispatch id,
  fat reference, call site.

## Usage patterns

Taken from real TwinCAT telescope-control code. Names below are generic.
Each pattern gets one minimal example.

1. **State pattern.** A controller FB holds `ipState : I_State` and
   reassigns it at run time to one of several state FB instances that
   extend a common base. Each state gets a back-pointer to the controller
   through `FB_init(THIS)`. Truly dynamic, so tier 1 can't express it.
2. **Injected dependency.** An input `comm : I_Comm`, wired once to a
   concrete implementer. The target doesn't change after wiring, but the
   callee's declared type doesn't say which one it is.
3. **Optional component.** `ipFocus : I_Focus`, checked with `<> 0` or
   `__ISVALIDREF` before use.
4. **Deep `EXTENDS` on concrete instances.** A chain like
   `FB_AzimuthAxis EXTENDS FB_Axis EXTENDS FB_BaseAxis`, used through
   `REFERENCE TO FB_AzimuthAxis`, a concrete type. Tier 1 covers this.
5. **Interface inheritance.** `I_Axis EXTENDS I_BaseAxis`, which needs
   interface-to-interface upcasts.

Not observed: `__NEW`/`__DELETE`, `__QUERYINTERFACE`/`__QUERYPOINTER`,
arrays of interfaces, and a `REFERENCE TO` / `POINTER TO` a base FB that
points at a derived instance.

Conclusion for this section: tier 1 alone compiles pattern 4 only. Patterns 1
to 3 need tier 2. In all of them, every implementer is part of the program
(closed world).

## Closed world

**Proposal:** tier 2 assumes the compiler sees every function block type in
the program. This holds today: compatibility libraries are parsed and merged
into one `Library` before analysis
([compatibility-libraries.md](compatibility-libraries.md)).

**Needs decision:** is separate compilation of libraries planned? If so, the
design needs a link step that builds the dispatch branches once all types are
known.

## Tier 1: static dispatch

- ADR-0041 Phase 1 shipped resolution along the `EXTENDS` chain, `THIS^` and
  `SUPER^`.
- The gap is `PROPERTY` ([#1692](https://github.com/ironplc/ironplc/issues/1692)),
  which is a compile-time rewrite to `GET`/`SET` calls.

**Proposal:**

- **Rule:** every call whose receiver is not an interface value is tier 1.
  That covers direct instances, `THIS^`, `SUPER^`, and `REFERENCE TO` /
  `POINTER TO` / `VAR_IN_OUT` of an FB type.
- The rule is sound only if the receiver's static type is always the
  referent's exact type. TwinCAT doesn't guarantee that: a pointer, a
  reference or a `VAR_IN_OUT` of an FB type may point at a derived
  instance, and a method call through it is dynamically bound
  ([Beckhoff: Method call](https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2527348875.html)).
- So in the first version, passing or binding a derived instance where a
  base FB type is expected (`REF=`, `REF()`, `ADR()`, pointer assignment,
  `VAR_IN_OUT` argument) is a diagnostic. This rejects valid TwinCAT code,
  as a stated interim restriction. It wasn't observed in the usage
  patterns. Lifting it means these receivers become tier 2 as well, and the
  fat-reference representation doesn't cover them, because `ADR()` and
  `VAR_IN_OUT` pass a plain address.
- **Current state (checked 2026-09-27):** a method call through
  `REFERENCE TO` an FB is rejected (`P4012`), a field read through it is not
  implemented (`P9999`), and `p^.Method()` does not parse. So no call reaches
  the wrong body today. But `REF=` does not check the target type between FB
  types: binding an unrelated FB to `REFERENCE TO FB_Base` compiles
  ([#1869](https://github.com/ironplc/ironplc/issues/1869)). The
  exact-type rule above needs that check, so it lands first. For pointers
  there is no such check to add: TwinCAT accepts `ADR()` of an unrelated FB
  into a `POINTER TO FB_Base` without a warning (checked in XAE
  3.1.4024, 2026-09-27). So a pointer's static type says nothing reliable about its
  referent, and the exact-type rule can't be enforced for `POINTER TO`.
  Method calls through `p^` then need either a runtime check or to stay
  unsupported. A method call
  through `VAR_IN_OUT` of an FB type is not implemented either (`P9999`).
- Calling through an interface value while tier 2 is off is a diagnostic
  (new `P####`) that names the flag.
- **Devirtualisation** (an interface with one implementer resolved
  statically) is deferred. It is an optimisation, and correctness doesn't
  depend on it.

## Tier 2: dynamic dispatch

### Mechanism options

- **M1: type tag, vtable, indirect call opcode.** The ADR-0041 sketch. It
  conflicts with ADR-0005 (runtime-tag dispatch rejected) and ADR-0006
  (call targets must be statically verifiable).
- **M2: closed-world branch.** The compiler knows every implementer. An
  interface value carries a small dispatch id, and each call site compiles to
  a bounded branch over the implementers of the interface, with a direct call
  in every branch. The default branch traps. Every target is an ordinary
  static call that the verifier already checks, and no new call opcode is
  needed. The cost is code size per call site, which grows with the number of
  implementers.
- **M3: function-id table plus verifier proof.** Like M1, but the verifier
  checks that each table is well-formed and that all entries share a
  signature.

**Proposal: M2. Needs decision,** because it is an interpretation of
ADR-0005 (see [Memory and safety](#memory-and-safety)).

| | M1 | M2 | M3 |
|---|---|---|---|
| New opcode | indirect call | none | indirect call |
| Verifier work | new, unresolved | none new | table checks |
| Code size | small per call site | grows with implementers | small per call site |
| Dispatch cost | constant | linear in implementers (or a jump table) | constant |
| Container format | vtable section | unchanged | table section |

The implementer counts seen in the usage patterns are in the single digits
per interface, so the linear cost of M2 is small in practice. If an
interface ever has many implementers, the branch can become a jump table,
because dispatch ids are dense.

### Representation

**Proposal:**

- An interface value is a **fat reference**: two slots, an instance
  reference and a dispatch id (ADR-0017, ADR-0026). FB instance layout does
  not change, so a program without interface values pays nothing.
- **Dispatch ids** are numbered per program by codegen, from 1 up, over
  every concrete FB type that implements at least one interface. 0 means
  null. They are derived from the
  [ADR-0055](../adrs/0055-concrete-type-ids-numbered-by-debug-tag.md)
  `TypeId`s through a map, not equal to them. `TypeId`s are sparse (user types
  start at 256 and anonymous types take ids too), and a dense numbering keeps
  the jump-table option open. Both are per compilation, so nothing is lost.
- A dispatch id names a **concrete type, not an interface**. So an upcast
  between interfaces (pattern 5) copies the fat reference unchanged.
- Assigning an FB instance or an exact-typed reference to an interface
  variable builds the fat reference from the static type. That is correct
  only because of the exact-type rule in tier 1.
- A property read or write through an interface is a `GET` / `SET` call,
  dispatched like any method.
- `SUPER^` inside an overriding method stays static.

## Opt-in

- **O1: flag.** With it off, a call through an interface value is a
  diagnostic.
- **O2: per type, automatically.** Only types used polymorphically pay.

**Proposal: O1 only. Needs decision.** With fat references, O2 is automatic
anyway: FB instances never change, and only interface values and their call
sites cost anything. What's left is whether tier 2 is allowed at all.

- Flag: `--allow-interface-dispatch`, next to the existing
  `--allow-fb-inheritance`.
- Enabled by `--dialect twincat`.
- Check it against ADR-0038 (no restrictions on flag combinations), for
  example with interface declarations allowed but dispatch off.

## Null and validity

**Proposal:**

- The default value of an interface variable is null: dispatch id 0 and
  instance reference 0.
- `= 0` and `<> 0` on an interface value compare the dispatch id.
  `__ISVALIDREF` on an interface value means the dispatch id is not 0
  (whether TwinCAT accepts `__ISVALIDREF` on an interface is unverified).
  `__ISVALIDREF` on `REFERENCE TO` keeps its existing meaning.
- A call through null lands in the default branch of the M2 dispatch and
  traps with a VM error code (ADR-0014; the category is chosen when
  implementing). The trap costs nothing extra, because the default branch
  exists anyway.

## `__QUERYINTERFACE` / `__QUERYPOINTER`

**Proposal:** a follow-up PR after tier 2, since the usage patterns don't use
them. Both compile the same way as a call. `__QUERYINTERFACE(src, dst)`
branches over the implementers of the source interface. For those that also
implement the target interface, it copies the fat reference and returns
`TRUE`. Otherwise it returns `FALSE`. No table is needed.

## Memory and safety

- No dynamic allocation in either tier. Dynamic dispatch doesn't need it:
  every instance is declared statically, and an interface value only refers
  to one.
- **ADR-0005** rejected runtime-tag dispatch because a correct tag is "an
  assumption, not a proof". The argument for M2 is that the runtime value
  selects between statically known, verified direct calls, like a `CASE`
  statement does. A bad dispatch id can therefore only reach the trap, or a
  verified method of another implementer.
- **The remaining gap** is the pairing between the instance reference and
  the dispatch id. If they disagree, a method of type A runs on an instance
  of type B. The compiler is the only thing that builds fat references, but
  `--allow-ref-type-punning`, `POINTER TO` an interface and `MEMCPY` could
  forge one. Still to establish: whether data-region bounds checks limit the
  damage to a wrong result, or whether it can be worse. Options: reject
  punning and pointers where interface values are involved, or check the
  pairing at dispatch.
- ADR-0017, ADR-0026 and ADR-0027: fat references are two ordinary slots.
  Instance layout and field offsets don't change.

## Verification

**Proposal:** with M2, the bytecode verifier needs no new rules. Every call
is a direct call. The instance and dispatch id pairing is a compiler
invariant that the verifier doesn't see (see above).

## Container format and debug info

**Proposal:** M2 needs no container change. For the debugger, add a table
from dispatch id to type name to the debug section, so that an interface
variable shows its concrete type. This can come after tier 2.

## Adjacent: FB_init

**Proposal:** a separate issue, in the same milestone as tier 2. The state
pattern declares instances as `FB_X(THIS)`, so tier 2 alone doesn't compile
it. `FB_init` is construction, not dispatch, and doesn't block this design.

## Requirements

REQ IDs (`REQ-OOP-<crate>-NNN`) are added once a section becomes normative,
together with their conformance tests.

## Open questions

- **Needs decision:** M2 as the mechanism, and whether its reading of
  ADR-0005 holds.
- **Needs decision:** tier 2 behind a flag enabled by `--dialect twincat`.
- **Needs decision:** is separate compilation of libraries planned?
- To establish: the damage a forged fat reference can do (see
  [Memory and safety](#memory-and-safety)).

## Related

- [ADR-0041](../adrs/0041-staged-method-and-interface-dispatch.md)
- [ADR-0055](../adrs/0055-concrete-type-ids-numbered-by-debug-tag.md) concrete type ids
- [#1434](https://github.com/ironplc/ironplc/issues/1434) OOP dispatch surface
- [#1692](https://github.com/ironplc/ironplc/issues/1692) PROPERTY
- [#1438](https://github.com/ironplc/ironplc/issues/1438) EXTENDS layout
- [#1419](https://github.com/ironplc/ironplc/issues/1419) interfaces and IMPLEMENTS
