# Plan: Publish LSP Diagnostics for Every File in the Workspace

## Overview

The IronPLC LSP server only publishes diagnostics for the file the user just edited, even though the compiler analyses the whole workspace and produces diagnostics for every file. When a project spans multiple files, errors in unedited files (including the ones causing analysis to fail) are silently dropped. This plan rewires the LSP layer to publish per-file diagnostics for every affected URI in the workspace and to clear stale diagnostics correctly.

## Current State

### What Exists

- **`LspProject::semantic(uri)`** at `compiler/ironplc-cli/src/lsp_project.rs:111` — runs `Project::semantic()` (whole-workspace analysis) and then **filters** `diagnostics.filter(|d| d.file_ids().contains(&file_id))` so only diagnostics touching the requested URI survive.
- **`did_open`/`did_change` handlers** at `compiler/ironplc-cli/src/lsp.rs:344` and `compiler/ironplc-cli/src/lsp.rs:370` — call `semantic(uri)` and emit a single `PublishDiagnostics` notification keyed to that URI.
- **Compiler `Project::semantic()`** at `compiler/project/src/project.rs:194` — already analyses every source in the project and returns `Result<(), Vec<Diagnostic>>` containing diagnostics from any file.
- **`Diagnostic::file_ids()`** at `compiler/dsl/src/diagnostic.rs:289` — returns the set of `FileId`s referenced by a diagnostic's primary plus secondary labels.
- **`FileId`** is `File(Arc<str>)` where the inner string is the file system path; can be reconstructed to a URI via `file://{path}`.
- **Test scaffolding (`TestServer`)** at `compiler/ironplc-cli/src/lsp.rs:462` — a self-contained in-process LSP harness that lets tests send notifications and consume `PublishDiagnostics` notifications synchronously.

### What's Missing

- No grouping of diagnostics by URI: the filter at `lsp_project.rs:121` drops every diagnostic that does not touch the edited URI.
- No fan-out in the handlers: `did_open`/`did_change` only ever send one `PublishDiagnostics` notification per edit.
- No tracking of which URIs have outstanding diagnostics, so when an error in `b.st` is fixed by editing `a.st` the squiggle in `b.st` would never be cleared even after fan-out.
- No multi-file integration tests covering the publish/clear lifecycle.

## Design

The standard LSP convention (rust-analyzer, gopls, Pyright, tsserver) is:

1. After any change, recompute project-wide diagnostics.
2. Group them by file URI.
3. Send one `publishDiagnostics` notification per file with current errors.
4. For every URI that previously had diagnostics but no longer does, send `publishDiagnostics(uri, [])` to clear the stale squiggles.

This plan adopts the same pattern.

### New API on `LspProject`

Replace the per-URI `semantic(&Uri) -> Vec<Diagnostic>` with a project-wide method:

```rust
pub(crate) fn semantic_all(&mut self) -> HashMap<Uri, Vec<lsp_types::Diagnostic>>;
```

It runs `self.wrapped.semantic()`, then for every diagnostic groups it by each `FileId` it touches (primary and secondary labels). Each `FileId::File(path)` is mapped to `Uri::from_str(&format!("file://{path}"))`. Diagnostics whose `FileId` is `BuiltIn` or whose path cannot be turned into a valid URI are attributed to the URI most recently provided to `change_text_document` (a fallback "primary" URI), so they never go missing in the diagnostics view.

The existing `semantic(uri)` is kept as a thin wrapper for tests/back-compat (it returns `semantic_all().remove(uri).unwrap_or_default()`), but the LSP handlers stop using it.

### Server-side fan-out and stale clearing

Add to `LspServer`:

```rust
struct LspServer<'a> {
    sender: &'a Sender<Message>,
    project: LspProject,
    /// URIs for which we have a non-empty PublishDiagnostics outstanding.
    /// Used to send empty-diagnostics notifications when previously-failing
    /// files no longer have errors.
    published_uris: HashSet<Uri>,
}
```

After the project state changes (`did_open`, `did_change`, future `did_save`/`did_close`), the server calls `project.semantic_all()`, then:

1. For every `(uri, diags)` in the map: send `PublishDiagnostics{uri, diagnostics: diags, version: ...}`. The version field is `Some(version)` only when `uri` matches the request's `uri`; for other URIs, it's `None` (the LSP spec allows omitting it for files the editor has not opened or that we are publishing on behalf of the workspace).
2. For every URI in `published_uris` that is NOT in the new map: send `PublishDiagnostics{uri, diagnostics: vec![], version: None}` to clear it.
3. Replace `published_uris` with the set of URIs that received non-empty diagnostics.

### Edge cases

- **Empty results:** When `semantic_all()` returns an empty map (analysis succeeded), every previously-published URI must be cleared and `published_uris` reset to empty.
- **Diagnostics with no resolvable file:** Always show on the editing URI to avoid silent loss. Already covered by the `BuiltIn`/fallback rule.
- **Trait change risk:** `LspProject::semantic(uri)` is called by tests; we keep the wrapper to avoid breaking them.
- **HashMap iteration order:** VS Code does not care, but tests must not assume order. The integration tests sort or look up by URI.

## Implementation Steps

1. **Add `semantic_all` to `LspProject`** (`compiler/ironplc-cli/src/lsp_project.rs`).
   - Helper `file_id_to_uri(&FileId) -> Option<Uri>` that reconstructs `file://{path}`.
   - Track `last_changed_uri: Option<Uri>` updated by `change_text_document` for the fallback.
   - Group diagnostics across all `file_ids()`.
   - Keep `semantic(&Uri)` as a back-compat wrapper.
2. **Update `LspServer`** (`compiler/ironplc-cli/src/lsp.rs`).
   - Add `published_uris: HashSet<Uri>` field, initialise empty.
   - Add a helper method `publish_diagnostics(&mut self, edited_uri: &Uri, edited_version: Option<i32>)` that runs `semantic_all`, sends per-file notifications, sends empty-clear notifications, and updates `published_uris`.
   - Replace the existing single-URI publish in `did_open` and `did_change` with this helper.
3. **Unit tests in `lsp_project.rs`.**
   - `semantic_all_when_two_files_each_have_errors_then_returns_diagnostics_for_each_uri`.
   - `semantic_all_when_diagnostic_has_secondary_label_in_other_file_then_diagnostic_appears_for_both_uris` (verifies cross-file diagnostics surface in both files).
   - `semantic_all_when_no_errors_then_empty_map`.
4. **Integration tests in `lsp.rs`** using the existing `TestServer`.
   - `multi_file_when_second_file_has_error_then_diagnostics_published_for_both_files` — open `a.st` (error-free) and `b.st` (parse error), assert two `PublishDiagnostics` notifications, one for each URI, with `b.st` containing the error.
   - `multi_file_when_error_fixed_in_second_file_then_clearing_diagnostics_published` — start with `b.st` broken, then send a `did_change` that fixes `b.st`, assert a clear notification for `b.st`'s URI with `diagnostics == []`.
   - `multi_file_when_unedited_file_starts_referencing_undefined_in_edited_file_then_diagnostic_published_for_unedited_file` — covers the "edit one file but error appears in another" scenario the user reported.
   - `single_file_when_error_introduced_then_cleared_then_clearing_notification_emitted` — regression guard for stale clearing on the same file.

## Acceptance Criteria

- `semantic_all` returns a map keyed by `Uri` containing every diagnostic the compiler produced, regardless of the URI being edited.
- `did_open`/`did_change` cause one `PublishDiagnostics` per affected URI plus one clearing `PublishDiagnostics` for each URI that no longer has diagnostics.
- All new unit and integration tests pass.
- `cd compiler && just` (compile, coverage, lint) passes locally before opening a PR.

## Out of Scope

- A workspace-vs-open-files setting toggle (the rust-analyzer/Pyright-style mode switch). IronPLC projects are typically small; default workspace-wide is fine. The toggle can be added later via `initializationOptions` without changing the architecture introduced here.
- Incremental analysis / dependency tracking. The compiler still re-analyses everything on each edit; that is unchanged.
- `did_save` / `did_close` handlers (none exist today; the same helper would slot in if they are added later).
