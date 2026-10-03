# OOP Dispatch Design

Date: 2026-09-27, revised 2026-10-01 after the maintainer review on #1870
Status: draft. Sections marked **Proposal** are suggestions for review.
Sections marked **Needs decision** are open for the maintainer. Sections
marked **Decided** record an answer from the review.

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
  instance table, instance index, call site.

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
3. **Optional component.** A component held as an interface or a
   `REFERENCE TO`, which may be unset. It is checked before use, with
   `<> 0` or with `__ISVALIDREF` on the reference.
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

**Decided:** no separate compilation of libraries and no link step for now.
If one is planned later, this design is revisited then: the dispatch
branches and the instance table are built once all types are known.

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
  patterns. Lifting it means these receivers become tier 2 as well, and an
  instance index doesn't cover them directly, because `ADR()` and
  `VAR_IN_OUT` pass a plain address (the same lookup as for `REFERENCE TO`,
  see [Representation](#representation)).
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
- A call through an interface value that tier 1 cannot resolve (see
  below) while tier 2 is off is a diagnostic (new `P####`) that names the
  flag.

### Interface calls resolved statically

**Proposal:** in scope for the first version, not deferred. The aim is to
keep as many calls as possible in tier 1, for safety and speed. The closed
world makes this sound:

- For each interface-typed variable, the analysis computes the concrete
  types it can hold: the types of the instances assigned or passed to it,
  followed through assignments between interface variables, until nothing
  changes.
- When that set has exactly one type, every call through the variable is a
  direct call to that type's method, as for a plain instance. The variable
  needs no dispatch id and no instance table entry.
- An interface that no value is ever called through needs nothing at run
  time.
- Only variables that can hold two or more types need tier 2.
- To measure on the #1199 corpus: how many interface variables fall into
  each case. The state pattern (pattern 1) needs tier 2 by design, because
  its whole point is that the state changes type.


## Tier 2: dynamic dispatch

### Mechanism options

- **M1: type tag, vtable, indirect call opcode.** The ADR-0041 sketch. It
  conflicts with ADR-0005 (runtime-tag dispatch rejected) and ADR-0006
  (call targets must be statically verifiable).
- **M2: closed-world branch over a two-slot reference.** The compiler knows
  every implementer. An interface value is two slots, an instance offset
  and a dispatch id, and each call site compiles to a bounded branch over
  the implementers, with a direct call in every branch. The default branch
  traps. **Rejected:** the two slots are writable memory and can disagree
  (see [Memory and safety](#memory-and-safety)), and a program's use of
  dynamic dispatch can't be told from its bytecode, because the branch is
  the same compare-and-call a user's own `CASE` produces.
- **M2b: closed-world branch over an instance table** (from the review).
  The same branch as M2, but the instance offset and the dispatch id come
  from a read-only table in the container, and the interface value is only
  an index into it (see [Representation](#representation)). A new
  `LOAD_INSTANCE` pushes the pair and traps on a bad index.
- **M3: function-id table plus verifier proof.** Like M1, but the verifier
  checks that each table is well-formed and that all entries share a
  signature.

**Proposal: M2b.** It keeps M2's reading of ADR-0005 (a runtime value
selects among statically verified direct calls, like `CASE`) and closes
M2's gap: the instance-to-type pairing is enforced by the VM, not assumed
from the compiler.

| | M1 | M2 | M2b | M3 |
|---|---|---|---|---|
| New opcode | indirect call | none | `LOAD_INSTANCE` (sub-opcode) | indirect call |
| Verifier work | new, unresolved | none new | load-time table checks | table checks |
| Code size | small per call site | grows with implementers | grows with implementers | small per call site |
| Dispatch cost | constant | linear in implementers | linear, plus one table read | constant |
| Interface value | tag + reference | 2 slots | 1 slot | tag + reference |
| Container format | vtable section | unchanged | instance table section | table section |
| Detectable from bytecode | yes | no | yes (`LOAD_INSTANCE`) | yes |

The implementer counts seen in the usage patterns are in the single digits
per interface, so the linear cost is small in practice. If an interface
ever has many implementers, the branch can become a jump table, because
dispatch ids are dense.

**Performance** (estimates from reading the VM; nothing is built or
measured): M2b costs about one extra decoded instruction per call against
M2. Both pay far more for `METHOD_CALL`'s copy-in and copy-out of every
field and for the branch, so the difference should be noise. M2b halves the
RAM of an interface value (8 B instead of 16 B) and adds about 8 B of
read-only container data per table entry. The Phase 2 ADR should require a
benchmark in `compiler/benchmarks`: a state-pattern scan cycle calling
through an interface, built with each representation.

### Representation

**Proposal:**

- The compiler writes a **read-only instance table** into the container:
  one entry `(data offset, dispatch id)` per instance that some interface
  value can refer to. Its size is known at compile time, and `no_std`
  targets read it in place.
- An interface value is **one slot: an instance index** into that table.
  0 means null. FB instance layout does not change, so a program without
  interface values pays nothing.
- **Dispatch ids** are numbered per program by codegen, from 1 up, over
  every concrete FB type that implements at least one interface. They are
  derived from the
  [ADR-0055](../adrs/0055-concrete-type-ids-numbered-by-debug-tag.md)
  `TypeId`s through a map, not equal to them. `TypeId`s are sparse (user types
  start at 256 and anonymous types take ids too), and a dense numbering keeps
  the jump-table option open. Both are per compilation, so nothing is lost.
- A dispatch id names a **concrete type, not an interface**. So an upcast
  between interfaces (pattern 5) copies the index unchanged. A downcast by
  assignment is a diagnostic, as in TwinCAT ("Cannot convert type 'I_Base'
  to type 'I_Derived'"). Only `__QUERYINTERFACE` converts downwards.
- Assigning an FB instance to an interface variable stores that instance's
  index, a compile-time constant. TwinCAT also accepts a `REFERENCE TO` an
  FB, whose referent is only known at run time; that needs a lookup from
  data offset to index. **Needs decision:** support it in the first version
  (a sub-opcode that finds the entry, linear or by binary search over a
  table sorted by offset), or make it a diagnostic until a usage pattern
  needs it. Taking the type from a reference's static type is correct only
  because of the exact-type rule in tier 1.
- **Which instances get an entry:** only instances with a fixed place in
  the data region, so an interface value never outlives what it refers to.
  Program, global and function block field instances have one. A function
  can't declare an instance (P4054). A method-local instance is not
  compiled yet (P9999). When it is, it gets a fixed scratch slot like every
  other method local (there is no stack, since IEC 61131-3 forbids
  recursion), so it can get an entry too. The #1199 corpus needs that:
  TcUnit test methods declare a local `fbComm : FB_NullComm` and pass it to
  an `I_Comm` input (18 lines in 2 test suites). An index to such an
  instance stays memory-safe after the method returns; it refers to an
  instance that is re-initialized on the next call.
- Interface values are opaque: no `ADR()`, no `POINTER TO` an interface, no
  type punning, no `MEMCPY` or `ANY` writes. With the table these are
  defence in depth, not what safety rests on.
- A property read or write through an interface is a `GET` / `SET` call,
  dispatched like any method.
- `SUPER^` inside an overriding method stays static.

## Opt-in

**Proposal:** both a compile-time switch and a run-time one.

- **Compiler flag** `--allow-interface-dispatch`, next to the existing
  `--allow-fb-inheritance`, enabled by `--dialect twincat`. With it off, a
  call that needs tier 2 is a diagnostic. Check it against ADR-0038 (no
  restrictions on flag combinations), for example with interface
  declarations allowed but dispatch off.
- **Run-time policy.** A program has dynamic dispatch if and only if some
  function body contains `LOAD_INSTANCE`. The container header gets
  `FLAG_HAS_DYNAMIC_DISPATCH` in a reserved bit (3) of `flags`, covered by
  `content_hash` and so by the signature. The loader recomputes it by
  walking the instructions with `decode_body` (one pass, no allocation) and
  rejects a mismatch, and rejects an instance table when the flag is clear.
- The `LOAD_INSTANCE` handler sits behind a VM Cargo feature, so a
  safety-certified or small embedded build can leave dynamic dispatch out of
  the binary entirely and reject such containers when they load. Expose
  `container.requires_dynamic_dispatch()` (or a general
  `required_features()`) for embedders, and show it in the CLI dump and the
  MCP `compile` result.

## Null and validity

**Proposal:**

- The default value of an interface variable is null: instance index 0.
  `itf := 0` makes it null again (TwinCAT 3.1.4024 accepts it).
- `= 0` and `<> 0` on an interface value compare the index.
- `__ISVALIDREF` on an interface value is a diagnostic. TwinCAT rejects it
  ("Operand for __ISVALIDREF must be of type REFERENCE"), so `= 0` is the
  only null test. `__ISVALIDREF` on `REFERENCE TO` keeps its existing
  meaning.
- Two interface values compare equal (`=`, `<>`) when their indexes are
  equal: one entry per instance, so equal indexes mean the same instance.
  TwinCAT accepts `iA = iB`.
- A call through null traps in `LOAD_INSTANCE`, with a VM error code
  (ADR-0014; the category is chosen when implementing).
- TwinCAT also stops at such a call, but without a diagnostic: the runtime
  goes to ERROR with no call stack and no message (see
  [TwinCAT behaviour checked](#twincat-behaviour-checked)). A trap naming
  the call site keeps the outcome and improves the diagnosis.

## `__QUERYINTERFACE` / `__QUERYPOINTER`

TwinCAT only allows `__QUERYINTERFACE` on interfaces that extend the
built-in `__SYSTEM.IQueryInterface`. Supporting it therefore also means
providing that interface.

**Proposal:** a follow-up PR after tier 2, since the usage patterns don't use
them. Both compile the same way as a call. `__QUERYINTERFACE(src, dst)`
branches over the implementers of the source interface. For those that also
implement the target interface, it copies the index and returns
`TRUE`. Otherwise it returns `FALSE`. No table is needed.

## TwinCAT behaviour checked

Checked in XAE 3.1.4024 on 2026-09-27, on a local runtime.

| Statement | TwinCAT |
|---|---|
| FB instance to interface | accepted |
| `REFERENCE TO` FB to interface | accepted |
| Interface upcast by `:=` | accepted |
| Interface downcast by `:=` | error |
| `iA = 0`, `iA = iB` | accepted |
| `__ISVALIDREF` on an interface | error |
| `__QUERYINTERFACE` without `__SYSTEM.IQueryInterface` | error |
| `REF=` of an unrelated FB | error |
| `ADR()` of an unrelated FB into `POINTER TO` base | accepted, no warning |
| Call through a never-assigned interface | compiles without a warning; at run time the runtime goes to state ERROR, with an empty call stack and nothing in the error list |

## Memory and safety

- No dynamic allocation in either tier. Dynamic dispatch doesn't need it:
  every instance is declared statically, and an interface value only refers
  to one. `METHOD_CALL` already takes the instance from the stack at run
  time and the body as a compile-time operand, so changing which object a
  call works on is already just a value. This fits `no_std` (ADR-0010).
- **ADR-0005** rejected runtime-tag dispatch because a correct tag is "an
  assumption, not a proof". The argument for M2b is that the runtime value
  selects between statically known, verified direct calls, like a `CASE`
  statement does, and that the pairing of instance and type is enforced by
  the VM.
- **What a forged M2 reference could do** (the question this section left
  open): `METHOD_CALL`'s copy-in and copy-out are bounds-checked against the
  whole data region, not the instance. With a dispatch id saying type A and
  an offset pointing at an instance of type B, A's method copies A's fields
  starting at B's offset and writes them back. Rust stays memory-safe, but
  the method silently overwrites neighbouring variables: corrupted process
  values with no trap. That is what rules out M2.
- **With M2b**, neither half of the pair comes from writable memory, so they
  can't disagree. A corrupted index either traps or names a real instance of
  a real implementer, whose own method then runs on it: a wrong-object bug,
  not memory corruption.
- ADR-0017, ADR-0026 and ADR-0027: an interface value is one ordinary slot.
  Instance layout and field offsets don't change.

## Verification

**Proposal:**

- At load time, the verifier checks each instance table entry once: it is
  inside the data region, doesn't overlap another entry, and its dispatch id
  is a type that implements an interface. The branches themselves are
  ordinary direct calls the verifier already checks.
- Two rules that "dynamic dispatch if and only if `LOAD_INSTANCE`" depends
  on, to write into the Phase 2 ADR:
  1. `METHOD_CALL` never takes its body from the stack. A variant that pops
     a function id would allow dynamic dispatch without `LOAD_INSTANCE`.
  2. The instance reference a `METHOD_CALL` or `FB_CALL` uses comes from
     `LOAD_INSTANCE`, or from `FB_LOAD_INSTANCE` of a slot no instruction
     stores to. Today `FB_LOAD_INSTANCE` reads a writable slot, so
     hand-crafted bytecode can already point a static call at another type's
     data ([#1997](https://github.com/ironplc/ironplc/issues/1997)). That is
     independent of this design and can be fixed first.

## Container format and debug info

**Proposal:**

- An instance table section, read in place.
- `FLAG_HAS_DYNAMIC_DISPATCH` in the header (see [Opt-in](#opt-in)).
- For the debugger, a table from dispatch id to type name in the debug
  section, so that an interface variable shows its concrete type. This can
  come after tier 2.

## Adjacent: FB_init

**Proposal:** a separate issue, in the same milestone as tier 2. The state
pattern declares instances as `FB_X(THIS)`, so tier 2 alone doesn't compile
it. `FB_init` is construction, not dispatch, and doesn't block this design.

## Requirements

REQ IDs (`REQ-OOP-<crate>-NNN`) are added once a section becomes normative,
together with their conformance tests.

## Open questions

- **Decided:** M2b as the mechanism (from the review).
- **Decided:** no separate compilation and no link step for now.
- **Needs decision:** an interface value from a `REFERENCE TO` an FB in the
  first version, or a diagnostic (see [Representation](#representation)).
- **Needs decision:** the compiler flag and the run-time policy as proposed
  in [Opt-in](#opt-in).
- To measure: how many interface variables the static resolution keeps in
  tier 1 on the #1199 corpus.

## Related

- [ADR-0041](../adrs/0041-staged-method-and-interface-dispatch.md)
- [ADR-0055](../adrs/0055-concrete-type-ids-numbered-by-debug-tag.md) concrete type ids
- [#1434](https://github.com/ironplc/ironplc/issues/1434) OOP dispatch surface
- [#1692](https://github.com/ironplc/ironplc/issues/1692) PROPERTY
- [#1438](https://github.com/ironplc/ironplc/issues/1438) EXTENDS layout
- [#1419](https://github.com/ironplc/ironplc/issues/1419) interfaces and IMPLEMENTS
