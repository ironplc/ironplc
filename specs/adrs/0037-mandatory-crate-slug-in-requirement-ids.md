# ADR-0037: Mandatory Crate Slug in Spec-Conformance Requirement IDs

status: proposed
date: 2026-07-21

## Context and Problem Statement

IronPLC links design specifications to conformance tests through requirement
IDs. Each testable claim in a `specs/design/*.md` document carries an ID such
as `**REQ-CF-001**`, where `CF` is a 2-letter *area* code identifying the
design section. A build script (`spec_requirements_gen`) parses the markdown,
scans a crate's own source tree for `#[spec_test(REQ_CF_001)]` annotations, and
a per-crate meta-test asserts every requirement has a test. See
[spec-conformance-testing.md](../design/spec-conformance-testing.md).

The enforcement binds each spec document to exactly **one** crate. The build
script only scans the source tree of the crate whose `build.rs` lists the
document, so that crate must contain a test for *every* requirement in it. If
two crates listed the same document, each would see the other's tests as
missing and both meta-tests would fail.

This single-crate coupling blocks features whose requirements are naturally
verified in different layers of the compiler. A language feature might have its
syntax validated in `parser`, its semantics in `analyzer`, its code generation
in `codegen`, and its runtime behavior in `vm`. Under the single-crate model,
such a feature's requirements must either be fragmented across several design
documents (one per crate) or forced into a single crate as awkward cross-layer
tests. Neither preserves "one design document per feature."

The question this ADR settles: **how does a requirement declare which crate
owns its conformance test, so one design document can span crates?**

## Decision Drivers

* **One design per feature** — a feature that spans compiler layers should be
  describable in a single design document, not split to satisfy the test
  infrastructure
* **Obvious ownership at the point of writing a test** — when adding a test, it
  should be immediately clear which crate the test belongs in
* **Greppability** — the association between a requirement and its owning crate
  should be recoverable from a single token
* **No silent gaps** — the mechanism must not allow a requirement to exist with
  no crate accountable for testing it
* **Simplicity of the enforcement code** — fewer special cases in
  `spec_requirements_gen` and the meta-tests

## Considered Options

* **Optional crate slug (backward compatible)** — add a slug to the ID
  (`REQ-EN-vm-005`) but keep the unslugged form (`REQ-EN-001`) valid, treating
  an unslugged requirement as owned by whichever single crate lists its
  document
* **Mandatory crate slug (remove the single-crate model)** — require a slug in
  every requirement ID; reject the unslugged form at build time; migrate all
  existing requirements
* **Separate ownership declaration** — keep IDs area-only and declare ownership
  elsewhere (a per-crate ID-range table in `build.rs`, or grouping markers in
  the markdown)

## Decision Outcome

Chosen option: **Mandatory crate slug (remove the single-crate model).**

Every requirement ID carries an owning-crate slug between the area code and the
number:

```
**REQ-<AREA>-<crate-slug>-<NNN>**
```

For example `REQ-EN-codegen-001` (owned by `codegen`) and `REQ-EN-vm-005`
(owned by `vm`) both belong to the enumeration (`EN`) design area but are
tested in different crates. The unslugged form `REQ-EN-001` is no longer valid.

`spec_requirements_gen` derives the current crate's slug from `CARGO_PKG_NAME`
(stripping the `ironplc-` prefix, which equals the crate directory name),
treats a requirement's slug as its sole owner with no fallback, and **panics at
build time** if a document it parses contains a `**REQ-…**` marker with no
slug. A crate's completeness meta-test considers only the requirements it owns.
A single workspace-level guard test asserts that every requirement is slugged
and that every slug is claimed by a crate that lists the corresponding
document, closing the gap where a requirement could name a crate that does not
list its document.

The area code is retained. In a single-crate document it can look redundant
with the slug, but it is the only field that groups requirements from the same
design concern once that concern owns requirements in multiple crates.

### Why not the optional slug

Backward compatibility keeps the unslugged form alive, which forces the
enforcement code to carry a fallback ("unslugged means owned by the sole
lister") and a fragile invariant ("a document may keep unslugged requirements
only while exactly one crate lists it"). The moment a second crate joins such a
document, its legacy requirements must be slugged anyway, or a meta-test breaks
silently. The optional form therefore preserves the exact foot-gun the change
is meant to remove, in exchange for avoiding a one-time migration. Removing the
form outright deletes the fallback, the invariant, and a class of silent
failure.

### Why not a separate ownership declaration

Declaring ownership in a `build.rs` range table or markdown grouping markers
keeps requirement IDs stable across refactors, but it hides ownership from the
place it matters most — the `#[spec_test(...)]` call site — and adds a second
source of truth that drifts from the ID. Encoding the crate in the ID makes
ownership visible wherever the ID appears and greppable from one token, which
directly serves the "obvious ownership" and "greppability" drivers.

### Consequences

* Good, because one design document can distribute its requirements across any
  number of crates, so a cross-layer feature is described in one place
* Good, because the owning crate is visible at the test call site and in the
  spec, recoverable by grepping a single token
* Good, because removing the unslugged form deletes the backward-compatibility
  fallback and the single-lister invariant, simplifying the enforcement code
* Good, because the build-time panic on an unslugged marker makes the
  single-crate model impossible to use even by accident
* Bad, because requirement identity is now coupled to test location: moving a
  test between crates changes its slug, its ID, and every reference. The
  compile-time validity check catches stale references immediately, so the fix
  is a mechanical find-and-replace, but the churn is real
* Bad, because adopting the change requires a one-time migration of every
  enforced requirement ID (~135 markers) and every `#[spec_test]` reference
  (~166) to the slugged form; this is mechanical and guarded by the build

## More Information

The full mechanism, grammar, and migration scope are described in
[cross-crate-spec-conformance.md](../design/cross-crate-spec-conformance.md).
This ADR supersedes the single-crate assumption documented in
[spec-conformance-testing.md](../design/spec-conformance-testing.md); that
document is updated to describe the mandatory-slug grammar.

Documents that carry requirement IDs but are not yet wired into conformance
(`partial-access-bit-syntax.md`, `subrange-codegen.md`, `time-literals.md`,
`mcp-server-distribution.md`) are unaffected until they are first wired, at
which point they adopt the slugged form.
