# Plan: Tc2_BuiltIns Implicit Compatibility Library

## Goal

Ship a new bundled compatibility library, **`Tc2_BuiltIns`**, that models the
surface TwinCAT provides to every PLC project *without any library reference* —
the built-in conversion operators. TwinCAT treats `BOOL_TO_STRING` as a compiler
operator, implicitly known in every project; no `.plcproj` ever references a
library to get it. That is exactly why the
[bindings plan](2026-08-08-compatibility-library-bindings.md) left
`BOOL_TO_STRING`'s placement open ("no vendor library is its true home"): none
of the referenced libraries is where it lives. Its true home is a library that
mirrors the implicit surface.

Two deliverables:

1. **Implicit activation** — a third activation channel: a bundled library whose
   manifest is marked `implicit = true` activates automatically whenever a
   TwinCAT project (`.plcproj`) is discovered, mirroring TwinCAT's own behavior
   where these names are in scope in every project. The `.plcproj` itself is the
   explicit signal; nothing is ever inferred from POU source content, so the
   *never sniff, never guess* rule is preserved.
2. **The `Tc2_BuiltIns` library** — first content: `BOOL_TO_STRING(IN: BOOL):
   STRING` with a math/spec-dictated ST body (`'TRUE'` / `'FALSE'`). Verified
   empirically before this plan: a STRING-returning user function with this name
   compiles and runs correctly in today's codegen, so no bindings machinery is
   needed.

This resolves the `BOOL_TO_STRING` portion of
[#1246](https://github.com/ironplc/ironplc/pull/1246) the way review asked
(vendor surface externalized as a library, per
[ADR-0042](../adrs/0042-library-functions-over-compiler-intrinsics.md)), and
closes the placement question the bindings plan deferred. Code written in
IronPLC against `Tc2_BuiltIns` compiles unchanged in TwinCAT because TwinCAT
always provides these names; a TwinCAT project using them compiles unchanged in
IronPLC because discovery lights the library up automatically.

## Non-goals

- `LREAL_TO_FMTSTR` and `ADR` (the rest of #1246): `LREAL_TO_FMTSTR` is
  `Tc2_Utilities` declare-only work in the bindings plan; `ADR` is a
  function-like operator on the dialect axis (ADR-0042 rule 4).
- `LTRUNC`/`LMOD`/`MODABS` (#1217/#1218): `Tc2_Math`, blocked on bindings.
- The wider `*_TO_*` conversion family. Only `BOOL_TO_STRING` is known-missing
  from the compiler-seeded standard surface; others are added here later only
  when a corpus shows a gap.
- Implicit activation for non-TwinCAT project formats. `implicit = true` today
  means "activated on TwinCAT `.plcproj` discovery" because that is the only
  vendor project discovery IronPLC has; when a second vendor project format
  arrives, the field grows a qualifier.
- Playground changes: the playground has no project context, so implicit
  activation does not apply there; explicit `?libraries=Tc2_BuiltIns` already
  works via the generic mechanism.

## Design doc reference

[compatibility-libraries.md](../design/compatibility-libraries.md) — new
**REQ-CL-sources-008** (implicit activation channel), amended *Activation
channels* and REQ-CL-sources-005 wording.
[compatibility-library-format.md](../design/compatibility-library-format.md) —
new **REQ-LF-sources-008** (optional `implicit` manifest field). LF-005..007
stay reserved for the bindings plan. Owning crate slug: `sources`.

## Architecture

**The manifest carries the flag** (libraries are data, not code — no hard-coded
name list in the loader):

```toml
name = "Tc2_BuiltIns"
vendor = "Beckhoff Automation GmbH"
default_version = "1.0.0"
implicit = true
references = [ ... ]
```

`implicit` is optional, defaults to `false`; a non-boolean value is a P6010
manifest error like any other shape violation.

**Discovery injects the reference.** `detect_twincat`
(`sources/src/discovery/mod.rs`) already merges `<PlaceholderReference>` /
`<LibraryReference>` entries across sub-projects with name dedup. After that
merge, it appends a synthetic `LibraryReference` for each bundled implicit
library not already referenced (`version: None`, `namespace: None`,
`declared_in` = the first `.plcproj`'s `FileId`). Everything downstream —
`resolve_references`, activation in `SourceProject::discover_and_add` and
`cli::enumerate_files`, dedup against explicit `--library` — is untouched, and
both the CLI and LSP paths get the behavior from the single discovery hook. The
registry lookup is a small helper (`append_implicit_references(registry, ...)`)
taking `&LibraryRegistry` so unit tests can drive it with a temp root;
`detect_twincat` calls it with `LibraryRegistry::bundled()`.

**Registry reads manifests only.** New
`LibraryRegistry::implicit_library_names()` iterates the bundled library
directories and parses each `library.toml` (no `.st` parsing); malformed
manifests are skipped there — they are diagnosed on load by the existing path,
and the bundled-provenance conformance test keeps bundled manifests valid.

**The library itself** is ordinary data under
`compiler/sources/resources/libs/Tc2_BuiltIns/`:

```
FUNCTION BOOL_TO_STRING : STRING
VAR_INPUT
    IN : BOOL;
END_VAR
IF IN THEN
    BOOL_TO_STRING := 'TRUE';
ELSE
    BOOL_TO_STRING := 'FALSE';
END_IF;
END_FUNCTION
```

No packaging changes: every installer copies the whole `libs/` tree, and the
playground's `gen-libs-index.mjs` enumerates directories generically.

## File map

**New**
- `specs/design/library-interfaces/tc2-builtins.md` — clean-room interface spec
  from public Beckhoff InfoSys references, committed as its own non-squashed
  commit *before* implementation (authoring policy).
- `compiler/sources/resources/libs/Tc2_BuiltIns/library.toml`
- `compiler/sources/resources/libs/Tc2_BuiltIns/1.0.0/Tc2_BuiltIns.st`
- `compiler/ironplc-cli/resources/test/twincat_builtins_solution/…` — e2e
  fixture: a solution whose `.plcproj` has **no** library references and whose
  POU calls `BOOL_TO_STRING`.

**Modified**
- `specs/design/compatibility-libraries.md` — implicit channel +
  **REQ-CL-sources-008**; REQ-CL-sources-005 reworded to name the three
  explicit channels.
- `specs/design/compatibility-library-format.md` — `implicit` field row +
  **REQ-LF-sources-008**.
- `compiler/sources/src/libraries/manifest.rs` — parse optional `implicit`.
- `compiler/sources/src/libraries/mod.rs` — `implicit_library_names()`.
- `compiler/sources/src/discovery/mod.rs` — synthetic-reference injection.
- `compiler/sources/src/spec_conformance.rs` — `#[spec_test]`s for the two new
  markers.
- `compiler/project/src/project.rs` — semantic-level test (implicit activation
  end to end).
- `compiler/ironplc-cli/tests/cli.rs` — binary-level e2e tests.

## Testing strategy

- Manifest: `implicit = true`, absent (defaults false), and non-boolean →
  P6010. (`manifest.rs` unit tests.)
- Registry: `implicit_library_names` returns `Tc2_BuiltIns` for the bundled
  root and respects a temp root without the flag.
- Discovery: a `.plcproj` with no references yields the implicit reference; a
  `.plcproj` already referencing an implicit library does not duplicate it.
- Semantic (project crate): program calling `BOOL_TO_STRING` in a `.plcproj`
  project analyzes clean with no reference; same program as bare source fails
  (dormant by default); `--library Tc2_BuiltIns`-style explicit activation
  works (existing generic path).
- Spec conformance: `sources_spec_req_cl_008_*` and `sources_spec_req_lf_008_*`
  per the reconcile-spec convention; existing REQ-CL-sources-005 test still
  passes (bare source activates nothing).
- CLI e2e: `check` on the new fixture passes with no `--library`; `compile` +
  VM run asserts `BOOL_TO_STRING(TRUE) = 'TRUE'` and
  `BOOL_TO_STRING(FALSE) = 'FALSE'` via string equality (verified expressible
  today).
- Existing `twincat_tc2_system_solution` tests keep passing (they now also
  activate `Tc2_BuiltIns`; its declarations must be inert for code that does
  not call them).
- `cd compiler && just` green.

## Tasks

- [ ] Clean-room interface spec (own commit, before implementation)
- [ ] Design-doc amendments (implicit channel, manifest field, REQ markers)
- [ ] Manifest `implicit` parse + registry `implicit_library_names()`
- [ ] Discovery synthetic-reference injection
- [ ] `Tc2_BuiltIns` package (manifest + ST)
- [ ] Unit, spec-conformance, semantic, and CLI e2e tests
- [ ] `cd compiler && just` green

## Implementation notes

- Empirically verified before planning: STRING-returning user functions
  compile and run (LEN/equality assertions pass in the VM), and the name
  `BOOL_TO_STRING` does not collide with the compiler-seeded conversion
  family — so the ST-body route needs zero compiler changes.
- User shadowing (REQ-CL-analyzer-004) applies unchanged: a user-defined
  `BOOL_TO_STRING` wins over the library's.
- `--dump-vars` renders STRING contents as `0`; e2e assertions therefore go
  through `LEN`/string-equality into BOOL/INT variables.
- Public documentation for the *name* `Tc2_BuiltIns` is thin — the implicit
  surface is documented as operators (`tc3_plc_intro`), not as a library
  manual. The manifest `references` cite the operator documentation the
  behavior was authored from; the library name is IronPLC's handle for that
  implicit surface, chosen by the project owner.
