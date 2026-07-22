# Plan: Merge Multiple LSP Workspace Folders Into One Compilation Unit

**Status: implemented.** This plan scopes a real, already-known gap
(flagged under "What's Missing" in the pre-existing
`specs/plans/multi-file-project-support.md`, but never given its own
phase there) — written up here at a finer grain because it's directly
motivated by a concrete external use case (an editor plugin using
`ironplcc lsp` as a diagnostics backend for a multi-sub-project TwinCAT
solution).

## Goal

`ironplcc lsp` currently only ever analyzes the **first** LSP workspace
folder sent by the client on `initialize` — every other folder is
silently dropped. A TwinCAT solution with multiple sub-projects (a
common real layout: a top-level solution directory containing several
independent `.plcproj`-rooted library projects as siblings, each
referencing the others by relative path) needs all of them loaded
together as one compilation unit for cross-project type resolution to
work — the same way `ironplcc check dir1 dir2 ...` (multiple CLI
arguments) already merges multiple directories today.

```
TestSolution/
├── TestRuntime/            (main .plcproj, references the libraries below)
├── TestLib1/                (sibling .plcproj, a shared library)
└── TestLib2/                (sibling .plcproj, another shared library)
```

An editor client that's aware of this structure (e.g. via a
`<PlaceholderReference>`-style cross-reference in a `.plcproj`, resolved
against sibling directories) can already tell the LSP server about all
three directories via `workspaceFolders` on `initialize` — every major
LSP client library supports sending more than one folder. `ironplcc`
just throws the extra ones away today.

## Verification against real code

Traced the exact behavior directly:

- `ironplc-cli/src/lsp.rs:127`:
  ```rust
  match initialize_params.workspace_folders {
      Some(folders) => {
          if let Some(folder) = folders.first() {
              server.project.initialize(folder);
          }
      }
      None => {}
  }
  ```
  Only `folders.first()` is ever used.
- `LspProject::initialize` (`ironplc-cli/src/lsp_project.rs:124`) takes a
  single `&WorkspaceFolder` and delegates to
  `FileBackedProject::initialize(&mut self, dir: &Path)`
  (`project/src/project.rs:161`), which delegates to
  `SourceProject::initialize_from_directory` (`sources/src/project.rs:85`).
- **`initialize_from_directory` calls `self.sources.clear()` before
  discovering** (`sources/src/project.rs:88`). This means simply looping
  over all `folders` and calling today's `initialize()` once per folder
  would not merge them — each call would wipe out the previous folder's
  sources, leaving only the last folder's files loaded (a different bug,
  not a fix).
- The correct merge pattern already exists, just not on this code path:
  `ironplc-cli/src/cli.rs`'s `create_project` (backing the CLI's
  multi-argument `ironplcc check dir1 dir2 ...`) loops over each input
  path, calls `enumerate_files`/`discovery::discover()` independently per
  path, accumulates every discovered file into one `Vec<PathBuf>`, and
  only then builds a single `FileBackedProject` and pushes every file
  into it. No clearing between iterations.

So the fix is squarely "make the LSP path do what the CLI path already
does," not new design space.

## Design

### New multi-directory initializer on `SourceProject`

```rust
/// Initialize project from multiple directories using project discovery,
/// merging all discovered files into one compilation unit. Unlike calling
/// initialize_from_directory per directory, this does not clear sources
/// between directories.
pub fn initialize_from_directories(&mut self, dirs: &[&Path]) -> Vec<Diagnostic> {
    self.sources.clear();
    let mut errors = vec![];
    for dir in dirs {
        match crate::discovery::discover(dir) {
            Ok(project) => {
                for file_path in &project.files {
                    if let Err(err) = self.add_file(FileId::from_path(file_path)) {
                        errors.push(err);
                    }
                }
            }
            Err(diag) => errors.push(diag),
        }
    }
    errors
}
```

A single unresolvable directory's discovery failure becomes one
collected diagnostic, not an abort of the remaining directories --
matching the same "one bad entry doesn't take down everything else"
principle already applied to `.plcproj` resolution (see "Done" #11 in
`twincat-status.md`).

### `Project` trait and `FileBackedProject`

Add a parallel `initialize_many(&mut self, dirs: &[&Path]) -> Vec<Diagnostic>`
to the `Project` trait (default-implemented in terms of the existing
single-directory `initialize` for any implementor that doesn't need the
distinction, e.g. `MemoryBackedProject`), overridden in
`FileBackedProject` to call the new `initialize_from_directories`.

### `lsp.rs`: pass every folder, not just the first

```rust
match initialize_params.workspace_folders {
    Some(folders) => {
        let dirs: Vec<PathBuf> = folders.iter().filter_map(|f| to_path_buf(&f.uri).ok()).collect();
        let paths: Vec<&Path> = dirs.iter().map(|p| p.as_path()).collect();
        server.project.initialize_many(&paths);
    }
    None => {}
}
```

## Non-goals

- Any change to the CLI's own multi-argument `check` handling — already
  correct, used as the template here.
- Any IDE/editor-side logic for *deciding* which sibling directories to
  advertise as workspace folders (e.g. parsing a `.plcproj`'s
  cross-project references and resolving them against sibling
  directories) — that's entirely client-side (editor plugin)
  responsibility, out of scope for `ironplcc` itself. This plan only
  covers `ironplcc` correctly using whatever folders a client already
  sends.
- Live re-initialization when workspace folders change after the initial
  `initialize` (LSP's `workspace/didChangeWorkspaceFolders` notification)
  — not confirmed as needed by the motivating use case; a separate
  follow-up if it turns out to matter.
- Deduplicating files that appear in more than one workspace folder's
  discovered set (e.g. overlapping/nested folders) — no evidence this
  occurs in practice; `add_file` already errors clearly on a duplicate
  `FileId` if it ever does.

## File Map

| File | Change |
|------|--------|
| `compiler/sources/src/project.rs` | New `initialize_from_directories()` |
| `compiler/project/src/project.rs` | `Project` trait: new `initialize_many()`; `FileBackedProject` override |
| `compiler/ironplc-cli/src/lsp.rs` | Pass all `workspace_folders`, not just the first |

## Testing Strategy

- `initialize_from_directories()`: two directories' files both end up in
  the same project; a directory that fails to discover doesn't prevent
  the other directory's files from loading (mirrors the `.plcproj`
  warnings test shape).
- `FileBackedProject::initialize_many`: same coverage at the `Project`
  trait level.
- LSP integration-style test (if the existing LSP test harness supports
  multi-folder `initialize` params): confirm a cross-folder type
  reference resolves once both folders are loaded together.
- Regression: single-workspace-folder `initialize` (today's only tested
  case) still works unchanged.

## Tasks

- [x] Confirm priority against the rest of `twincat-status.md`'s "Next"
      list before starting implementation
- [x] `initialize_from_directories()` + tests
- [x] `Project` trait `initialize_many()` + `FileBackedProject` override
- [x] `lsp.rs`: pass all workspace folders
- [x] Tests from Testing Strategy
- [x] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork
- [ ] Merge into `twincat-dev`, update `twincat-status.md`, push

## Implementation Notes

- `SourceProject::initialize_from_directory` and the new
  `initialize_from_directories` share a private `discover_and_add`
  helper (discover one directory, add its files, don't clear) --
  `initialize_from_directory` clears once then calls it once,
  `initialize_from_directories` clears once then calls it per directory.
  No duplicated discovery logic between the single- and multi-directory
  paths.
- `Project::initialize_many`'s default implementation (for any future
  implementor that doesn't override it) delegates to `initialize` for
  zero or one directories, and returns a `Problem::NoContent` diagnostic
  for more than one -- deliberately not attempting a "call `initialize`
  per directory" default, since that would silently reproduce the exact
  clearing bug this plan fixes for any implementor whose `initialize`
  clears state per call. Only `FileBackedProject` needed a real
  multi-directory implementation; `MemoryBackedProject` never receives
  directories at all (its `initialize` already errors unconditionally).
- Renamed `LspProject::initialize` to `initialize_many` (no `initialize`
  overload kept) -- one single-folder-shaped call site (`lsp.rs`), so a
  parallel single-folder method would have been unused surface area.
- The most direct verification of "does this actually fix the reported
  problem" wasn't a unit test on the merge logic in isolation, but an
  end-to-end test (`initialize_many_when_two_folders_then_cross_folder_type_resolves`):
  two real temp directories, one declaring a function block, the other
  referencing it by type name, wired through the real
  `LspProject::initialize_many` -> `semantic_all()` path -- proving the
  cross-folder type reference resolves with zero diagnostics once both
  folders are loaded together, not just that files get merged into one
  list.
