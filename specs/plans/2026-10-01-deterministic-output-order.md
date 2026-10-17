# Deterministic Output Order for Echo, Tokenize, Symbols and LSP

Plan for [#1951](https://github.com/ironplc/ironplc/issues/1951).

## Goal

Several outputs that are not compiler diagnostics or bytecode list their
items in `HashMap` order, which changes from one process to the next. Give
each one a fixed, meaningful order:

- `ironplcc echo` and `ironplcc tokenize` with several files: the files in
  the order they were given (command-line order, then the sorted order that
  project discovery already uses inside a directory).
- MCP `symbols` tool: the `functions` and `types` arrays in source order
  (file, then position in the file).
- LSP `textDocument/documentSymbol`: the symbols of the document in source
  order, whatever their kind.
- LSP `textDocument/publishDiagnostics` for the whole workspace: one
  notification per document in a fixed order (by URI), including the empty
  notifications that clear documents that no longer have problems.

`compile` and `check` already merge sources sorted by `FileId`
(`project/src/project.rs`), so analysis results and bytecode are not
affected; this change does not touch that ordering.

## Audit

Every non-test iteration of a `HashMap` or `HashSet` in `compiler/` was
reviewed. Besides the outputs above, the only order-dependent places are the
ones [#1963](https://github.com/ironplc/ironplc/pull/1963) already fixes
(user function block descriptors and enumeration debug definitions in
codegen, the "did you mean" tie-break in `find_closest_match`). The rest are
lookups, sets used only with `contains`, map-to-map copies, or outputs that
are sorted later (`types_all`, `project_manifest`, `pou_lineage`; the MCP
`run` trace map is a `serde_json::Map`, which is a `BTreeMap`). Diagnostics
are not re-sorted before printing in the CLI, the LSP or the MCP server; they
keep pipeline order, which is deterministic once sources are merged in a
fixed order. No follow-up issue is needed.

## Architecture

- **Sources.** `SourceProject` stores its sources in a
  `HashMap<FileId, Source>`. Replace it with an `IndexMap<FileId, Source>`
  (the `indexmap` crate is already a workspace dependency through the
  analyzer). `sources()` and `sources_mut()` then return the sources in
  insertion order; replacing a source keeps its position, and
  `remove_source` uses `shift_remove` so the remaining order is kept.
  The explicit sort in `project/src/project.rs` stays, so `check` and
  `compile` keep their current merge order.
- **Extractors.** `SemanticContext::user_defined_functions()` and
  `user_defined_types()` (`analyzer/src/extractors.rs`) sort their result by
  the declaration span: file id (as a string, the same key the project merge
  uses), then start offset, then name as a tie-break for built-in or
  synthetic spans. Both the MCP `symbols` tool and the LSP document symbols
  get source order from this one place. `FunctionEnvironment` and
  `TypeEnvironment` keep their `HashMap`s; they are lookup tables, and
  changing their storage is a larger change than this issue needs.
- **Document symbols.** `document_symbols` (`ironplc-cli/src/lsp_project.rs`)
  builds types, then function blocks, then functions. Sort the final list by
  range start so the outline follows the file, independent of kind.
- **Workspace diagnostics.** `semantic_all` returns a
  `HashMap<UriKey, Vec<Diagnostic>>` and `published_uris` is a
  `HashSet<UriKey>`. Publish in URI order: collect into a `BTreeMap` (or sort
  the keys) before sending, and sort the stale URIs before clearing them.

## Prefactoring

None needed. Each fix is local to the function that produces the output: the
storage change in `SourceProject` is the fix itself, and the extractor sort
is one place that serves both consumers. No new branching is repeated across
call sites. `lsp_project.rs` is already over 1000 lines; the change adds only
a sort call, and any new test helpers go in a new test module rather than in
that file.

## Design doc reference

- `specs/design/mcp-server.md`, `symbols` section: add
  **REQ-TOL-mcp-056** — the `programs`, `functions`, `function_blocks` and
  `types` arrays list declarations in source order (sources in the order the
  project merges them, then position within the source), identically on
  every call. Tagged on the new MCP test.
- `docs/reference/compiler/ironplcc.rst`: state that `echo` and `tokenize`
  process files in the order given.

## File map

- `compiler/sources/Cargo.toml` — add `indexmap`.
- `compiler/sources/src/project.rs` — `IndexMap` storage, `shift_remove`.
- `compiler/analyzer/src/extractors.rs` — sort by declaration span.
- `compiler/mcp/src/tools/symbols.rs` — test for REQ-TOL-mcp-056.
- `compiler/ironplc-cli/src/lsp_project.rs`, `compiler/ironplc-cli/src/lsp.rs`
  — sort document symbols and the order of published diagnostics.
- `compiler/ironplc-cli/src/cli.rs` (tests) or `compiler/ironplc-cli/tests/`
  — `echo` with several files.
- `specs/design/mcp-server.md`, `docs/reference/compiler/ironplcc.rst`.

## Tests

Each test asserts the exact order, and uses enough items (16 or more files
or declarations, declared in an order that is neither alphabetical nor
reverse alphabetical) that a hash order would match it only by chance. Each
is seen failing before its fix.

## Tasks

Single core change PR (no prefactor).

- [ ] `SourceProject`: test `sources_when_many_files_added_then_returned_in_insertion_order`
      (and `sources_mut`, re-adding a file keeps its place, removing one keeps
      the rest in order); switch to `IndexMap`.
- [ ] CLI: test that `echo` on several files prints them in argument order.
- [ ] Extractors: tests that `user_defined_functions` / `user_defined_types`
      return declarations in source order, across two files too; sort by span.
- [ ] MCP `symbols`: test for REQ-TOL-mcp-056; add the requirement.
- [ ] LSP: test that `document_symbols` returns types, function blocks and
      functions interleaved in source order; sort by range start.
- [ ] LSP: test that workspace diagnostics are published in URI order; sort.
- [ ] Docs: `ironplcc echo`/`tokenize` file order.
- [ ] `cd compiler && just`, `cd specs && just`, docs build.
