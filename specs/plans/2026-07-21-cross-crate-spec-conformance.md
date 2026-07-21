# Plan: Cross-Crate Spec Conformance

## Goal

Let a single design spec link conformance tests that live in more than one
crate, while preserving the existing validity (removing a requirement breaks
compilation) and completeness (every requirement has a test) guarantees.
Ownership becomes explicit and **mandatory** via a crate slug in every
requirement ID. The single-crate form is removed entirely — there is no
backward-compatible fallback — so every enforced requirement is migrated.

## Design doc reference

[specs/design/cross-crate-spec-conformance.md](../design/cross-crate-spec-conformance.md)
— grammar, generator behavior, orphan guard, migration. Supersedes the
single-crate model in
[specs/design/spec-conformance-testing.md](../design/spec-conformance-testing.md).

## Architecture

1. **ID grammar** requires a crate slug: `REQ-<AREA>-<crate-slug>-<NNN>`.
   Parsed by anchoring on the ends — leading `[A-Z0-9]+` is the area, trailing
   `[0-9]+` is the number, the lowercase middle is the crate slug. An empty
   middle is an error. Ident form is unchanged
   (`REQ-VC-vm-cli-001` → `REQ_VC_vm_cli_001`).

2. **`spec_requirements_gen` is crate-aware and strict.** It derives the
   current crate's slug from `CARGO_PKG_NAME` (strip `ironplc-`, which equals
   the crate directory name). `owner(req)` is the requirement's slug, with **no
   fallback**. If a listed doc contains a `**REQ-…**` marker with no slug,
   `generate()` panics with an actionable message — this is the mechanism that
   removes the single-crate form. A requirement enters this crate's
   `UNTESTED`/`ALL` only when `owner(req) == my_slug`. `build.rs` signatures are
   unchanged.

3. **Workspace orphan guard.** A single workspace-level test parses every
   enforced `specs/design/*.md` for requirement IDs (recovering `(doc, slug)`)
   and every `compiler/*/build.rs` for the `.md` filenames it lists (recovering
   the slug from the `build.rs` directory name), then asserts every
   `(slug, doc)` used by a requirement is claimed by a listing crate and that
   no enforced requirement lacks a slug. No separate manifest — both sides come
   from files in the tree.

## Migration surface (enforced set)

| Doc(s)                                                        | Crate     | Areas             | ~refs |
|--------------------------------------------------------------|-----------|-------------------|-------|
| `bytecode-container-format.md`, `bytecode-instruction-set.md`| container | `CF`, `IS`        | ~12   |
| `enumeration-codegen.md`                                     | codegen   | `EN`              | ~33   |
| `mcp-server.md`                                              | mcp       | `STL`,`TOL`,`ARC` | ~90   |
| `vm-cli.md`                                                  | vm-cli    | `VC`              | ~31   |

~135 requirement markers and ~166 `#[spec_test]` references. Each doc is
single-crate today, so its slug is uniform throughout the doc — a mechanical
per-doc rename. Unwired docs (`partial-access-bit-syntax.md`,
`subrange-codegen.md`, `time-literals.md`, `mcp-server-distribution.md`) are
**out of scope**; they adopt the slug form when first wired.

## File map

**Modified**
- `compiler/spec_requirements_gen/src/lib.rs` — slug parsing, crate-slug
  derivation, `owner()` with no fallback, panic-on-unslugged, unit tests.
- `specs/design/spec-conformance-testing.md` — mark the single-crate model
  superseded; cross-reference the mandatory grammar and the cross-crate doc.
- `specs/steering/development-standards.md` — update the "Design Requirement"
  section to require the crate slug.
- `.claude/commands/reconcile-spec.md` — update the ID format and the
  find-highest-ID grep to the slugged form.
- **Enforced spec docs** (rename markers): `bytecode-container-format.md`,
  `bytecode-instruction-set.md`, `enumeration-codegen.md`, `mcp-server.md`,
  `vm-cli.md`.
- **Enforced test sources** (rename `#[spec_test]` refs): container, codegen,
  mcp, vm-cli conformance modules (`spec_conformance.rs`, `tests/cli.rs`, and
  any other `src/` files carrying `#[spec_test]`).

**Created**
- `specs/adrs/0037-mandatory-crate-slug-in-requirement-ids.md` — ADR recording
  the decision to make the crate slug mandatory and remove the single-crate
  model (drafted; committed with this plan).
- Workspace orphan-guard test in the `ironplc-test` crate
  (`compiler/test/src/…`), reading the repo via `CARGO_MANIFEST_DIR`
  (`../../specs/design`, `../*/build.rs`).
- Proof-of-concept: split one enforced doc across a second crate to exercise
  the cross-crate path end-to-end (see Phase 4).

**Unchanged (verified)**
- `compiler/spec_test_macro/src/lib.rs` — the ident form already handles the
  extra segment.
- `build.rs` call sites — signatures unchanged; slug is auto-derived.

## Verification strategy (TDD, red-first)

The conformance system exists to **fail loudly when spec and code disagree**,
so every guarantee is proven by deliberately creating its failure, observing
red, then fixing to green. The headline proof that the new work does anything
is that a cross-crate scenario is **red on today's generator and green only
after** the crate-aware change — that single red→green is the load-bearing
evidence.

| Guarantee | Red (deliberately broken) | Green (after fix) |
|-----------|---------------------------|-------------------|
| Cross-crate capability | 2 crates list one demo doc, each owns a slugged req + test; run on the current generator → the other crate's req shows as `UNTESTED` | crate-aware `owner()` filter → both meta-tests pass |
| Mandatory slug | leave an unslugged `**REQ-…**` in a listed doc | generator panics naming the marker → slug it |
| Validity | `#[spec_test(REQ_…_missing)]` for a non-existent req | compile error until the req exists |
| Completeness | add a slugged req with no test | owner's `UNTESTED` non-empty → meta-test fails |
| Orphan guard | slug a req for a crate that doesn't list the doc | guard fails naming (req, slug, doc) |

**Little old code is removed.** The single-crate model is the *absence* of
slug-awareness, not a deletable subsystem. The work is tightening `generate()`
(additive), renaming IDs (a rename, not a deletion), and adding the guard.
`spec_test_macro` is untouched and no module becomes dead. Removal of the
single-crate model is enforced behaviorally by the build-time panic.

**Ordering foot-gun.** Once `owner()` has no fallback, a still-unslugged
requirement in a listed doc loses its owner and would be *silently untested*.
The panic-on-unslugged therefore ships in the **same** change as the owner
filter, and the capability proof (Phase 0) uses an **isolated, fully-slugged
demo doc** so it is not entangled with the half-migrated real docs.

## Tasks

### Phase 0 — Red harness (prove the gap before implementing)
- [ ] Add a throwaway demo design doc under `specs/design/` (e.g.
      `cross-crate-demo.md`) containing exactly two slugged requirements owned
      by two different already-wired crates (e.g. `REQ-XD-codegen-001` and
      `REQ-XD-vm-001`), plus minimal `build.rs`/`spec_conformance` wiring in
      each so both crates list it and each has one `#[spec_test]`.
- [ ] Run the build on the **current** generator and confirm it is **RED**: a
      meta-test reports the other crate's requirement as `UNTESTED`. Capture the
      exact failure. This is the evidence the old system cannot express
      cross-crate.
- [ ] Keep this harness until Phase 1 turns it green; then either delete it or
      graft it into the real PoC (Phase 4). Do not leave the demo doc in the
      tree at merge.

### Phase 1 — Generator (mandatory slug, strict) — turns Phase 0 GREEN
- [ ] Add slug parsing to `spec_requirements_gen`: split a raw `REQ-…` ID into
      `(area, slug, number)` by anchoring on the ends. Unit tests: hyphenated
      slug (`REQ-VC-vm-cli-001`), single-word slug, and an unslugged ID
      (`REQ-CF-001`) which must be reported as invalid.
- [ ] Derive the current crate slug from `CARGO_PKG_NAME` (strip `ironplc-`).
- [ ] Set `owner(req)` = the requirement's slug with no fallback; filter
      `UNTESTED` and `ALL` to `owner(req) == my_slug`. Panic in `generate()`
      when a listed doc contains an unslugged `**REQ-…**` marker, naming the
      offending marker and doc.
- [ ] Unit tests: req owned by another crate is excluded from `UNTESTED`; a
      slugged req untested in its owner appears in that owner's `UNTESTED`; a
      doc with an unslugged marker triggers the panic.
- [ ] `cargo test -p ironplc-spec-requirements-gen` passes.
- [ ] Confirm the Phase 0 red harness now passes (RED→GREEN) — the two-crate
      demo doc builds and both meta-tests are green. This is the proof the
      crate-aware change is load-bearing.

### Phase 2 — Migrate the enforced set (one doc/crate per commit)
For each of container (CF, IS), codegen (EN), mcp (STL, TOL, ARC), vm-cli (VC):
- [ ] Rename every `**REQ-XX-NNN**` in the doc(s) to `**REQ-XX-<crate>-NNN**`
      (including table-form `| **REQ-XX-NNN** |` rows).
- [ ] Rename every `#[spec_test(REQ_XX_NNN)]` reference (and any prose/comment
      references to the old ID) to the slugged form, across `src/` and `tests/`.
- [ ] Build the crate; confirm compilation (validity check) and the
      `all_spec_requirements_have_tests` meta-test both pass. A missed
      reference fails to compile; a missed marker leaves `UNTESTED` non-empty.
- [ ] Commit per crate so each rename is independently reviewable.

### Phase 3 — Workspace orphan guard
- [ ] Add the guard test in `ironplc-test`. Parse enforced `specs/design/*.md`
      for requirement IDs → `(doc, slug)`; parse `compiler/*/build.rs` for
      listed `.md` filenames + directory-name slug → `(slug, doc)` listings.
- [ ] Assert: no enforced requirement is unslugged; every `(slug, doc)` a
      requirement uses is claimed by a listing crate; every doc a requirement's
      slug names is actually listed by that crate. Failures name the
      requirement, slug, and doc.
- [ ] Add fixture-based unit tests for the guard's markdown and `build.rs`
      parsers so the guard is covered without depending on live repo state.

### Phase 4 — Proof of concept (end-to-end cross-crate)
- [ ] Pick one enforced doc that genuinely spans layers and move a small,
      naturally-runtime subset of its requirements to a second crate.
      Recommended: `enumeration-codegen.md`, splitting a runtime-observable
      enumeration behavior from `codegen` to `vm` (re-slugging those
      requirements `-vm-` and adding the `vm` `build.rs`/`spec_requirements`/
      `spec_conformance` wiring, mirroring the container reference).
- [ ] Add the moved `#[spec_test(REQ_EN_vm_…)]` in `vm`; confirm both codegen
      and vm meta-tests pass with the same doc listed by both crates.
- [ ] Verify the negative paths by hand: (a) removing a requirement breaks its
      `#[spec_test]` compilation; (b) omitting a slugged test leaves that
      owner's `UNTESTED` non-empty; (c) an unslugged marker panics the build;
      (d) a slug naming an unlisted crate trips the orphan guard.

### Phase 5 — Docs and CI
- [ ] Update `spec-conformance-testing.md`, `development-standards.md`, and
      `reconcile-spec.md` per the file map.
- [ ] Land ADR-0037 (drafted with this plan); flip its `status:` to `accepted`
      once the change is implemented and merged.
- [ ] Run `cd compiler && just` (compile, coverage ≥ 85%, clippy, fmt) — all
      green — before opening any PR.

## Risks and mitigations

- **Large mechanical rename.** ~166 references across four crates. Mitigated by
  migrating one doc/crate per commit, and by the build itself catching any
  missed reference (compile error) or missed marker (meta-test failure).
- **ID churn on future refactors.** Moving a test between crates changes its
  slug and every reference. Mitigated by the compile-time validity check; the
  fix is a find-and-replace.
- **`build.rs` parsing brittleness in the guard.** The guard reads `.md`
  filenames from `build.rs` source text. Mitigated by fixture-based unit tests
  and matching only quoted `"*.md"` literals within `generate` calls.
- **Coverage gate.** New generator branches and the guard must carry tests to
  stay above 85%; the unit tests in Phases 1–3 are scoped to cover them.

## Out of scope

- Migrating unwired docs (`partial-access-bit-syntax.md`, `subrange-codegen.md`,
  `time-literals.md`, `mcp-server-distribution.md`). They adopt the slug form
  when first wired into conformance.
