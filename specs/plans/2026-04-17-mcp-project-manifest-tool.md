# Plan: MCP `project_manifest` Tool

## Context

The MCP server (tracked by `specs/plans/2026-04-14-mcp-server-plan.md`) has completed phases 1–6 (`bootstrap`, `list_options`, `parse`, `check`, `explain_diagnostic`, `symbols`) and phases 8–9 (`compile`, `container_drop`). Phase 7 in the parent plan groups five context tools together; this plan implements **only `project_manifest`** to keep the PR small. The remaining four context tools (`project_io`, `pou_scope`, `pou_lineage`, `types_all`) will follow in separate PRs.

`project_manifest` returns a flat summary of what is declared across the supplied sources: file names, Program / Function / Function Block names, and user-defined types grouped by kind (`enumerations`, `structures`, `arrays`, `subranges`, `aliases`, `strings`, `references`). Design doc: `specs/design/mcp-server.md` lines 270–299, requirements REQ-TOL-200..201.

## Approach

Mirror the established pattern in `compiler/mcp/src/tools/check.rs` and `symbols.rs`:

- Free function `build_response(sources, options) -> ProjectManifestResponse`.
- Reuse `common::{validate_sources, parse_options, serialize_diagnostics, SourceInput}` and the shared `ParseCheckInput` envelope for the server handler.
- Build a fresh `MemoryBackedProject`, call `semantic()`, and populate the response from `SemanticContext`.
- Unit tests colocated; subprocess tests in `tests/cli.rs` for success / semantic-error / validation paths.

### Files

| File | Action |
|------|--------|
| `compiler/mcp/src/tools/project_manifest.rs` | **Create** — `build_response()` + unit tests |
| `compiler/mcp/src/tools/mod.rs` | Modify — add `pub mod project_manifest;` |
| `compiler/mcp/src/server.rs` | Modify — register `#[tool]` handler |
| `compiler/mcp/tests/cli.rs` | Modify — add subprocess tests |

### Response shape

```
{ ok,
  files: [],
  programs: [], functions: [], function_blocks: [],
  enumerations: [], structures: [], arrays: [], subranges: [],
  aliases: [], strings: [], references: [],
  diagnostics: [] }
```

### Data sources (all on `SemanticContext`)

- `files`: pulled from the `sources` input — each `SourceInput.name`
- `programs`: `context.symbols().get_programs()`
- `function_blocks`: `context.symbols().get_function_blocks()`
- `functions`: `context.functions().iter_user_defined()`
- Type buckets: iterate `context.types().iter_user_defined()` and bucket by `IntermediateType` variant:
  - `Enumeration` → `enumerations`
  - `Structure` → `structures`
  - `Array` → `arrays`
  - `Subrange` → `subranges`
  - `String` → `strings`
  - `Reference` → `references`
  - anything else (primitive wrappers, function/function-block types) → `aliases`

### REQ-TOL-201: partial manifest on semantic failure

Unlike `symbols.rs`, which bails on error, `project_manifest` must return whatever the analyzer recognized even when analysis fails. `MemoryBackedProject::semantic_context()` returns `Option<&SemanticContext>` and is populated even after an `Err` result (see `compiler/project/src/project.rs:290-302`). Approach:

1. Run `project.semantic()`; collect any diagnostics.
2. Always try `project.semantic_context()` afterwards; if `Some`, populate all fields; if `None`, just populate `files` and the diagnostics.
3. Set `ok: true` iff there are no error-severity diagnostics.

### Sorting

Emit each array in lexicographic order for deterministic output. `symbols.rs` does not sort, but `project_manifest` is pure-projection and sorting stabilizes both unit tests and client-side diffing.

## Verification

Unit tests in `project_manifest.rs`:
- valid source with one program / one FB / one function / one enum / one struct → correct buckets populated, `ok: true`
- multi-file sources → `files` lists both names
- semantic error with recoverable parse → partial manifest + `ok: false` + diagnostics (REQ-TOL-201)
- empty source name → `P8001` diagnostic, `ok: false`
- missing dialect → `P8001` diagnostic, `ok: false`

Subprocess tests in `tests/cli.rs` mirroring `symbols` coverage: success, semantic-error, validation paths.

Project-wide:
- `cd compiler && just` passes (compile, coverage ≥ 85 %, clippy). User directive: skip `just format`.
