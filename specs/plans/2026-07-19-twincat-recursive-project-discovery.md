# Plan: Recursive Project Discovery (`.plcproj` and Fallback File Enumeration)

## Goal

`discover()` in `compiler/sources/src/discovery/mod.rs` only looks at the
immediate children of the directory it's given — both `detect_twincat()`
(searching for a `.plcproj`) and `detect_fallback()` (enumerating
supported files when no project file is found) call `fs::read_dir(dir)`
once and never recurse into subdirectories.

Real TwinCAT project layouts are inherently nested (Visual-Studio-style
solution/project structure). In a private test corpus of real TwinCAT
projects, `.plcproj` lives 2-3 levels below the directory a user would
naturally point the tool at:

```
TestProject/                             <- naturally the "project directory"
  TestProject/
    TestProjectRuntime/
      TestProjectRuntime.plcproj         <- actual project file, 2 levels down
      POUs/...
```

`ironplcc check TestProject` never finds the `.plcproj`, falls through to
the fallback enumerator (which also only checks the top level), finds
nothing there either, and reports `P9002 Set of valid source files has
no content`. Pointing at the exact subdirectory works immediately —
confirming discovery itself is the blocker, not anything downstream.

## Verification this is real (not just theoretical)

Confirmed directly against a private local checkout of a real TwinCAT
project laid out exactly as above:

```
$ ironplcc check --dialect codesys TestProject
error[P9002]: Set of valid source files has no content

$ ironplcc check --dialect codesys TestProject/TestProject/TestProjectRuntime
error[P6007]: File type is not supported ... VisualizationManager.TcVMO
error[P0002]: Syntax error ... POUs/MAIN.TcPOU:66:33
```

The second command immediately finds and processes real content
(different, unrelated errors — confirming the fix target is purely
discovery, not something downstream).

Also confirmed (relevant to the design below): the real corpus has a
directory with **two** `.plcproj` files (`TestProjectRuntime.plcproj` and
`TestRuntime.plcproj` — a stale duplicate from an apparent rename),
and both `.git/` and `.idea/` directories are present under the
project root — recursion must not wastefully (or riskily, in `.git`'s
case) descend into these.

Also confirmed multi-file cross-file type resolution already works
correctly today once files ARE discovered together (verified separately,
not part of this change) — this fix's payoff is real: it's what's
actually standing between the tool and being usable against real TwinCAT
checkouts at all, for both the single-project and (once resolvable)
cross-file cases.

## Design

### One shared recursive-walk helper

```rust
/// Recursively collects all regular files under `dir`. Skips hidden
/// directories (name starts with `.` -- `.git`, `.idea`, `.vs`, etc.,
/// all observed in real checkouts) and does not follow symlinks (treats
/// them as neither a file nor a directory), which also rules out
/// symlink cycles. Each directory's entries are sorted by name before
/// recursing, so the result is deterministic regardless of filesystem
/// iteration order.
fn walk_files(dir: &Path, out: &mut Vec<PathBuf>) { ... }
```

Both `detect_twincat()` and `detect_fallback()` call this once and then
filter the shared file list according to their own criteria (`.plcproj`
extension vs. `FileType::is_supported()`), instead of duplicating
traversal logic.

### `detect_twincat()`: multiple `.plcproj` candidates

Since recursion can now surface more than one `.plcproj` in the tree
(confirmed real, see above), candidates are sorted lexicographically by
full path and the first is used — preserving the existing "just pick
one, no ambiguity error" behavior (the pre-existing single-level code
already picked an arbitrary match with no disambiguation), just made
deterministic across the whole tree instead of non-deterministic within
one directory's `read_dir` order. Verified this tie-break happens to pick
the "obviously correct" one in the real duplicate case
(`TestProjectRuntime.plcproj` sorts before `TestRuntime.plcproj`, and also
happens to be the one whose name matches its containing folder).

**Non-goal**: smarter disambiguation (e.g., preferring a `.plcproj` whose
filename matches its parent directory's name) — no evidence this is
needed beyond the one coincidental case already found, and it would add
inference complexity beyond what this bug fix requires.

### `detect_twincat()`: `root_dir` must resolve relative to the `.plcproj`'s own directory

Currently `parse_plcproj(&plcproj_path, dir)` passes the *original*
`dir` argument as the base for resolving each `<Compile Include="...">`
path. That was correct when `.plcproj` was required to live directly in
`dir`, but once it can be nested arbitrarily deep, `<Compile>` paths
(which are always relative to the `.plcproj` file's own location) must
resolve against `plcproj_path.parent()`, not the original `dir`. Also
updates `DiscoveredProject.root_dir` to reflect the actual project
directory (currently unused by any other code, but part of the public
struct, so should be correct regardless).

### `detect_fallback()`

Same shared `walk_files()`, filtered by `FileType::is_supported()`,
sorted for deterministic output — otherwise unchanged (still returns
`dir` as `root_dir`, since fallback mode has no project-file location to
derive it from).

### `detect_beremiz()`: left unchanged, deliberately

Beremiz's `plc.xml` convention is a flat, single-file-in-the-given-
directory layout (unlike TwinCAT's nested Visual-Studio-style structure)
— no evidence or report of this needing recursion, so left as-is to keep
this change scoped to the actual reported problem.

## Non-goals

- A depth limit or file-count cap on the recursive walk. Hidden-directory
  skipping (`.git` in particular) already rules out the realistic
  pathological case; adding an arbitrary cap for defense-in-depth beyond
  that isn't something the bug report calls for.
- Making `detect_beremiz()` recursive.
- Verifying/fixing cross-file type resolution itself — confirmed
  separately (already works for ordinary FB/struct/enum references;
  `INTERFACE`-typed variables still hit the pre-existing, unrelated
  `P2008`/`INTERFACE`-as-variable-type gap regardless of file layout).
- A new `walkdir` dependency — hand-rolled recursion is simple enough
  here and avoids adding a dependency for this.

## File Map

| File | Change |
|------|--------|
| `compiler/sources/src/discovery/mod.rs` | `walk_files()` helper; `detect_twincat()`/`detect_fallback()` use it; `root_dir` fix in the `.plcproj` case |

## Testing Strategy

- `detect_twincat`: `.plcproj` nested 2-3 levels deep is found (matches
  the real test-corpus layout); hidden directories (`.git`-named dir
  containing a decoy `.plcproj`) are not descended into; multiple
  `.plcproj` candidates at different depths resolve deterministically
  (sorted); `<Compile Include="...">` paths in a nested `.plcproj`
  resolve relative to the `.plcproj`'s own directory, not the original
  root passed to `discover()`.
- `detect_fallback`: supported files nested in subdirectories are found;
  hidden directories are skipped; deterministic (sorted) output
  preserved (regression, existing tests already check this at one
  level).
- Regression: all existing single-level tests continue to pass unchanged
  (recursion is a superset of the single-level case).
- Symlink handling: a symlinked directory is not followed (cheap to test
  cross-platform via `std::os::unix::fs::symlink` under `#[cfg(unix)]`,
  or skip if not practical in CI).

## Tasks

- [x] Write plan (this document)
- [x] `walk_files()` shared recursive helper
- [x] Wire into `detect_twincat()` (+ `root_dir`/`parse_plcproj` base-path fix)
- [x] Wire into `detect_fallback()`
- [x] Tests from Testing Strategy (including the symlink-cycle-safety test)
- [x] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork (no PR against `ironplc/ironplc` without explicit
      go-ahead, per standing instruction)
- [ ] Merge into `twincat-dev`, update `twincat-status.md`, push

## Implementation Notes

- **Verified end-to-end against the real corpus, before and after**:
  `ironplcc check TestProject` (top-level directory) went from
  `P9002 Set of valid source files has no content` to actually parsing
  real POUs (surfacing unrelated, pre-existing errors — unsupported
  `.TcVMO`/`.TcTTO` visualization file types, an unsupported
  `REFERENCE TO` variable, an unrelated parser edge case — confirming the
  fix is isolated to discovery, nothing downstream needed to change).
- **The real duplicate-`.plcproj` case resolved correctly by coincidence
  of the lexicographic tie-break**: `TestProjectRuntime.plcproj` sorts
  before `TestRuntime.plcproj`, and is also the one whose name matches
  its containing folder (the "correct" one). Not a designed heuristic —
  just confirms the simple tie-break didn't make things worse for the one
  real case found with multiple candidates.
- **The `root_dir`/base-path fix was easy to miss but load-bearing**:
  `parse_plcproj`'s second argument is used to resolve every
  `<Compile Include="...">` path, and those paths are always relative to
  the `.plcproj` file's own directory — not the directory originally
  passed to `discover()`. This was harmless before (when `.plcproj` was
  required to live directly in that directory, the two were the same
  path) but silently wrong once nesting is allowed. Caught by writing a
  test with a `.plcproj` referencing a file in its own subdirectory
  before assuming the straightforward wiring was correct.
