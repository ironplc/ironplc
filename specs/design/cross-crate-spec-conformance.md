# Design: Cross-Crate Spec Conformance

> **Status:** Proposal. This document describes how a single design spec links
> conformance tests that live in more than one crate. It supersedes the
> single-crate model in
> [spec-conformance-testing.md](./spec-conformance-testing.md): the crate slug
> is **mandatory** and the single-crate form is removed entirely.

## Overview

In the original model a design spec is bound to exactly one crate: whichever
crate's `build.rs` lists the spec markdown is accountable for testing *every*
requirement in it. That coupling blocks features whose requirements are
naturally verified in different layers. This design makes requirement
**ownership explicit and required** — every requirement ID carries a crate
slug — so one design doc can distribute its requirements across any number of
crates. There is no unslugged form and no backward-compatible fallback.

## Problem

The original pipeline (see [spec-conformance-testing.md](./spec-conformance-testing.md))
has three pieces:

1. Requirement IDs `**REQ-XX-NNN**` in `specs/design/*.md`, where `XX` is a
   2-letter *area* code (`CF` = container format, `EN` = enumeration, …).
2. `spec_requirements_gen::generate(&["spec.md"])`, called from a crate's
   `build.rs`. It parses the listed markdown for requirement markers, scans
   **that crate's own** `src/` and `tests/` for `#[spec_test(REQ_XX_NNN)]`,
   and emits `spec_requirements.rs` (a const per requirement, plus `ALL` and
   `UNTESTED`).
3. An `all_spec_requirements_have_tests` meta-test per crate asserting
   `UNTESTED.is_empty()`.

Ownership is **implicit and total**: `generate()` only scans one crate's
source tree, so the listing crate must test *all* of the spec's requirements.
If two crates listed the same `.md`, each would see the other's tests as
`UNTESTED`, and both meta-tests would fail. A design doc is therefore pinned to
a single crate.

This blocks the natural case where a feature's requirements span layers — for
example, a language feature whose syntax is tested in `parser`, semantics in
`analyzer`, codegen in `codegen`, and runtime behavior in `vm`. Under the
original model those must either be split into separate spec docs (fragmenting
one design across several files) or forced into a single crate (writing
awkward cross-layer tests in one place).

## Approach

Every requirement declares which crate owns its test, in the ID itself.
`generate()` is crate-aware: a crate is accountable only for the requirements
whose slug matches its own. Every crate that participates in a spec lists the
same `.md` in its `build.rs`; each writes tests for its own subset; each
meta-test passes independently. The unslugged form is rejected at build time,
so the single-crate model cannot be used even by accident.

### Requirement ID grammar

Every ID carries a crate slug between the area code and the number:

```
**REQ-<AREA>-<crate-slug>-<NNN>**
        |          |          |
        |          |          +-- trailing digits (zero-padded)
        |          +------------- REQUIRED lowercase crate slug, may contain '-'
        +------------------------ uppercase area code
```

Examples:

| ID                       | Area | Crate slug  | Number | Owner        |
|--------------------------|------|-------------|--------|--------------|
| `REQ-EN-codegen-001`     | `EN` | `codegen`   | `001`  | crate codegen|
| `REQ-EN-vm-005`          | `EN` | `vm`        | `005`  | crate vm     |
| `REQ-VC-vm-cli-001`      | `VC` | `vm-cli`    | `001`  | crate vm-cli |

The **area** groups requirements by design section (e.g. all enumeration
requirements share `EN`); the **slug** names the owning crate. They are
independent: the same area can own requirements in several crates, which is
exactly the cross-crate case. `REQ-EN-001` (no slug) is **not** a valid ID.

**Parsing rule** (anchor on the ends so hyphenated slugs like `vm-cli` work):
strip `REQ-`; the leading `[A-Z0-9]+` run is the area, the trailing `[0-9]+`
run is the number, and the lowercase text in between is the crate slug. An
empty middle is an error, not a legacy case.

The Rust identifier form is unchanged — hyphens become underscores:
`REQ-VC-vm-cli-001` → `REQ_VC_vm_cli_001`, referenced by
`#[spec_test(REQ_VC_vm_cli_001)]`.

### Crate-aware generation

`generate()` derives the current crate's slug from `CARGO_PKG_NAME` (strip the
`ironplc-` prefix, which equals the crate's directory name — `ironplc-vm-cli`
→ `vm-cli`). For every requirement it parses, `owner(req)` is the requirement's
slug; there is no fallback. Then:

- **Build-time rejection:** if a listed doc contains a `**REQ-…**` marker with
  no crate slug, `generate()` panics with an actionable message. This is what
  enforces the removal of the single-crate form — an unslugged requirement
  fails the build of any crate that lists its doc.
- `UNTESTED` = { `req` : `owner(req) == my_slug` **and** `req` not tested in
  this crate's `src`/`tests` }.
- `ALL` is filtered to the crate's owned subset (informational; nothing outside
  the generated module consumes it today).

With this change, N crates may each list the same `.md`. Each tests only its
`REQ-*-<its-slug>-*` requirements, and every meta-test passes. The validity
guarantee is unchanged: removing a requirement from the spec still breaks
compilation of any `#[spec_test]` that references it, in whichever crate that
test lives.

### Workspace orphan guard

Per-crate decoupling leaves one hole: a requirement slugged for crate `foo`
where **no** crate with slug `foo` lists that doc would never be checked by any
meta-test. A single workspace-level test closes it:

> The guard parses every enforced `specs/design/*.md` (those listed by some
> `build.rs`) for requirement IDs, recovering each `(doc, slug)`. It parses
> every `compiler/*/build.rs` for the `.md` filenames it lists, recovering the
> crate slug from the `build.rs` directory name, giving the `(slug, doc)`
> listing set. It asserts every `(slug, doc)` used by a requirement is claimed
> by a listing crate, and that no enforced requirement lacks a slug. Any orphan
> or unslugged requirement fails with an actionable message.

Both sides are recovered from files already in the tree — no separate manifest
to keep in sync.

## Trade-offs

**Embedding the crate in the ID couples requirement identity to test
location.** Requirement IDs are permanent and never reused. If a test moves
crates during a refactor, its slug — and therefore its ID and every reference —
churns. The validity check catches stale references at compile time, so the
fix is a mechanical find-and-replace, but the churn is real. This is the
accepted cost of making ownership obvious at the point where a test is written:
the ID tells you exactly which crate the test belongs in, and every association
is greppable from a single token.

The area code (`EN`, `VC`, …) is retained even though it can look redundant in
the single-crate-per-doc case. It earns its place in the cross-crate case,
where one area owns requirements in multiple crates and the area is the only
thing that still groups them as one design concern.

## Migration

There is no compatibility shim, so every enforced requirement is renamed to
carry its owning crate's slug. The enforced set is the docs listed by a
`build.rs`:

| Doc(s)                                                   | Crate     | Areas          |
|----------------------------------------------------------|-----------|----------------|
| `bytecode-container-format.md`, `bytecode-instruction-set.md` | container | `CF`, `IS`     |
| `enumeration-codegen.md`                                 | codegen   | `EN`           |
| `mcp-server.md`                                          | mcp       | `STL`,`TOL`,`ARC` |
| `vm-cli.md`                                              | vm-cli    | `VC`           |

For each, every `**REQ-XX-NNN**` in the markdown becomes
`**REQ-XX-<crate>-NNN**`, and every `#[spec_test(REQ_XX_NNN)]` reference
becomes `#[spec_test(REQ_XX_<crate>_NNN)]`. Because each of these docs is
currently owned by a single crate, the slug inserted is that crate's slug
throughout — a purely mechanical rename per doc, verified by the build (any
missed reference fails to compile; any missed marker trips the guard).

Unwired docs that carry REQ IDs but no `build.rs` (`partial-access-bit-syntax.md`,
`subrange-codegen.md`, `time-literals.md`, `mcp-server-distribution.md`) are
**not** migrated now. They are unenforced today; they adopt the slug form when
they are first wired into conformance.

## Summary of changes

| Component | Change |
|-----------|--------|
| Requirement ID grammar | Mandatory `<crate-slug>` segment; unslugged form removed |
| `spec_requirements_gen` | Parse the slug; derive crate slug; `owner(req)` = slug with no fallback; **panic on unslugged marker**; filter `UNTESTED`/`ALL` by owner |
| Enforced specs + tests | Rename every enforced requirement ID and every `#[spec_test]` reference to include the crate slug |
| Participating `build.rs` | Unchanged signature; multiple crates may now list the same `.md` |
| Workspace test | New guard asserting every requirement is slugged and every slug is claimed |
| Steering docs | Document the mandatory grammar in `development-standards.md` and `spec-conformance-testing.md` |
| `#[spec_test]` macro | No change — the ident form already handles the extra segment |
