# Diagnostic Line/Column in Playground

## Goal

Display diagnostic locations in the playground as line and column numbers
instead of raw byte offsets. Reuse the offset-to-line/column conversion the
LSP server already performs so a single routine serves both.

## Context

Follow-up to `2026-04-21-playground-line-numbers.md`, which added a line
number gutter but explicitly excluded mapping diagnostic byte offsets to line
numbers.

Today the playground shows `offset 123–145` (`playground/app.js`
`renderDiagnostics`). The LSP server already converts offsets to line/column
for `textDocument/publishDiagnostics` in two near-duplicate helpers
(`span_to_range` and `map_label` in `compiler/ironplc-cli/src/lsp_project.rs`).

## Architecture

Add one shared function in `ironplc_dsl::diagnostic` that converts a byte
offset in a source string to a 0-based (line, column) pair. Replace the inline
loops in `lsp_project.rs` with calls to this helper, and extend the playground
WASM `DiagnosticInfo` with 1-based `start_line`/`start_column`/`end_line`/
`end_column` fields computed using the same helper. Update `app.js` to render
`line N, column M` instead of `offset X–Y`.

## File map

- `compiler/dsl/src/diagnostic.rs` — add `offset_to_line_column` helper + tests
- `compiler/ironplc-cli/src/lsp_project.rs` — use the helper in `span_to_range`
  and `map_label`
- `compiler/playground/src/lib.rs` — add line/column fields to `DiagnosticInfo`
  and populate them from source + offsets
- `playground/app.js` — render line/column instead of offset
- `playground/tests/e2e.spec.js` — update/add test for the new display

## Tasks

- [ ] Add `offset_to_line_column` in `ironplc_dsl::diagnostic`
- [ ] Refactor LSP helpers to use it
- [ ] Extend playground `DiagnosticInfo` with line/column fields
- [ ] Update `app.js` to render line/column
- [ ] Run `cd compiler && just`
