# Plan: Don't Abort Project Discovery on a Single Unresolvable `.plcproj` Entry

## Goal

`parse_plcproj()` in `compiler/sources/src/discovery/mod.rs` treats *any*
single unresolvable `<Compile Include="...">` entry as fatal for the
*entire* project — it returns `Err(...)` on the first one found, before
even attempting to resolve the rest. One bad reference (a stale entry,
a case-sensitivity mismatch, a genuinely missing file) means **zero**
files from that project ever get checked, even ones that are perfectly
valid and would otherwise pass cleanly.

```
$ ironplcc check <project-dir>      # A.st is valid; project.plcproj also references a missing file
error[P6004]: Unable to read file
  Referenced file does not exist: MISSING_FILE.st (...)
Error: "Error enumerating files"
```

`A.st` never gets checked at all. Every other per-file problem in the
codebase (unsupported file type, syntax error, semantic error) is
collected and reported *without* preventing the rest of the project from
being checked — this is the one place that doesn't follow that pattern.

## Verification this is real

Confirmed directly with a synthetic repro (as above): a `.plcproj` with
one valid `<Compile>` entry and one referencing a nonexistent file
produces a single `P6004` diagnostic and a hard `Err` — the valid file
is never even attempted. Also confirmed against a real project's own
survey (a different project using IronPLC as a diagnostics backend):
27 files across 2 sub-projects there have never been checked at all
because of this — one case is a case-sensitivity mismatch between a
`.plcproj` entry and the real filename (fine on the Windows/NTFS
filesystem the project was authored on, fatal on Linux/ext4), the other
a genuinely missing referenced file (a non-ST asset, plausibly never
committed to that particular checkout).

## Design

### `DiscoveredProject` gains a `warnings` field

```rust
pub struct DiscoveredProject {
    pub project_type: ProjectType,
    pub root_dir: PathBuf,
    pub files: Vec<PathBuf>,
    /// Non-fatal problems found during discovery -- currently just
    /// `.plcproj` <Compile Include="..."> entries that don't resolve to
    /// a real file. These don't prevent the rest of the project's files
    /// from being checked; callers should surface them without treating
    /// discovery itself as having failed.
    pub warnings: Vec<Diagnostic>,
}
```

### `parse_plcproj`: collect and skip, don't abort

Instead of `return Err(...)` on the first `!resolved.is_file()`, push a
diagnostic onto a local `warnings: Vec<Diagnostic>` and `continue` the
loop over `<Compile>` elements -- every other resolvable entry still gets
added to `files`. Returns `Ok(DiscoveredProject { ..., warnings })`
unconditionally once the XML itself is parsed -- there's no longer a
per-entry failure path.

**What still remains a hard `Err`**: the `.plcproj` file itself being
unreadable (I/O error) or malformed XML -- both mean there's no project
structure to resolve at all, unlike a single bad entry within an
otherwise-readable, well-formed project file.

### Callers surface `warnings` without treating discovery as failed

`enumerate_files()` in `compiler/ironplc-cli/src/cli.rs` (currently
`Result<Vec<PathBuf>, Vec<Diagnostic>>`, called from `create_project()`
which already has a `suppress_output: bool` in scope) gains a
`suppress_output: bool` parameter, prints `project.warnings` via the
existing `handle_diagnostics` helper (same rendering path already used
for real errors, just non-fatal here), and returns `Ok(project.files)`
exactly as before -- the return type doesn't change, so every other
caller/subcommand is unaffected.

## Non-goals

- A case-insensitive filesystem fallback for `<Compile Include="...">`
  resolution -- surfacing the mismatch as a non-fatal warning (so the
  rest of the project still gets checked) is the fix; silently
  "correcting" a case mismatch would hide a real project-file bug from
  the user instead of just not letting it block everything else.
- Changing `discover()`'s own signature (`Result<DiscoveredProject,
  Diagnostic>`) -- the change is additive (`warnings` field) and
  entirely internal to how `parse_plcproj` populates it.
- Any change to `detect_fallback()` or `detect_beremiz()` -- neither has
  an analogous "one bad reference aborts everything" failure mode to fix.

## File Map

| File | Change |
|------|--------|
| `compiler/sources/src/discovery/mod.rs` | `DiscoveredProject.warnings`; `parse_plcproj` collects+skips instead of aborting |
| `compiler/ironplc-cli/src/cli.rs` | `enumerate_files` takes `suppress_output`, prints `project.warnings` non-fatally |

## Testing Strategy

- `parse_plcproj`/`discover`: a `.plcproj` with one valid and one
  unresolvable `<Compile>` entry now returns `Ok` with the valid file
  present and one warning recorded (regression-flips the two existing
  tests that currently assert this is a hard `Err` -- that assertion was
  testing the exact bug this branch fixes).
  a `.plcproj` where *every* entry is unresolvable still returns `Ok`
  with an empty file list and one warning per entry (not a special
  case -- matches the existing "no `<Compile>` entries at all" precedent).
- Regression: the `.plcproj`-file-itself-unreadable and malformed-XML
  cases still return `Err` unchanged.
- CLI: `enumerate_files`/`create_project` print warnings without
  aborting the rest of the check (verified via an integration-style test
  or direct inspection of `create_project`'s behavior).

## Tasks

- [ ] Write plan (this document)
- [ ] `DiscoveredProject.warnings` field
- [ ] `parse_plcproj`: collect+skip instead of abort
- [ ] Update the 2 existing tests whose old assertions tested the bug
      being fixed
- [ ] New tests from Testing Strategy
- [ ] `enumerate_files`/`create_project`: thread `suppress_output`,
      surface warnings non-fatally
- [ ] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork
- [ ] Merge into `twincat-dev`, update `twincat-status.md`, push
