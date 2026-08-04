# Plan: Compatibility Libraries

## Goal

Deliver the first increment of the [Compatibility Libraries](../design/compatibility-libraries.md)
design: IronPLC ships dormant, named bundles of vendor declarations that are
activated out of band, and injected into the compilation unit so their symbols
resolve under their exact vendor names. The two capabilities that must land
early are:

1. **Reading the referenced-library list from a `.plcproj` project file** and
   activating the matching bundled libraries.
2. **A bundled library that defines `PI`** (and `e`), so `PI` resolves as a
   compile-time-folded constant with no new keyword and no per-symbol flag.

This closes the long-standing `PI` gap through the general library mechanism
rather than the stalled `--allow-math-constants` approach, and lays the
substrate for `Tc2_Math` and OSCAT.

## Non-goals

- Collision / precedence resolution between simultaneously-active libraries — a
  deferred *Future Goal* in the design.
- Cross-library / cross-vendor mixing as a supported configuration.
- The general mechanism for *detecting* an unsupported library/flag
  configuration (design Open Question 3). This plan only holds the invariant
  that declare-only calls fail to compile.
- Qualified-access-as-a-requirement (design Open Question 1). The parser must
  not *add* qualifiers; making a library *require* qualified access is out of
  scope here.
- Full runtime bodies for native vendor functions (e.g. `Tc2_Math` numeric
  fidelity). Signature/declare-only support lands; intrinsics come later.

## Design doc reference

[specs/design/compatibility-libraries.md](../design/compatibility-libraries.md).
Every `REQ-CL-*` marker in that doc is delivered by a phase below and wired to a
`#[spec_test]`. Areas: `sources`, `analyzer`, `plc2plc`, `playground`.

## Architecture

A **compatibility library** is bundled data: a manifest (name, vendor, version,
target) plus IEC 61131-3 declarations. A loader in the `sources` crate parses a
library into an `ironplc_dsl::common::Library`. An **activated-library set**
(names) is carried on the project and threaded into analysis; the analyzer
prepends each activated library's `Library` to the existing
`analyze(sources: &[&Library])` merge (`resolve_types` already does
`library.extend()`), so activated declarations resolve exactly like user source.
Because the merge and conditional environment seeding already exist, the new
surface is: the on-disk format + loader, the activation set + its two input
channels (project file, CLI/playground), and a rule that rejects calls to
declare-only POUs.

Activation order in the merge is **base stdlib → activated libraries → user
source**, so a user declaration shadows a library declaration of the same name
(REQ-CL-analyzer-004). Inter-library collisions are out of scope (design
*Future Goals*); the first increment assumes activated libraries do not collide.

## File map

**New — bundled library + loader (`sources`)**
- `compiler/sources/resources/compat-libraries/math/library.toml` — manifest.
- `compiler/sources/resources/compat-libraries/math/math.st` — `VAR_GLOBAL CONSTANT PI, e`.
- `compiler/sources/src/libraries/mod.rs` — registry + loader (name → `Library`).
- `compiler/sources/src/libraries/manifest.rs` — manifest parse + provenance.

**New — spec-conformance wiring**
- `compiler/sources/build.rs`, `compiler/analyzer/build.rs`,
  `compiler/plc2plc/build.rs`, `compiler/playground/build.rs` — each calls
  `ironplc_spec_requirements_gen::generate(&["compatibility-libraries.md"])`
  (create where absent; reference `compiler/container/build.rs`).
- `compiler/{sources,analyzer,plc2plc}/src/spec_conformance.rs` and the
  `playground` equivalent — `#[spec_test]` tests + the
  `all_spec_requirements_have_tests` meta-test.

**New — declare-only safety**
- `compiler/analyzer/src/rule_call_not_declare_only.rs` — reject calls to a
  declare-only POU.
- `compiler/problems/resources/problem-codes.csv` — new `P####` for the above.
- `docs/compiler/problems/P####.rst` — its documentation.

**New — test fixtures**
- A `.plcproj` referencing the library, plus an `.st`/POU using `PI`.

**New — provenance & policy**
- `specs/steering/compatibility-library-authoring.md` (+ `.kiro/steering/` pointer) — the authoring policy (already added).
- `compiler/sources/resources/compat-libraries/<name>/library.toml` — `[provenance]` fields.
- Provenance conformance test in `compiler/sources` (walks manifests).
- (Tier C is *refused* by this mechanism, not quarantined — see the design's *Licensing…*.)

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
- Prefer structural asserts: `PI` folds to a known `LREAL`; a declare-only call
  produces the expected `P####`; `plc2plc` output of a library-using program is
  byte-identical to its input.
- End-to-end: parse → activate → analyze → (Phase 1) confirm `PI` resolves and
  folds; (Phase 2) same with activation coming only from the `.plcproj`.
- **Provenance conformance test (Phase 5):** a `sources` test walks every bundled
  library manifest and asserts it is well-formed, its `derivation`/`license` are
  from the allowed sets, and any Tier C content (a `vendored` derivation or
  non-permissive license) is refused. This enforces the *machine-checkable* half
  of the
  [authoring policy](../steering/compatibility-library-authoring.md); the
  human-only half (no forbidden input used, clearance performed) stays a reviewer
  responsibility.
- Run `cd compiler && just` (compile, coverage ≥85%, clippy, fmt) before any PR.

## Tasks

### Phase 1 — Library representation, the `math` library (PI), explicit activation *(early)*
- [x] Write plan and add `REQ-CL-*` markers to the design doc
- [ ] Define the on-disk library format: manifest (`library.toml` with identity + `[provenance]` + `[bindings]`) plus `.st` declarations — **REQ-CL-sources-002**
- [ ] Validate POU bindings on load: `st` has a body, `intrinsic:<name>` names an implemented intrinsic, `declare-only` is signature-only — **REQ-CL-sources-009**
- [ ] Add the bundled `math` library defining `PI` and `e`
- [ ] Implement the library loader + registry (name → `Library`) in `sources`
- [ ] Carry an activated-library set on the project; add `--library <name>` to the CLI — **REQ-CL-sources-006**
- [ ] Inject activated libraries into the analyze merge, base → library → user — **REQ-CL-analyzer-001**
- [ ] Resolve library symbols flat under exact vendor names — **REQ-CL-analyzer-002**
- [ ] `PI` resolves as a constant and folds in a `VAR` initializer — **REQ-CL-analyzer-003**
- [ ] A user declaration shadows a library declaration of the same name — **REQ-CL-analyzer-004**
- [ ] Activated set derives only from explicit activation; never inferred from source — **REQ-CL-sources-005**
- [ ] Selecting a dialect does not activate any library (dialect ≠ vendor) — **REQ-CL-analyzer-006**
- [ ] Wire `sources` + `analyzer` `build.rs` to the design doc; add `spec_conformance` + meta-tests; `#[spec_test]` the markers above; `#[ignore]` sources-001/003/004/007/008 and analyzer-005 for now
- [ ] `cd compiler && just` green

### Phase 2 — Read the library list from `.plcproj` *(early)*
- [ ] In the twincat detector, parse `<PlaceholderReference>` and `<LibraryReference>` elements inside `<ItemGroup>` (MSBuild `xmlns`), extracting `Include`, `Namespace`, and (for placeholders) `DefaultResolution` — **REQ-CL-sources-001**
- [ ] Skip references marked `<SystemLibrary>true</SystemLibrary>` for now
- [ ] Auto-activate matching bundled libraries (no CLI flag needed)
- [ ] Resolve reference → bundled library by strict, case-sensitive **name** match; treat a `*` version as "any bundled version" — **REQ-CL-sources-003**
- [ ] Diagnose a referenced-but-unshipped library, naming it — **REQ-CL-sources-004**
- [ ] Un-ignore sources-001/003/004; add the `.plcproj`-driven end-to-end `PI` test using a fixture that references a bundled library
- [ ] `cd compiler && just` green

### Phase 3 — Declare-only bodies + safety
- [ ] Add a `declare-only` library POU (signature only) — e.g. a `Tc2_Math`-style `FLOOR : LREAL` — using the binding kind from Phase 1
- [ ] Analyzer rule: a *call* to a `declare-only` POU is a compile error, naming the library + POU (new `P####`) — **REQ-CL-analyzer-005**
- [ ] Add the problem-code CSV entry + `docs/compiler/problems/P####.rst`; un-ignore analyzer-005
- [ ] `cd compiler && just` green

### Phase 4 — Round-trip fidelity + playground
- [ ] `plc2plc` emits user source unchanged; injected library declarations are never rendered — **REQ-CL-plc2plc-001**
- [ ] Wire `plc2plc` `build.rs` + `spec_conformance`; add the round-trip test
- [ ] Playground: serve library files as plain text, load as sources, activate — **REQ-CL-playground-001**
- [ ] Wire `playground` `build.rs` + spec test; update `playground/` frontend to fetch library files
- [ ] `cd compiler && just` green

### Phase 5 — Provenance & licensing policy enforcement
- [ ] Conformance test in `sources` walks every bundled manifest and asserts provenance is well-formed with `derivation`/`license` from the allowed (permissive / own-authored) sets — **REQ-CL-sources-007**
- [ ] The mechanism refuses Tier C: a `vendored` derivation or a non-permissive license is rejected, not bundled — **REQ-CL-sources-008**
- [ ] Un-ignore sources-007/008; confirm the [authoring policy](../steering/compatibility-library-authoring.md) reviewer checklist is referenced from contribution docs
- [ ] `cd compiler && just` green

### Deferred / out of scope (see the design's *Non-Goals* and *Future Goals*)
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
