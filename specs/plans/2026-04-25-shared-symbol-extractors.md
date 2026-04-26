# Plan: Share Symbol Extraction Between MCP and LSP

## Goal

Eliminate the duplicated symbol traversal logic that exists in both the MCP `symbols` tool and the LSP `document_symbols` provider. Both servers walk the same `SemanticContext` (programs, function blocks, functions, types, variables) with the same iterate → filter (skip builtin/stdlib) → project pattern, but each side reimplements the traversal inline before mapping to its protocol-specific shape. Move the shared traversal into a single neutral module in `ironplc-analyzer`, leaving each server with only its protocol mapping.

## Background

Today the two servers share most of the underlying compiler crates (`analyzer`, `parser`, `dsl`, `project`, `problems`) and use the same `Project::semantic()` entry point. Diagnostics are already a thin protocol-conversion layer, and project/cache management serve different concerns (live editing vs. compiled-bytecode handoff), so neither is duplication. The one true overlap is symbol extraction:

- **MCP** — `compiler/mcp/src/tools/symbols.rs:156-280` defines `extract_programs`, `extract_functions`, `extract_function_blocks`, `extract_types`, and `extract_variables_from_scope`. It builds MCP JSON structs (`ProgramSymbol`, `FunctionSymbol`, `FunctionBlockSymbol`, `TypeSymbol`, `VariableInfo`) by mapping over `context.symbols().get_programs()`, `context.functions().iter_user_defined()`, `context.types().iter_user_defined()`, etc.
- **LSP** — `compiler/ironplc-cli/src/lsp_project.rs:138-228` (`document_symbols`) inlines the same traversal: `context.types().iter()` + `is_builtin()`/`Elementary` skip and `context.functions().iter()` + `is_stdlib()` skip — emitting `lsp_types::DocumentSymbol`.

The traversals are nearly identical except (a) MCP uses `iter_user_defined()` while LSP filters `iter()` manually, and (b) LSP filters by `file_id` while MCP returns everything. LSP's manual filter has a latent bug: `IntermediateType::FunctionBlock` and `IntermediateType::Function` survive the filter and appear in the type list (as `CLASS`/`FUNCTION`) in addition to being emitted from `functions().iter()`, producing duplicates in the document outline. Switching LSP to the shared extractor (which uses `iter_user_defined()`) fixes this incidentally.

## Architecture

Add a new public module `extractors` to `ironplc-analyzer` that exposes neutral, borrow-based views over a `SemanticContext`. The extractors keep the data the protocol-specific callers need (name, span, kind, variables, parameters, return type) without leaking either protocol's types. Each caller reduces to: call extractor → map to its protocol shape.

Key design choices:

1. **Borrow-based**, not owned — the extractors return `Vec<TypeSymbolView<'a>>` etc. holding references to the underlying `SymbolInfo`/`FunctionSignature`/`TypeAttributes`. This avoids any allocation churn for LSP's per-keystroke document symbol calls and keeps full fidelity for MCP's JSON serialization.
2. **No filtering by file_id in the extractors** — file filtering is an LSP concern (workspace-wide types vs. one document). Extractors return the full set; callers filter by `span.file_id` when they need to.
3. **Variable direction normalization is shared** — the `VariableType` → `"In"/"Out"/"InOut"/"Local"/"Global"/"External"` mapping currently in `tools/symbols.rs:256-268` becomes a `VariableDirection` enum + helper in the extractors module. MCP serializes the discriminant string; LSP can ignore it since `document_symbols` doesn't currently surface variables (but will get the helper for free if it adds VAR outline children later).
4. **Type kind classification is shared** — the per-`IntermediateType` match (currently duplicated in `tools/symbols.rs:231-238` as strings and `lsp_project.rs:273-288` as `lsp_types::SymbolKind`) becomes a single `TypeSymbolKind` enum + classifier. Each caller maps the enum to its target.

Since `ironplc-analyzer` has no `lsp-types` or `serde_json` dependency (and shouldn't), the protocol mapping stays in each server's crate.

## File Map

| File | Change |
|------|--------|
| `compiler/analyzer/src/extractors.rs` | **New.** Public `extractors` module with `ProgramSymbol`, `FunctionBlockSymbol`, `FunctionSymbolView`, `TypeSymbolView`, `VariableSymbol`, `VariableDirection`, `TypeSymbolKind`, plus `extract_programs`, `extract_function_blocks`, `extract_user_defined_functions`, `extract_user_defined_types`, `extract_variables_in_scope`, `classify_type_kind`, `normalize_variable_direction`. |
| `compiler/analyzer/src/lib.rs` | Declare `pub mod extractors;` and re-export the public types. |
| `compiler/mcp/src/tools/symbols.rs` | Replace inline `extract_programs`/`extract_functions`/`extract_function_blocks`/`extract_types`/`extract_variables_from_scope` with calls to the shared extractors. Convert each view to the existing MCP JSON struct (`ProgramSymbol`, `FunctionSymbol`, `TypeSymbol`, `VariableInfo`). Existing tests remain unchanged and continue to pass. |
| `compiler/ironplc-cli/src/lsp_project.rs` | Replace the inline loops in `document_symbols` with calls to `extract_user_defined_types` and `extract_user_defined_functions`, filter by `file_id`, and map to `lsp_types::DocumentSymbol`. Keep `intermediate_type_to_symbol_kind` only as the local `TypeSymbolKind` → `lsp_types::SymbolKind` adapter (or replace it in favor of mapping `TypeSymbolKind` directly). |
| `compiler/analyzer/src/extractors.rs` (tests) | Unit tests for each extractor: programs, FBs, functions, types, variable direction normalization, type kind classification. BDD-style names per project convention. |

No public API of `SemanticContext`, `SymbolEnvironment`, `FunctionEnvironment`, or `TypeEnvironment` changes. No new crate is added.

## Behavior Changes

- **LSP**: Function-block types and function types stop appearing as duplicate `CLASS`/`FUNCTION` entries in document symbols (they were already emitted via `functions().iter()`). This is a bug fix that falls out of using `iter_user_defined()` consistently.
- **MCP**: No user-visible change. Same JSON shape, same filtering, same tests pass.

## Tasks

- [x] Write plan
- [ ] Add `extractors` module with neutral views, `VariableDirection`, `TypeSymbolKind`, and extractor functions
- [ ] Add unit tests for the extractors (programs/FBs/functions/types, direction normalization, type kind classification)
- [ ] Refactor `compiler/mcp/src/tools/symbols.rs` to consume the shared extractors; delete the local `extract_*` functions
- [ ] Refactor `compiler/ironplc-cli/src/lsp_project.rs::document_symbols` to consume the shared extractors and map to `lsp_types::DocumentSymbol`; remove the inline duplicate-type-listing
- [ ] Run full CI pipeline (`cd compiler && just`) and confirm coverage stays >= 85%
- [ ] Commit and push to `claude/reduce-mcp-lsp-duplication-rACgc`

## Out of Scope

- Sharing `parse_options()` between MCP and LSP (smaller, separate cleanup).
- Adding LSP equivalents for MCP-only tools (`pou_scope`, `pou_lineage`, `types_all`, `parse`).
- Refactoring diagnostic conversion (already minimal and protocol-specific).
- Refactoring `cache.rs` / `lsp_project.rs` project-management code (different concerns, not duplication).
