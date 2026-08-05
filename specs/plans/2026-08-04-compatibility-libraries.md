# Plan: Compatibility Libraries

## Goal

Deliver the first increment of the [Compatibility Libraries](../design/compatibility-libraries.md)
design: IronPLC ships dormant, named bundles of vendor declarations that are
activated out of band, and injected into the compilation unit so their symbols
resolve under their exact vendor names. The two capabilities that must land
early are:

1. **Reading the referenced-library list from a `.plcproj` project file** and
   activating the matching bundled libraries.
2. **The `Tc2_System` compatibility library** — the TwinCAT library whose global
   constants include `PI` — so `PI` resolves as a compile-time-folded constant
   with no new keyword and no per-symbol flag. `Tc2_System` is the exact name a
   `.plcproj` references, so activation matches by name.

This closes the long-standing `PI` gap through the general library mechanism
rather than the stalled `--allow-math-constants` approach, and lays the
substrate for further libraries and beyond. The key outcome is **`PI` for
TwinCAT**.

## Non-goals

- Collision / precedence resolution between simultaneously-active libraries — a
  deferred *Future Goal* in the design.
- Cross-library / cross-vendor mixing as a supported configuration.
- The general mechanism for *detecting* an unsupported library/flag
  configuration. This plan only holds the invariant that declare-only calls fail
  to compile.
- Qualified-access-as-a-requirement. The parser must not *add* qualifiers; making
  a library *require* qualified access is deferred (design *Future Goals*).
- Full runtime bodies for native vendor functions (e.g. `Tc2_Math` numeric
  fidelity). Signature/declare-only support lands; intrinsics come later.

## Design doc reference

Two design docs:
[compatibility-libraries.md](../design/compatibility-libraries.md) (behavior,
`REQ-CL-*`) and
[compatibility-library-format.md](../design/compatibility-library-format.md)
(on-disk format + installation, `REQ-LF-*`). Every marker in both is delivered by
a phase below and wired to a `#[spec_test]`. Owning crate slugs: `sources`,
`analyzer`, `plc2plc`, `playground`.

## Architecture

A **compatibility library** is on-disk data: a manifest (`name`, `vendor`,
`default_version`, `references`) plus per-version subdirectories of IEC 61131-3
declarations, in the package format specified by
[compatibility-library-format.md](../design/compatibility-library-format.md).
Libraries are installed on disk and read at runtime (not embedded in the
compiler). A loader in the `sources` crate parses a library into an
`ironplc_dsl::common::Library`. An **activated-library set**
(names) is carried on the project and threaded into analysis; the analyzer
prepends each activated library's `Library` to the existing
`analyze(sources: &[&Library])` merge (`resolve_types` already does
`library.extend()`), so activated declarations resolve exactly like user source.
Because the merge and conditional environment seeding already exist, the new
surface is: the on-disk format + loader, and the activation set + its two input
channels (project file, CLI/playground). The first increment ships only
fully-defined ST — bindings, intrinsics, and declare-only are deferred.

Activation order in the merge is **base stdlib → activated libraries → user
source**, so a user declaration shadows a library declaration of the same name
(REQ-CL-analyzer-004). Inter-library collisions are out of scope (design
*Future Goals*); the first increment assumes activated libraries do not collide.

## File map

**New — bundled library + loader (`sources`)**
- `compiler/sources/resources/libs/Tc2_System/library.toml` — manifest (`name`, `vendor`, `default_version`, `references`).
- `compiler/sources/resources/libs/Tc2_System/1.0.0/Tc2_System.st` — `VAR_GLOBAL CONSTANT PI : LREAL := 3.1415926535897932384626433832795;` (version subdirectory).
- `compiler/sources/src/libraries/mod.rs` — registry + loader (name → `Library`), reading installed on-disk libraries at runtime.
- `compiler/sources/src/libraries/manifest.rs` — manifest parse (identity + `default_version` + references).

**New — spec-conformance wiring**
- `compiler/sources/build.rs` calls `generate(&["compatibility-libraries.md", "compatibility-library-format.md"])` (it owns `REQ-CL-*` and `REQ-LF-*`); `compiler/analyzer/build.rs`, `compiler/plc2plc/build.rs`, `compiler/playground/build.rs` each call `generate(&["compatibility-libraries.md"])`
  (create where absent; reference `compiler/container/build.rs`).
- `compiler/{sources,analyzer,plc2plc}/src/spec_conformance.rs` and the
  `playground` equivalent — `#[spec_test]` tests + the
  `all_spec_requirements_have_tests` meta-test.

**New — test fixtures**
- A `.plcproj` referencing the library, plus an `.st`/POU using `PI`.

**New — provenance & policy**
- `specs/design/compatibility-library-format.md` — the on-disk format + installation design (already added).
- `specs/steering/compatibility-library-authoring.md` (+ `.kiro/steering/` pointer) — the authoring policy (already added).
- Manifest `references` field (the public references authored from); the clean-room spec is a separate non-squashed git commit, not a file in the package.
- Provenance conformance test in `compiler/sources` (walks manifests; asserts `references` present).

**Modified**
- `compiler/sources/src/discovery/mod.rs` — twincat detector reads the
  `.plcproj` library references (in addition to `<Compile Include>`).
- `compiler/sources/src/project.rs`, `compiler/project/src/project.rs` — carry
  the activated-library set; include activated `Library`s in the analyze slice.
- `compiler/analyzer/src/stages.rs` — inject activated libraries at the base of
  the merge; keep precedence base → library → user.
- `compiler/ironplc-cli/bin/main.rs` — `--library <name>` (repeatable).
- `compiler/playground/src/lib.rs` + `playground/` frontend — fetch served
  library files and activate.

## Testing strategy

- Each `REQ-CL-*` marker gets a `#[spec_test(REQ_CL_<slug>_NNN)]` test named
  `{area}_spec_req_cl_<slug>_NNN_<brief>`, per the
  [reconcile-spec](../../.claude/commands/reconcile-spec.md) convention.
- **Wiring a crate's `build.rs` to list the design doc enforces *all* of that
  crate's markers at once** (the completeness meta-test). So in the phase that
  first wires a crate, add `#[spec_test]` for every marker that crate owns, using
  `#[ignore]` for markers whose implementation lands in a later phase. Un-ignore
  them as each phase completes.
- Prefer structural asserts: `PI` folds to a known `LREAL`; `plc2plc` output of a
  library-using program is byte-identical to its input.
- End-to-end: parse → activate → analyze → (Phase 1) confirm `PI` resolves and
  folds; (Phase 2) same with activation coming only from the `.plcproj`.
- **Provenance conformance test (Phase 4):** a `sources` test walks every bundled
  library manifest and asserts it is well-formed and records a non-empty
  `references` list. This enforces the *machine-checkable* half
  of the
  [authoring policy](../steering/compatibility-library-authoring.md); the
  human-only half (no forbidden input used, clearance performed) stays a reviewer
  responsibility.
- Run `cd compiler && just` (compile, coverage ≥85%, clippy, fmt) before any PR.

## Tasks

### Phase 1 — The `Tc2_System` library (PI), package format, explicit activation *(early)*
- [x] Write plan and add `REQ-CL-*` / `REQ-LF-*` markers to the design docs
- [x] Implement the on-disk package format per [compatibility-library-format.md](../design/compatibility-library-format.md): package layout (version subdirectories), manifest schema, and name resolution; libraries read from disk at runtime — **REQ-LF-sources-001**, **002**, **004**
- [x] Manifest identity validated on load (`name`, `vendor`, `default_version`) — **REQ-CL-sources-002**
- [x] Add the bundled `Tc2_System` library defining `PI`
- [x] Implement the library loader + registry (name → `Library`) in `sources`
- [x] Carry an activated-library set on the project; add `--library <name>` to the CLI — **REQ-CL-sources-006**
- [x] Inject activated libraries into the analyze merge, base → library → user — **REQ-CL-analyzer-001**
- [x] Resolve library symbols flat under exact vendor names — **REQ-CL-analyzer-002**
- [x] `PI` resolves as a constant and folds in a `VAR` initializer — **REQ-CL-analyzer-003**
- [x] A user declaration shadows a library declaration of the same name — **REQ-CL-analyzer-004**
- [x] Activated set derives only from explicit activation; never inferred from source — **REQ-CL-sources-005**
- [x] Selecting a dialect does not activate any library (dialect ≠ vendor) — **REQ-CL-analyzer-006**
- [x] Wire `sources` `build.rs` (both design docs) + `analyzer` `build.rs`; add `spec_conformance` + meta-tests; `#[spec_test]` the markers above; `#[ignore]` sources-001/003/004/007 for now
- [x] `cd compiler && just` green

> **Note on the guard-forced scope.** The workspace orphan guard
> (`compiler/test/tests/spec_conformance_guard.rs`) requires *every* slug in an
> enforced doc to be claimed by a listing crate. Listing
> `compatibility-libraries.md` therefore also required wiring the `plc2plc` and
> `playground` `build.rs` in Phase 1, with their single markers
> (`REQ-CL-plc2plc-001`, `REQ-CL-playground-001`) as `#[ignore]`d `#[spec_test]`
> placeholders (real behavior lands in Phase 3). Injection is done in the
> `project` crate's `run_semantic_analysis` (prepending the loaded libraries to
> the analyze slice) rather than by changing the `analyze` signature — merge
> collection precedes folding, so slice order (library before user) is
> sufficient for precedence, and the analyzer owns dormancy via its
> `#[spec_test]`s.

### Phase 2 — Read the library list from `.plcproj` *(early)*
- [x] In the twincat detector, parse `<PlaceholderReference>` and `<LibraryReference>` elements inside `<ItemGroup>` (MSBuild `xmlns`), extracting `Include`, `Namespace`, and (for placeholders) `DefaultResolution` — **REQ-CL-sources-001**
- [x] Skip references marked `<SystemLibrary>true</SystemLibrary>` for now
- [x] Auto-activate matching bundled libraries (no CLI flag needed)
- [x] Resolve reference → bundled library by strict, case-sensitive **name** match; treat a `*` version as "any bundled version" — **REQ-CL-sources-003**
- [x] Diagnose a referenced-but-unshipped library, naming it — **REQ-CL-sources-004**
- [x] Un-ignore sources-001/003/004; add the `.plcproj`-driven end-to-end `PI` test using a fixture that references a bundled library
- [x] `cd compiler && just` green

### Phase 3 — Round-trip fidelity + playground
- [ ] `plc2plc` emits user source unchanged; injected library declarations are never rendered — **REQ-CL-plc2plc-001**
- [ ] Wire `plc2plc` `build.rs` + `spec_conformance`; add the round-trip test
- [ ] Playground: serve library files as plain text, load as sources, activate — **REQ-CL-playground-001**
- [ ] Wire `playground` `build.rs` + spec test; update `playground/` frontend to fetch library files
- [ ] `cd compiler && just` green

### Phase 4 — Provenance policy enforcement
- [ ] Conformance test in `sources` walks every bundled manifest and asserts it records a non-empty `references` list (a factual record, no legal judgment) — **REQ-CL-sources-007**
- [ ] Un-ignore sources-007; confirm the [authoring policy](../steering/compatibility-library-authoring.md) — the reviewer checklist and the non-squashed clean-room-spec-commit rule — is referenced from contribution docs
- [ ] `cd compiler && just` green

### Deferred / out of scope (see the design's *Non-Goals* and *Future Goals*)
- **Bindings** (per-version manifest table), non-ST implementations (VM
  intrinsics), the **declare-only** state, and the fail-if-unimplemented compile
  error. The first increment ships only fully-defined ST.
- Collision / precedence resolution; cross-vendor mixing.
- Accepting source-written namespace qualifiers (flat names only for now).
- **Tier C (vendored third-party, e.g. OSCAT)** — not shipped through this
  mechanism; it is a *separate distribution mechanism*. Tier A (math) and Tier B
  (clean-room shims) do not depend on it.
- Dialect-driven default activation — **decided against** (dialect ≠ vendor).

## Implementation notes

- `REQ-CL-*` markers are inert until a crate's `build.rs` lists the design doc;
  the workspace orphan guard (`compiler/test/tests/spec_conformance_guard.rs`)
  only checks docs that are listed. Wiring therefore happens per phase, alongside
  the tests, so CI stays green throughout.
- Keep new analyzer modules under the 1000-line limit; the loader belongs in
  `sources` (it produces a `Library`), not the analyzer.
- The activated-library merge reuses `resolve_types`'s existing `library.extend`;
  no new merge machinery is needed — only the ordering (base → library → user).
- The `.plcproj` library-reference shape is grounded against real projects — see
  the design doc's *Appendix: `.plcproj` library-reference shapes*. References
  live in `<ItemGroup>` as `<PlaceholderReference>` (version usually `*`) or
  `<LibraryReference>` (pinned `Name,Version,Vendor`); the `<Namespace>` child
  supplies the qualifier the source may write.
