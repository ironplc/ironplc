# Plan: Resolve TwinCAT projects via `.sln` → `.tsproj` → `PrjFilePath`

## Problem

`detect_twincat` in `compiler/sources/src/discovery/mod.rs` finds
`.plcproj` files by recursively walking the directory tree and globbing
for the `.plcproj` extension (`walk_files` + filter). It has no concept
of a `.sln` solution or a `.tsproj` TwinCAT project at all -- it treats
every `.plcproj` it happens to find on disk as live.

That heuristic can silently pick the wrong project. Real example, found
in a private TwinCAT checkout (`MONETN`): the directory
`MONETN/MONETN/MONETNRuntime/` contains two files --
`MONETNRuntime.plcproj` (current) and `MONETRuntime.plcproj` (a stale
leftover from renaming the project `MONET` -> `MONETN`). The existing
per-directory dedup (see `2026-08-02-multi-plcproj-discovery.md`) keeps
only the first sorted entry, so it currently picks
`MONETNRuntime.plcproj` -- but only because `"MONETNRuntime"` sorts
before `"MONETRuntime"`. That is alphabetical luck, not correctness; a
differently-named stale file would silently pick the wrong project with
no error.

The authoritative chain that Visual Studio / TcXaeShell actually uses is
different and unambiguous:

- The `.sln` file lists the projects in the solution (line-oriented
  format, not XML): `Project("{TypeGUID}") = "Name", "RelativePath",
  "{ProjectGUID}"`. In `MONETN.sln`, only one of the three listed
  projects is a TwinCAT PLC project (`MONETN\MONETN.tsproj`) -- the
  others (`DriveManager.tcdmproj`, `Scope.tcmproj`) are different VS
  project types entirely and must be ignored.
- Each `.tsproj` (XML) has one or more nested `<Project GUID="..."
  Name="..." PrjFilePath="...">` elements -- one per PLC sub-project it
  contains. `MONETN.tsproj` has two: one with
  `PrjFilePath="MONETNRuntime\MONETNRuntime.plcproj"` (the live PLC
  project) and one with `PrjFilePath="MONETNTwinSAFE\MONETNTwinSAFE.splcproj"`
  (a TwinSAFE safety project -- different extension, out of scope, see
  below). `MONET.tsproj` (the orphaned pre-rename file, not referenced
  by the `.sln` at all) points at the stale `MONETRuntime.plcproj`.
  Resolving through the `.sln` never touches it.

So `.sln` -> `.tsproj` -> `PrjFilePath` deterministically identifies
exactly the live `.plcproj` files, with no dependence on file naming or
sort order.

Filed as
[ironplc/ironplc#1292](https://github.com/ironplc/ironplc/issues/1292),
a follow-up from PR #1279's review discussion. Part of the broader
TwinCAT dialect effort (#1199).

## Design

Add a new, higher-priority resolution path ahead of the existing
recursive walk, and keep the walk as an explicit fallback:

1. **Find `.sln` files.** Look directly in the directory passed to
   `discover()` (not recursively) -- a `.sln` is always the file a user
   points Visual Studio / `ironplcc` at; it does not need the same
   nested-directory tolerance `.plcproj` does. If more than one `.sln`
   exists at that level, this is ambiguous; treat it the same as "no
   `.sln`" and fall through to the existing walk (do not guess).
2. **Parse the `.sln`.** Line-oriented, not XML, and with a real
   grammar: a project entry
   (`Project("{...}") = "Name", "RelativePath", "{...}"`) may own nested
   `ProjectSection` blocks, solution folders are project entries too,
   and a `Global` block with its own nested sections closes the file.
   Delegate to [`solp`](https://crates.io/crates/solp) (MIT, the parsing
   library behind the `solv` solution-validation tool) rather than
   hand-rolling a line scanner: call `solp::parse_str` and read
   `Solution::projects`. Keep only entries whose `path_or_uri` ends in
   `.tsproj` (case-insensitive), discarding other VS project types
   (`.tcdmproj`, `.tcmproj`, solution folders, etc.) outright.

   Delegating also makes a corrupt `.sln` a *whole-file* rejection: a
   line scanner would happily scrape entries out of a truncated file and
   resolve them to nothing, whereas a failed parse falls back to the
   recursive walk (step 6), which is the behavior a user with a broken
   solution file actually wants.

   Costs, accepted deliberately: `solp` pulls in 55 transitive crates
   (`lalrpop` as a build-time parser generator, `miette` with `fancy`,
   `serde-xml-rs`, `jwalk`, ...) and pins every direct dependency with
   `=`, which forces the workspace lockfile onto `serde` 1.0.228 and
   `winnow` 1.0.3. Both show up in the published SBOM. The alternative
   -- ~30 lines of `split('"')` -- is cheaper on paper but re-derives a
   format we do not own, and IronPLC already prefers a maintained parser
   over an ad-hoc one for `.plcproj` (`roxmltree`) and TOML (`toml`).
3. **Parse each `.tsproj`.** Resolve its path relative to the `.sln`'s
   directory (normalizing `\` like the existing `.plcproj` `<Compile
   Include>` handling). Find every nested `<Project GUID="..."
   PrjFilePath="...">` element (distinguish from the enclosing top-level
   `<Project ProjectGUID="...">` by the presence of `PrjFilePath`, not
   by nesting depth, since `roxmltree` traversal via `.descendants()` is
   already the pattern `parse_plcproj` uses). Keep only entries whose
   `PrjFilePath` ends in `.plcproj` (case-insensitive) -- explicitly
   **skip `.splcproj`** (TwinSAFE safety projects use a different
   compilation model entirely; out of scope here and for the compiler
   generally today).
4. **Resolve each `PrjFilePath`** relative to its owning `.tsproj`'s
   directory, same backslash-normalization as step 3.
5. **Feed the resulting `.plcproj` path list into the existing merge
   logic.** Refactor `detect_twincat` so the "parse each `.plcproj` and
   merge" body (currently the second half of the function, from `let
   single = ...` onward) takes an explicit `Vec<PathBuf>` of already
   resolved `.plcproj` paths, rather than always deriving that list from
   `walk_files` itself. Both the new `.sln`-based path and the existing
   recursive-walk path call the same merge routine -- no duplicated
   merge/dedup logic.
6. **Fallback.** If no `.sln` is found (or more than one, per step 1),
   fall through unchanged to today's recursive `.plcproj` walk. This is
   required by the issue's acceptance criteria (bare-`.plcproj`
   directories and non-solution layouts must keep working) and costs
   nothing extra since the merge logic is now shared.

No change to `parse_plcproj` itself, to `DiscoveredProject`, or to the
library-reference parsing -- this only changes *which* `.plcproj` paths
get fed into the existing, unchanged parse/merge pipeline.

## Files

- `compiler/sources/Cargo.toml`: add the `solp` dependency.
- `compiler/sources/src/discovery/mod.rs`:
  - New `find_sln(dir) -> Option<PathBuf>` -- top-level-only search,
    `None` on zero or multiple matches.
  - New `parse_sln(sln_path) -> Vec<PathBuf>` -- hands the file to
    `solp::parse_str` and returns resolved `.tsproj` paths.
  - New `resolve_plcproj_via_tsproj(tsproj_path) -> Vec<PathBuf>` --
    returns resolved `.plcproj` paths from a single `.tsproj`'s nested
    `<Project PrjFilePath="...">` entries, filtered to `.plcproj`.
  - New `resolve_windows_path(base, relative, extension)` -- the one
    place that filters by extension and resolves a Windows-style
    relative path, shared by the `.sln` and `.tsproj` steps.
  - `detect_twincat`: try `.sln`-based resolution first (steps 1-4
    above, flattened across however many `.tsproj` the `.sln`
    references); if it yields no `.plcproj` paths (no `.sln`, or a
    `.sln` found but it references no `.tsproj`/no `.plcproj`), fall
    back to the existing recursive-walk candidate list. Feed whichever
    list into the existing shared merge body (step 5).

## Tests

Synthetic fixtures only (mirroring the structure of the real `MONETN`
case, not copying its actual proprietary content). Because the `.sln`
grammar is handled by a parser rather than a line scanner, the `.sln`
fixture helper writes what TcXaeShell actually emits -- UTF-8 BOM, CRLF,
version preamble, a solution-folder entry with a nested
`ProjectSection`, and a trailing `Global` block -- instead of a reduced
`Project(...)`-lines-only file:

- `discover_when_sln_present_then_resolves_via_tsproj` -- minimal
  `.sln` + `.tsproj` + `.plcproj`, asserts the file named by
  `PrjFilePath` is loaded.
- `discover_when_sln_project_name_contains_comma_then_resolves` -- a
  project named `Machine, rev B`; asserts the free-text name does not
  shift which field the path is read from.
- `discover_when_sln_is_malformed_then_falls_back_to_walk` -- a
  truncated `.sln` naming a `.tsproj` that does not exist, plus a
  bare `.plcproj` in the same directory; asserts the whole `.sln` is
  rejected and the recursive walk finds the project, rather than a
  partial scrape resolving to nothing.
- `discover_when_sln_lists_non_tsproj_project_then_ignored` -- `.sln`
  with one `.tsproj` entry and one non-PLC entry (e.g. `.tcmproj`),
  asserts only the `.tsproj`-derived files are loaded and the other
  entry does not cause an error.
- `discover_when_sln_and_stale_duplicate_plcproj_then_picks_named_one`
  -- the `MONETN` regression case: two `.plcproj` in one directory, a
  `.tsproj` naming only one of them via `PrjFilePath`, a stale
  `.tsproj` (not referenced by the `.sln`) naming the other. Asserts
  the `.sln`-driven result loads only the `PrjFilePath`-named one,
  regardless of alphabetical order (name the stale file so it would
  sort first, so this test would fail under the old glob-only logic).
- `discover_when_tsproj_references_splcproj_then_skipped` -- a
  `.tsproj` with one `.plcproj` `<Project>` entry and one `.splcproj`
  entry; asserts only the `.plcproj` one is loaded.
- `discover_when_multiple_sln_at_top_level_then_falls_back_to_walk` --
  two `.sln` files in the same directory; asserts behavior matches
  today's (no-`.sln`) recursive-walk result, not an error.
- `discover_when_no_sln_then_falls_back_to_recursive_walk` -- confirms
  all existing recursive-walk tests are unaffected (regression only,
  no new assertions needed beyond the existing suite passing).
- `discover_when_sln_references_tsproj_with_multiple_plcproj_then_merges_all`
  -- one `.tsproj` with two `.plcproj`-typed `<Project>` entries (a main
  PLC project + a library sub-project, the common real-world shape);
  asserts both are merged into one compilation unit via the existing
  merge logic.

All existing tests in this module must keep passing unchanged --
this is strictly additive above the existing recursive-walk path, which
remains the fallback.

## Manual verification

Not committed to the repo (real, personal TwinCAT solutions), but used
to sanity-check before relying on synthetic fixtures alone: run
`ironplcc check --dialect twincat` (or the LSP) against local `brotlib`
checkouts (`MONETN`, `MONETS`, `IAG50cm`, etc.) before and after the
change and confirm the file selection differs only where expected (i.e.
`MONETN` now correctly excludes the stale `MONET.tsproj`'s
`MONETRuntime.plcproj`).

## Out of scope

- `.splcproj` (TwinSAFE) support -- explicitly skipped at the
  `.tsproj`-parsing step, not modeled at all.
- Other VS project types referenced by a `.sln` (`.tcdmproj` /
  DriveManager, `.tcmproj` / Scope, etc.) -- ignored, not an error.
- Solution configurations/platforms (`GlobalSection
  (SolutionConfigurationPlatforms)` etc.) -- irrelevant to source
  discovery, not parsed.
- Nested/multi-level `.sln` discovery (a `.sln` referencing another
  `.sln`) -- not a real TwinCAT pattern as far as observed; not
  handled.
