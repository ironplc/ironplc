# Plan: Cross-Crate Spec Conformance

## Goal

Let a single design spec link conformance tests that live in more than one
crate, while preserving the existing bidirectional validity (removing a
requirement breaks compilation) and completeness (every requirement has a
test) guarantees. Ownership becomes explicit via an optional crate slug in the
requirement ID, and `spec_requirements_gen` becomes crate-aware.

## Design doc reference

[specs/design/cross-crate-spec-conformance.md](../design/cross-crate-spec-conformance.md)
— proposal, grammar, trade-offs. Builds on
[specs/design/spec-conformance-testing.md](../design/spec-conformance-testing.md).

## Architecture

1. **ID grammar** gains an optional crate slug: `REQ-<AREA>-<crate-slug>-<NNN>`.
   Parsed by anchoring on the ends — leading `[A-Z0-9]+` is the area, trailing
   `[0-9]+` is the number, the lowercase middle is the crate slug (empty =
   legacy). Ident form is unchanged (`REQ-RT-vm-cli-001` → `REQ_RT_vm_cli_001`).

2. **`spec_requirements_gen` is crate-aware.** It derives the current crate's
   slug from `CARGO_PKG_NAME` (strip the `ironplc-` prefix — equals the crate
   directory name), computes `owner(req)` = the requirement's slug if present
   else the current crate's slug, and includes a requirement in this crate's
   `UNTESTED`/`ALL` only when `owner(req) == my_slug`. Requirements owned by
   other crates are skipped. Existing `generate(&[...])` call sites are
   unchanged; an explicit `generate_for_crate(slug, &[...])` override is added
   for cases where the slug should not be auto-derived.

3. **Single-lister invariant.** The "empty slug ⇒ owned by the lister"
   fallback is only sound while a doc has exactly **one** participating crate.
   The moment a second crate lists the same doc, every legacy (unslugged)
   requirement in that doc must be given an explicit slug, otherwise both
   crates would claim the unslugged requirements and one meta-test would fail.
   This is a migration rule, not a code change, and is enforced by the orphan
   guard below.

4. **Workspace orphan guard.** A single workspace-level test parses every
   `specs/design/*.md` for requirement IDs (recovering each requirement's doc
   and slug), parses every `compiler/*/build.rs` for the `.md` filenames it
   lists (recovering the crate slug from the `build.rs` directory name), and
   asserts every `(slug, doc)` pair used by a requirement is claimed by some
   crate that lists that doc. Legacy unslugged requirements are checked to be
   listed by exactly one crate. Any orphan or ambiguity fails with an
   actionable message. This needs no separate manifest — both sides are
   recovered from files already in the tree.

## File map

**Modified**
- `compiler/spec_requirements_gen/src/lib.rs` — slug parsing, crate-slug
  derivation, `owner()` filtering, `generate_for_crate`, unit tests.
- `specs/design/cross-crate-spec-conformance.md` — add the single-lister
  invariant note (item 3 above).
- `specs/design/spec-conformance-testing.md` — cross-reference the extended
  grammar and the cross-crate doc.
- `specs/steering/development-standards.md` — document the optional crate slug
  in the "Design Requirement" section.
- `.claude/commands/reconcile-spec.md` — note the slug form when a spec spans
  crates.

**Created**
- A workspace orphan-guard test. Location: the `ironplc-test` crate
  (`compiler/test/src/…`, added to its build so it runs under `just`), reading
  the repo via `CARGO_MANIFEST_DIR` (`../../specs/design`, `../*/build.rs`).
- Proof-of-concept: a new small design doc under `specs/design/` with two
  slugged requirements owned by two different crates, and the two `#[spec_test]`
  functions plus `build.rs` wiring that exercise the cross-crate path
  end-to-end. (New doc chosen so existing IDs stay untouched.)

**Unchanged (verified)**
- `compiler/spec_test_macro/src/lib.rs` — the ident form already handles the
  extra segment.
- Existing `build.rs` call sites (container, codegen, mcp, vm-cli) — legacy
  behavior preserved via auto-derived slug + single-lister fallback.

## Tasks

### Phase 1 — Generator (crate-aware, backward compatible)
- [ ] Add slug parsing to `spec_requirements_gen`: a helper that splits a raw
      `REQ-…` ID into `(area, slug, number)` by anchoring on the ends; empty
      slug for legacy IDs. Add unit tests including hyphenated slugs
      (`REQ-RT-vm-cli-001`) and legacy IDs (`REQ-CF-001`).
- [ ] Derive the current crate slug from `CARGO_PKG_NAME` (strip `ironplc-`).
      Add `generate_for_crate(slug, files)`; make `generate(files)` delegate to
      it with the derived slug so existing call sites need no change.
- [ ] Compute `owner(req)` and filter `UNTESTED` and `ALL` to
      `owner(req) == my_slug`. Add unit tests covering: a req owned by another
      crate is excluded; an unslugged req is owned by the sole lister; a slugged
      req untested in its owner shows up in that owner's `UNTESTED`.
- [ ] `cargo test -p ironplc-spec-requirements-gen` passes; the four existing
      participating crates still build and their meta-tests still pass
      (no behavior change for single-lister docs).

### Phase 2 — Workspace orphan guard
- [ ] Add the guard test in the `ironplc-test` crate. Parse `specs/design/*.md`
      for requirement IDs → `(doc, slug)`; parse `compiler/*/build.rs` for
      listed `.md` filenames + directory-name slug → `(slug, doc)` listings.
- [ ] Assert: every slugged `(slug, doc)` is claimed by a listing crate; every
      doc with any slugged requirement is listed by every crate whose slug it
      uses; every doc containing legacy unslugged requirements is listed by
      exactly one crate. Failures name the offending requirement, slug, and doc.
- [ ] Add a unit-level self-check (fixture strings) for the guard's parsers so
      the guard itself is covered without depending on live repo state.

### Phase 3 — Proof of concept (end-to-end cross-crate)
- [ ] Add a new small design doc in `specs/design/` describing one behavior
      that genuinely spans two crates, with two slugged requirements (one per
      crate). Recommended pair: `codegen` (has infra) + `vm` (adjacent runtime).
- [ ] Wire both crates' `build.rs` to list the new doc (adding a `build.rs` and
      `spec_requirements`/`spec_conformance` modules to `vm` if absent, mirroring
      the container reference).
- [ ] Add one `#[spec_test(REQ_…_codegen_…)]` in codegen and one
      `#[spec_test(REQ_…_vm_…)]` in vm; confirm both meta-tests pass with the
      same doc listed by both crates.
- [ ] Confirm the negative paths by hand: (a) removing a requirement from the
      doc breaks compilation of its `#[spec_test]`; (b) omitting a slugged
      test leaves that owner's `UNTESTED` non-empty; (c) a slug typo in the doc
      trips the orphan guard.

### Phase 4 — Docs and CI
- [ ] Update `spec-conformance-testing.md`, `development-standards.md`, and
      `reconcile-spec.md` per the file map. Add the single-lister invariant note
      to the design doc.
- [ ] Run `cd compiler && just` (compile, coverage ≥ 85%, clippy, fmt) — all
      green — before opening any PR.

## Risks and mitigations

- **ID churn on refactor.** Moving a test between crates changes its slug and
  every reference. Mitigated by: the validity check catches stale references at
  compile time (mechanical find-and-replace), and this is expected to be rare.
- **Second-lister migration foot-gun.** Adding a second crate to an existing
  single-crate doc without slugging its legacy requirements silently breaks a
  meta-test. Mitigated by the orphan guard, which fails with a message pointing
  at the unslugged requirements that now need slugs.
- **`build.rs` parsing brittleness in the guard.** The guard reads `.md`
  filenames from `build.rs` source text. Mitigated by covering the parser with
  fixture-based unit tests and keeping the match narrow (quoted `"*.md"`
  literals within `generate`/`generate_for_crate` calls).
- **Coverage gate.** New generator branches and the guard must carry tests to
  stay above the 85% threshold; the unit tests in Phases 1–2 are scoped to
  cover them.

## Out of scope

- Migrating existing single-crate specs to the slug form. They keep working
  unchanged; migration is incremental and follows the reconcile flow when a
  spec actually needs to span crates.
