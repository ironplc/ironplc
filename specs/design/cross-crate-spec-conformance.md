# Design: Cross-Crate Spec Conformance

> **Status:** Proposal. This document explores how to let a single design
> spec link conformance tests that live in more than one crate. It builds on
> [spec-conformance-testing.md](./spec-conformance-testing.md), which describes
> the existing single-crate mechanism.

## Overview

Today a design spec is bound to exactly one crate: whichever crate's
`build.rs` lists the spec markdown becomes accountable for testing *every*
requirement in it. This document proposes making requirement **ownership
explicit** — via a crate slug embedded in the requirement ID — so that one
design doc can distribute its requirements across several crates while
keeping the bidirectional validity/completeness guarantees intact.

## Problem

The current pipeline (see [spec-conformance-testing.md](./spec-conformance-testing.md))
has three pieces:

1. Requirement IDs `**REQ-XX-NNN**` in `specs/design/*.md`, where `XX` is a
   2-letter *area* code (`CF` = container format, `IS` = instruction set,
   `EN` = enumeration, `VC` = vm-cli, …).
2. `spec_requirements_gen::generate(&["spec.md"])`, called from a crate's
   `build.rs`. It parses the listed markdown for requirement markers, scans
   **that crate's own** `src/` and `tests/` for `#[spec_test(REQ_XX_NNN)]`,
   and emits `spec_requirements.rs` (a const per requirement, plus `ALL` and
   `UNTESTED`).
3. An `all_spec_requirements_have_tests` meta-test per crate asserting
   `UNTESTED.is_empty()`.

Ownership is **implicit and total**: `generate()` only scans one crate's
source tree, so the listing crate must test *all* of the spec's
requirements. If two crates listed the same `.md`, each would see the other's
tests as `UNTESTED`, and both meta-tests would fail. A design doc is therefore
pinned to a single crate.

This blocks the natural case where a feature's requirements are verified in
different layers — for example, a language feature whose syntax is tested in
`parser`, semantics in `analyzer`, codegen in `codegen`, and runtime behavior
in `vm`. Under the current design those must either be split into four
separate spec docs (fragmenting one design across four files) or forced into a
single crate (writing awkward cross-layer tests in one place).

## Approach

Make each requirement declare which crate owns its test. `generate()` becomes
crate-aware: a crate is accountable only for the requirements whose owner
matches its own slug. Every crate that participates in a spec lists the same
`.md` in its `build.rs`; each writes tests for its own subset; each meta-test
passes independently.

### Requirement ID grammar

Extend the ID with an optional crate slug placed between the area code and the
number:

```
**REQ-<AREA>-<crate-slug>-<NNN>**
        |          |          |
        |          |          +-- trailing digits (zero-padded)
        |          +------------- lowercase crate slug, may contain '-'
        +------------------------ uppercase area code
```

Examples:

| ID                    | Area | Crate slug | Number | Notes                      |
|-----------------------|------|------------|--------|----------------------------|
| `REQ-RT-vm-001`       | `RT` | `vm`       | `001`  | Owned by crate `vm`        |
| `REQ-RT-vm-cli-001`   | `RT` | `vm-cli`   | `001`  | Hyphenated slug            |
| `REQ-CF-001`          | `CF` | *(none)*   | `001`  | Legacy — owned by lister   |

**Parsing rule** (anchor on the ends, not the delimiters, so hyphenated slugs
work): strip the `REQ-` prefix; the leading `[A-Z0-9]+` run is the area code
and the trailing `[0-9]+` run is the number; whatever lowercase text remains
in the middle is the crate slug. An empty middle means a legacy, unslugged
requirement.

The Rust identifier form is unchanged — hyphens become underscores as today:
`REQ-RT-vm-cli-001` → `REQ_RT_vm_cli_001`, referenced by
`#[spec_test(REQ_RT_vm_cli_001)]`.

### Crate-aware generation

`generate()` learns the current crate's canonical slug and partitions
requirements by owner. The slug can be derived from `CARGO_PKG_NAME` by
stripping the `ironplc-` prefix (which already equals the crate's directory
name — `ironplc-vm-cli` → `vm-cli`), or passed explicitly for clarity:

```rust
// build.rs
fn main() {
    ironplc_spec_requirements_gen::generate_for_crate(
        "vm",                       // this crate's slug
        &["user-defined-function-calls-design.md"],
    );
}
```

Define `owner(req)` = the requirement's crate slug if present, otherwise the
current crate's slug (preserving legacy behavior). Then:

- `UNTESTED` = { `req` : `owner(req) == my_slug` **and** `req` not tested in
  this crate's `src`/`tests` }.
- Requirements owned by another crate are skipped entirely — that crate is
  accountable for them, not this one.
- `ALL` is filtered to the crate's owned subset (it is informational only; no
  code outside the generated module consumes it today).

With this change, N crates may each list the same `.md`. Each tests only its
`REQ-*-<its-slug>-*` requirements, and every meta-test passes. The validity
guarantee is unchanged: removing a requirement from the spec still breaks
compilation of any `#[spec_test]` that references it, in whichever crate that
test lives.

### Closing the orphan gap

Per-crate decoupling introduces one hole: a requirement slugged for crate
`foo` where **no** crate with slug `foo` lists that doc would never be
checked by any meta-test. A single workspace-level guard closes it:

> A workspace conformance test reads every `specs/design/*.md`, collects the
> distinct crate slugs used in requirement IDs, and asserts each slug is
> claimed by at least one crate's `build.rs` (i.e., some crate with that slug
> lists that doc). Any orphaned slug fails the test with an actionable
> message.

This test lives once (e.g., in the `test` crate) and needs a small manifest of
"which crate lists which doc." That mapping can itself be generated by having
each `generate_for_crate` call record `(slug, doc)` pairs into a shared file,
or maintained explicitly. The exact placement is left to the implementation
plan.

## Trade-offs

**Embedding the crate in the ID couples requirement identity to test
location.** Requirement IDs are meant to be permanent and never reused. If a
test later moves crates during a refactor, its slug — and therefore its ID and
every reference to it — churns. This is the cost of the ergonomic win.

The main alternative keeps IDs area-only and declares ownership *separately*:

- **Per-crate range/prefix table in `build.rs`** — e.g. crate `vm` claims
  `REQ-RT-100..199`. IDs stay stable across refactors, but ownership is no
  longer visible at the call site, and ranges are brittle as specs grow.
- **Grouping markers in the markdown** — e.g. a `<!-- crate: vm -->` comment
  before a block of requirements. Stable IDs, ownership visible in the spec,
  but not visible when writing the *test*, and easy to get out of sync with
  the `build.rs` listing.

The slug-in-ID approach is recommended because it optimizes the thing that is
actually painful: **when you write a test, the ID tells you exactly which
crate it belongs in**, and every association is greppable from a single token.
ID churn on cross-crate refactors is rare and, when it happens, is a mechanical
find-and-replace that the validity check catches immediately.

## Migration

The grammar is backward compatible: existing `REQ-XX-NNN` IDs have an empty
crate slug and continue to be owned by their single listing crate, so no
existing spec or test needs to change. New cross-crate specs adopt the slug
form. Existing specs can be migrated incrementally, one requirement at a time,
following the same reconcile flow used today (see
[reconcile-spec](../../.claude/commands/reconcile-spec.md)).

## Summary of changes

| Component | Change |
|-----------|--------|
| Requirement ID grammar | Optional `<crate-slug>` segment between area and number |
| `spec_requirements_gen` | Parse the slug; add crate-aware `generate_for_crate`; filter `UNTESTED`/`ALL` by owner |
| Participating `build.rs` | Pass the crate slug; multiple crates may list the same `.md` |
| Workspace test | New guard asserting every slug is claimed by some crate |
| Steering docs | Document the extended grammar in `development-standards.md` and `spec-conformance-testing.md` |
| `#[spec_test]` macro | No change — ident form already handles the extra segment |
