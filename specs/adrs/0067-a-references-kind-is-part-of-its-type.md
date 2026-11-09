# A Reference's Kind Is Part of Its Type

status: proposed
date: 2026-11-01

## Context and Problem Statement

IronPLC accepts two reference syntaxes with two usage models
([reference-to-twincat.md](../design/reference-to-twincat.md)):

* An **explicit** reference: IEC 61131-3 `REF_TO`, and CODESYS `POINTER TO`.
  It is bound with `r := REF(x)` (or `ADR(x)`) and read and written through
  with `r^`.
* An **alias** reference: CODESYS and TwinCAT `REFERENCE TO`. It is bound with
  `r REF= x`, and a bare `r` reads and writes `x`.

The `codesys` dialect enables both, and `twincat` enables only `REFERENCE TO`
([ADR-0038](0038-no-restrictions-on-flag-combinations.md), postscript).

Both resolve to one representation. The parser tags each declaration with a
`RefSyntax`, but the analyzer resolves both to `SemanticType::Reference {
target_type }`, and `TypeEnvironment::reference_to` interns one type per
target. So a `REF_TO INT` and a `REFERENCE TO INT` are the same `TypeId`. The
design records this on purpose: `REFERENCE TO` "maps onto the existing AST"
and "needs no new backend", and for an array element "the analyzer and
codegen collapse [the tag] to a bool at their boundaries, leaving the type
system and backend unchanged". Alias behaviour comes from one pass,
`xform_insert_implicit_deref`. It collects the names that the current POU
declares `REFERENCE TO` in its own variable sections, and rewrites bare uses
of those names into explicit dereferences. ADR-0038 names that
per-declaration tag as what keeps the two kinds unambiguous in one program.

Because the alias behaviour hangs on a declaration the pass can see by name,
and not on the type, every alias reference reached any other way behaves as
an explicit one. On `main` at `6e40df2`, under `--dialect twincat` unless
noted, with `x : INT := 7`:

| Where the `REFERENCE TO INT` is | Program | IronPLC today |
|---|---|---|
| A variable of the POU itself | `r REF= x; y := r; r := 5;` | Accepted; reads and writes `x` |
| An input of another block's instance | `b.r REF= x; y := b.r;` | P2032 |
| An element of an array | `refs[0] REF= x; y := refs[0];` | P2032; `refs[0]^` is accepted instead |
| A variable of a named type `TYPE IR : REFERENCE TO INT` | `r REF= x; y := r;` | P2032; `r^` is accepted instead |
| A structure field of type `IR` | `s.r REF= x; y := s.r;` | P2032; `s.r^` is accepted instead |
| A block input, bound in the call | `b(r := x);` | P9999 |
| A function input, bound in the call | `F(x)`, `F(r := x)` | P4026 |
| `REF()` of the reference, `codesys` | `rr REF= x; rt := REF(rr); y := rt^;` | Accepted; `rt` refers to `rr` itself, and `y` is 0, not 7 |

A structure field written `r : REFERENCE TO INT` in place does not parse
(P0002); that is a parser gap, not part of this decision.

The same gap now limits a semantic rule. When
[#2520](https://github.com/ironplc/ironplc/pull/2520) made a function block's
reference input check a reference given to it, it had to leave `b(r := x)`
alone: for a `REF_TO` input that is a mistake, and for a `REFERENCE TO` input
it is how CODESYS and TwinCAT bind the reference, and the input's type cannot
say which keyword declared it.

[ADR-0036](0036-no-ironplc-dialect.md) makes each row where IronPLC rejects
or misreads a program the real target accepts a defect, not a dialect to
preserve. The array row is the exception: TwinCAT does not declare an array
of references at all (question 4 below).

Where does IronPLC record whether a reference is an alias, so that every
place holding one behaves as its declaring keyword says?

## Decision Drivers

* **The real target's behaviour wherever a reference is held**
  ([ADR-0036](0036-no-ironplc-dialect.md)): a variable, a field, an element,
  a named type, an input and an argument alike.
* **One place answers "is this reference an alias?"**, and every pass and
  rule asks it ([compiler-standards.md](../steering/compiler-standards.md),
  "Classify an enum's variants in one place").
* **Both kinds stay unambiguous in one program**, since a shipped dialect
  enables both ([ADR-0038](0038-no-restrictions-on-flag-combinations.md),
  corollary).
* **The analyzer decides and lowering translates**
  ([ADR-0058](0058-the-analyzer-decides-and-lowering-translates.md)): alias
  behaviour is resolved before lowering, which sees explicit dereferences
  only.
* **Backends stay as they are**
  ([ADR-0057](0057-backends-consume-a-target-neutral-lowered-program.md),
  [ADR-0059](0059-backends-depend-only-on-the-ir-crate.md)). At run time both
  kinds are a variable index, and nothing a backend does depends on the kind.

## Considered Options

* **A** — One type; alias behaviour for the names a POU declares (today).
* **B** — One type; record the keyword on every declaration a reference can
  be held in, and look it up by place.
* **C** — The kind is part of the analyzer's reference type, and lowering
  drops it.
* **D** — The kind is part of the type through the lowered program, so
  backends see it.

## Decision Outcome

Proposed option: **C — the kind is part of the analyzer's reference type**,
because it is the only option where every place holding a reference knows its
kind by the type the analyzer already computes for that place, and it leaves
the lowered program and the backends unchanged.

* **The type.** `SemanticType::Reference` gains a kind with two values,
  `Explicit` for `REF_TO` and `POINTER TO`, and `Alias` for `REFERENCE TO`.
  `TypeEnvironment::reference_to(target, kind)` interns one type per target
  and kind. So `REF_TO INT` and `REFERENCE TO INT` are two types, and
  `REF_TO INT` and `POINTER TO INT` stay one, as they are today.
* **One classifier.** A method on the type environment says whether a type
  is an alias reference, and every pass and rule asks it rather than matching
  on the kind.
* **Alias behaviour keys on the type of a place.** `xform_insert_implicit_deref`
  types each place it rewrites by the walk the rules already share,
  `variable_type::type_id`, instead of by the names it collected. It already
  runs after the symbol environment is built and before call binding and
  expression typing, so it moves in the pipeline not at all; it takes the
  symbol and type environments it does not take today. A field, an element,
  a variable of a named type and a structure field then read and write
  through, as the POU's own variable does.
* **A call binds an alias input.** A value given to an input whose type is an
  alias reference binds it, as `r REF= x` does, and reaches lowering as
  `REF(x)`. Rules then judge it as a binding, so `rule_call_input_type` can
  check a value given to an explicit reference input as well.
* **Lowering drops the kind.** `lowering/src/types.rs` builds the IR
  reference type from the target alone, as it does now. The IR, the bytecode
  backend and the VM do not change.
* **Diagnostics name the kind.** A reference type is spelled as its kind is
  written, `REF_TO INT` or `REFERENCE TO INT`, wherever a diagnostic or
  `echo --types` names it.

### Consequences

* Good, because every row of the table above behaves as CODESYS and TwinCAT
  do, from one rule rather than one rule per position.
* Good, because a semantic rule can tell an alias reference from an explicit
  one by type, which `rule_call_input_type` needs to finish the check #2520
  left half done.
* Good, because ADR-0038's corollary holds more strongly: the two kinds are
  unambiguous in one program wherever a reference is held, not only where the
  POU declares it.
* Good, because the run-time model is untouched: no IR change, no backend
  change and no new opcode.
* Neutral, because `REF_TO INT` and `POINTER TO INT` stay one type; whether
  CODESYS pointers ever need a kind of their own is a separate question.
* Bad, because every site that matches `SemanticType::Reference` or asks
  `is_reference` must decide what it means for each kind. There are dozens in
  the analyzer, a few in lowering, and some in the MCP server.
* Bad, because programs change what they get. Programs the table shows
  rejected or misread are accepted and read through, and `rt := REF(rr)`
  changes meaning. Each such change is a compatibility decision a person
  makes (see the questions below).

### Confirmation

* `reference-to-twincat.md` gains a requirement for each position in the
  table, under its existing `REQ-RTO-*` areas, and the table becomes those
  requirements' tests, run through the VM where the program compiles.
* An analyzer test shows `REF_TO INT` and `REFERENCE TO INT` intern to two
  type ids, and `REF_TO INT` and `POINTER TO INT` to one.
* The core change leaves the sources of the `ir` crate and the bytecode
  backend unchanged; only tests whose expectations the questions below
  change are edited there.
* On acceptance, `reference-to-twincat.md` stops saying the analyzer
  collapses the tag, and ADR-0038 gains a postscript that its corollary is
  now met by the type.

## Pros and Cons of the Options

### A — One type; alias behaviour for the names a POU declares

* Good, because it exists and needs no change.
* Bad, because every alias reference the pass cannot see by name behaves as
  an explicit reference, so IronPLC rejects or misreads programs CODESYS and
  TwinCAT accept, which ADR-0036 makes a defect.
* Bad, because a rule cannot tell the kinds apart, so a check like #2520's
  must leave the ambiguous shape alone.

### B — One type; record the keyword on every declaration

* Good, because the type system is untouched.
* Bad, because a reference is held in places that are not one declaration:
  an array element's kind is the element type's, a named type's is its
  definition's, and a field of an inherited block is declared elsewhere. Each
  is a lookup of its own, the "classify in one place" rule is broken by
  construction, and a new place to hold a reference needs a new table.
* Bad, because it would duplicate what the type of a place already answers.

### C — The kind is part of the analyzer's reference type

* Good, because the walk every rule shares already gives the type of any
  place, so each place knows its kind with no lookup of its own.
* Good, because lowering and the backends do not change.
* Bad, because the change reaches every site that inspects a reference type.

### D — The kind is part of the type through to the backends

* Good, because nothing is erased, so a backend could treat the kinds
  differently.
* Bad, because no backend needs to: both kinds are a variable index, and
  alias behaviour is resolved in the analyzer. It would change the IR and
  every backend for no difference in what they emit
  ([backend-guide.md](../steering/backend-guide.md)).

## More Information

### Questions for the person who decides

Each of these changes what an existing program gets, so a person settles it
before the core change is written.

1. **`REF()` and `ADR()` of an alias reference.** Today `REF(rr)` is a
   reference to the reference variable itself. Proposed: since `rr` names
   the variable it is bound to, `REF(rr)` is a reference to that variable,
   so `rt := REF(rr)` makes `rt` refer to `x`. The vendors' rule is not
   confirmed yet (see below).
2. **Assigning between kinds.** May an explicit reference be bound from an
   alias reference, or the reverse (`rr REF= rt`)? Proposed: an alias
   reference used as a value is the variable it names, so neither kind is
   ever implicitly the other.
3. **`^` on an alias reference.** IronPLC accepts `refs[0]^`, `r^` on a named
   type and `s.r^` today, because a bare read does not go through there.
   Proposed: once it does, follow TwinCAT on whether `^` is still accepted
   on an alias reference, which is to be confirmed.
4. **Arrays of alias references.** Beckhoff's documentation says a reference
   may not be the base type of an array, a pointer or a reference. IronPLC
   accepts `ARRAY [..] OF REFERENCE TO T`, and REQ-RTO-parser-220 and
   REQ-RTO-codegen-420 test it. Proposed: a separate compatibility decision,
   made with this one since the element case is in the table.
5. **Call binding scope.** Binding an alias input in a call can land with the
   type change or after it. Proposed: in the same series, since #2520's check
   waits on it.

### Vendor behaviour to confirm before accepting

These come from search summaries of Beckhoff's and CODESYS's pages. The
session that drafted this ADR could not open the pages, because its network
policy blocks both hosts. Each is to be checked against the pages, or
against the toolchains, before this ADR moves to `accepted`.

* A `REFERENCE TO` input passed in a call is bound with `:=`, not `REF=`
  ([Beckhoff, TwinCAT 3 references](https://infosys.beckhoff.com/content/1033/tc3_plc_intro/136301707.html);
  [CODESYS, REFERENCE data type](https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_datatype_reference.html),
  whose example is `fbDoIt(riInput:=iValue);`).
* A reference type may not be the base type of an array, a pointer or a
  reference, and may not refer to a bit
  ([Beckhoff](https://infosys.beckhoff.com/content/1033/tc3_plc_intro/136301707.html)).
* Not found either way: what `ADR` of a `REFERENCE TO` yields, and whether
  `^` is accepted on one.

### Delivery

Per the [development process](../steering/development-standards.md#development-process),
an issue lists the core change PRs, after a prefactor that routes every site
that asks whether a type is a reference through the one classifier, with one
kind still. The core change then adds the kind, interns by it, and moves the
implicit dereference onto the type; the call binding and the questions above
land as the person decides them.

### Relationship to other records

* **ADR-0038** stands. Its decision is about flag combinations; only the
  mechanism its corollary names, the per-declaration `RefSyntax` tag, is
  replaced by the kind in the type.
* **reference-to-twincat.md** stays right that both kinds share one backend
  and one run-time representation. What changes is its statement that the
  analyzer collapses the tag.
* **ADR-0036** is why each row of the table is a defect to fix rather than a
  behaviour to keep behind a flag.
