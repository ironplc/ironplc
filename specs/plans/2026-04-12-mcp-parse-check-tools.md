# Plan: MCP `parse` and `check` Tools

## Goal

Implement the `parse` and `check` MCP tools from the MCP server design (`specs/design/mcp-server.md`). These are the first tools that accept `sources` and `options` parameters, so this plan also delivers the shared infrastructure (input validation, options parsing, diagnostic mapping) that all future source-accepting tools will reuse.

## Design doc reference

`specs/design/mcp-server.md` — requirements REQ-STL-001 through REQ-STL-006, REQ-TOL-010 through REQ-TOL-026, REQ-ARC-010, REQ-ARC-011, REQ-ARC-050.

## Architecture

### Approach

Both tools use `MemoryBackedProject` from `ironplc-project` — the same `Project` trait abstraction that the CLI uses via `FileBackedProject`. The MCP server constructs a fresh `MemoryBackedProject` per tool call, loads the `sources` array via `add_source()`, runs the appropriate pipeline method, and discards the project when the handler returns.

- **`parse`**: constructs a `MemoryBackedProject`, adds sources, calls `sources_mut()` to parse each source via `library()`, collects diagnostics and extracts structure from the parsed `Library`.
- **`check`**: constructs a `MemoryBackedProject`, adds sources, calls `semantic()` which runs parse + full semantic analysis. Collects all diagnostics.

This matches the design's intent (REQ-ARC-010: construct a fresh in-memory project per call, discard it when done) and satisfies REQ-STL-006 (no disk I/O) since `MemoryBackedProject` never touches the filesystem.

### Shared infrastructure (`tools/common.rs`)

A new module provides types and helpers reused by every source-accepting tool:

1. **`SourceInput`** — `{ name: String, content: String }`, serde-deserializable. Represents one entry in the `sources` array.
2. **`validate_sources()`** — enforces REQ-STL-004: names must be non-empty, at most 256 bytes, no NUL/`/`/`\`, no duplicates. Returns a `Vec<McpDiagnostic>` on validation failure.
3. **`parse_options()`** — accepts a `serde_json::Value`, extracts `dialect`, validates it against `Dialect::ALL`, extracts and validates feature flag overrides against `CompilerOptions::FEATURE_DESCRIPTORS`, rejects unknown keys (REQ-TOL-025). Returns `Result<CompilerOptions, Vec<McpDiagnostic>>`.
4. **`McpDiagnostic`** — the JSON-serializable diagnostic type with `code`, `message`, `file`, `start_line`, `start_col`, `end_line`, `end_col`, `severity`. Satisfies REQ-TOL-023.
5. **`map_diagnostic()`** — converts `ironplc_dsl::diagnostic::Diagnostic` into `McpDiagnostic` using the source text to convert byte offsets to 1-indexed line/column numbers counting Unicode scalar values. Mirrors the existing LSP conversion in `lsp_project.rs:569-608` but produces 1-indexed values per REQ-TOL-023.
6. **`map_diagnostics()`** — batch conversion that takes a `&[Diagnostic]` and a lookup map from `FileId` to source content.

### Diagnostic mapping detail

The `map_diagnostic` function receives the source text for the file referenced by the diagnostic's `FileId`. It walks the source text up to the byte offset, counting newlines and Unicode scalar values (not bytes). Line and column numbers are 1-indexed. A tab counts as one column. `end_line`/`end_col` point to the character immediately after the span. When the file ID cannot be found (should not happen in normal operation), the function falls back to line 1, column 1.

### `parse` tool

The handler:
1. Deserializes `sources` and `options` from the MCP call arguments.
2. Validates sources (`validate_sources`).
3. Parses options (`parse_options`).
4. For each source: constructs a `FileId::from_string(name)` and calls `parse_program(content, &file_id, &options)`.
5. On success: walks the `Library.elements` to build the `structure` array. On failure: records the diagnostic and contributes no structure entries for that source.
6. Converts all diagnostics to `McpDiagnostic` via `map_diagnostics`.
7. Returns `ParseResponse { ok, structure, diagnostics }`.

The `structure` array entries have: `kind` (one of `"program"`, `"function"`, `"function_block"`, `"type"`, `"configuration"`), `name` (string or `null`), `file`, `start_line`, `end_line`. The `start_line`/`end_line` are derived from the declaration's `SourceSpan` (for `ProgramDeclaration` via `name.span`, for `FunctionBlockDeclaration` via `span`, etc.).

### `check` tool

The handler:
1. Deserializes `sources` and `options`.
2. Validates sources and options (same as `parse`).
3. Constructs a `MemoryBackedProject` with the parsed `CompilerOptions`.
4. Loads each source via `project.add_source(FileId::from_string(name), content)`.
5. Calls `project.semantic()` — this runs parse + full semantic analysis internally.
6. On `Err`, collects all diagnostics (parse errors, type resolution failures, and semantic rule violations).
7. Converts all diagnostics to `McpDiagnostic`.
8. Returns `CheckResponse { ok, diagnostics }` where `ok` is `true` when no diagnostic has `severity: "error"`.

### New crate dependencies

`compiler/mcp/Cargo.toml` gains:
- `ironplc-project` — for `MemoryBackedProject`, `Project` trait (parse + semantic analysis)
- `ironplc-dsl` — for `Diagnostic`, `Library`, `FileId`, `LibraryElementKind`, `SourceSpan`

Both are workspace-local path dependencies, same pattern as other crates. The MCP crate does **not** depend on `ironplc-analyzer` directly; it accesses semantic analysis through `MemoryBackedProject::semantic()` from `ironplc-project`.

### Tool descriptions

Per REQ-ARC-050, the tool descriptions are the exact strings from the design doc:
- `parse`: "Syntax check only. Use while drafting to confirm the source tokenizes and parses. Do NOT use this to validate a change -- it does not catch type errors, undeclared symbols, or any other semantic rule. Call `check` for that."
- `check`: "Primary validator. Runs parse and full semantic analysis and returns structured diagnostics. ALWAYS run this before reporting success to the user and before calling `compile` or `run`. Self-heal by reading the returned diagnostics, fixing the code, and calling `check` again. Call `explain_diagnostic` to understand any unfamiliar problem code BEFORE editing the source."

### MCP tool input parameters

The `rmcp` crate uses `#[tool]` macros with typed parameters. Both tools accept a JSON object with two required fields:

```rust
#[tool(name = "parse", description = "...")]
fn parse(&self, #[tool(aggr)] input: ParseCheckInput) -> Result<Content, ErrorData>
```

Where `ParseCheckInput` is:
```rust
#[derive(Debug, Deserialize, JsonSchema)]
struct ParseCheckInput {
    sources: Vec<SourceInput>,
    options: serde_json::Value,
}
```

Using `serde_json::Value` for `options` allows us to do manual validation of keys (reject unknowns per REQ-TOL-025) rather than silently ignoring them via serde's default behavior.

## File map

| File | Action |
|------|--------|
| `compiler/mcp/Cargo.toml` | Modify: add `ironplc-project` and `ironplc-dsl` dependencies |
| `compiler/mcp/src/tools/mod.rs` | Modify: add `pub mod common;`, `pub mod parse;`, `pub mod check;` |
| `compiler/mcp/src/tools/common.rs` | New: shared types (`SourceInput`, `McpDiagnostic`, `StructureEntry`), validation (`validate_sources`, `parse_options`), diagnostic mapping (`map_diagnostic`, `map_diagnostics`) |
| `compiler/mcp/src/tools/parse.rs` | New: `ParseResponse`, `build_response()`, handler logic, structure extraction, tests |
| `compiler/mcp/src/tools/check.rs` | New: `CheckResponse`, `build_response()`, handler logic, tests |
| `compiler/mcp/src/server.rs` | Modify: add `#[tool]` methods for `parse` and `check` in the `#[tool_router]` block |

## Tasks

### Step 1: Add crate dependencies

- [ ] Add `ironplc-project` and `ironplc-dsl` to `compiler/mcp/Cargo.toml` as path dependencies with matching version
- [ ] Verify `cargo check -p ironplc-mcp` still compiles

### Step 2: Implement shared infrastructure (`tools/common.rs`)

- [ ] Define `SourceInput` struct with serde Deserialize and JsonSchema
- [ ] Define `McpDiagnostic` struct with serde Serialize (fields: `code`, `message`, `file`, `start_line`, `start_col`, `end_line`, `end_col`, `severity`)
- [ ] Define `StructureEntry` struct with serde Serialize (fields: `kind`, `name`, `file`, `start_line`, `end_line`)
- [ ] Implement `validate_sources()`: check REQ-STL-004 constraints (non-empty name, max 256 bytes, no NUL/`/`/`\`, no duplicate names)
- [ ] Implement `parse_options()`: extract `dialect` string, match against `Dialect::ALL` via `Display` format, validate feature flag keys against `FEATURE_DESCRIPTORS`, reject unknown keys, build `CompilerOptions` with dialect preset + flag overrides
- [ ] Implement `map_diagnostic()` and `map_diagnostics()`: byte-offset-to-line/col conversion using source text, 1-indexed, Unicode scalar values
- [ ] Write tests:
  - `validate_sources_when_empty_name_then_error`
  - `validate_sources_when_name_too_long_then_error`
  - `validate_sources_when_name_contains_slash_then_error`
  - `validate_sources_when_name_contains_backslash_then_error`
  - `validate_sources_when_name_contains_nul_then_error`
  - `validate_sources_when_duplicate_names_then_error`
  - `validate_sources_when_valid_then_ok`
  - `parse_options_when_missing_dialect_then_error`
  - `parse_options_when_unknown_dialect_then_error`
  - `parse_options_when_unknown_key_then_error`
  - `parse_options_when_ed2_dialect_then_default_options`
  - `parse_options_when_ed3_dialect_then_edition3_enabled`
  - `parse_options_when_rusty_dialect_then_vendor_flags_enabled`
  - `parse_options_when_flag_override_then_applied`
  - `map_diagnostic_when_single_line_then_correct_line_col`
  - `map_diagnostic_when_multi_line_then_correct_line_col`
  - `map_diagnostic_when_unicode_then_counts_scalar_values`
  - `map_diagnostic_when_tab_then_counts_as_one_column`

### Step 3: Implement `parse` tool (`tools/parse.rs`)

- [ ] Define `ParseResponse { ok: bool, structure: Vec<StructureEntry>, diagnostics: Vec<McpDiagnostic> }` with Serialize
- [ ] Implement `build_response(sources, options_value)` function:
  - Validate sources and options (return early with error diagnostics if invalid)
  - For each source: call `parse_program()`, extract structure on success, collect diagnostics on failure
  - Build structure entries from `LibraryElementKind` variants (program, function, function_block, type, configuration)
  - Map all diagnostics to `McpDiagnostic`
  - Set `ok` based on whether any diagnostic has `severity: "error"`
- [ ] Write tests:
  - `build_response_when_valid_program_then_ok_true`
  - `build_response_when_syntax_error_then_ok_false_with_diagnostics`
  - `build_response_when_valid_program_then_structure_has_program`
  - `build_response_when_function_and_program_then_structure_has_both`
  - `build_response_when_function_block_then_structure_has_fb`
  - `build_response_when_type_declaration_then_structure_has_type`
  - `build_response_when_invalid_sources_then_error_diagnostic`
  - `build_response_when_invalid_options_then_error_diagnostic`
  - `build_response_when_multiple_sources_then_all_parsed`

### Step 4: Implement `check` tool (`tools/check.rs`)

- [ ] Define `CheckResponse { ok: bool, diagnostics: Vec<McpDiagnostic> }` with Serialize
- [ ] Implement `build_response(sources, options_value)` function:
  - Validate sources and options
  - Construct `MemoryBackedProject`, load sources via `add_source()`
  - Call `project.semantic()` to run parse + full semantic analysis
  - Collect diagnostics from the `Err` result
  - Map all diagnostics to `McpDiagnostic`
  - Set `ok` based on whether any diagnostic has `severity: "error"`
- [ ] Write tests:
  - `build_response_when_valid_program_then_ok_true`
  - `build_response_when_syntax_error_then_ok_false`
  - `build_response_when_semantic_error_then_ok_false`
  - `build_response_when_undeclared_variable_then_diagnostic`
  - `build_response_when_type_error_then_diagnostic`
  - `build_response_when_invalid_sources_then_error_diagnostic`
  - `build_response_when_invalid_options_then_error_diagnostic`
  - `build_response_when_multiple_valid_sources_then_ok_true`
  - `build_response_when_parse_error_in_one_source_then_still_reports`

### Step 5: Wire tools into the MCP server (`server.rs`)

- [ ] Add `pub mod common;`, `pub mod parse;`, `pub mod check;` to `tools/mod.rs`
- [ ] Add `#[tool]` method for `parse` in the `#[tool_router]` block in `server.rs` with the description from REQ-ARC-050
- [ ] Add `#[tool]` method for `check` in the `#[tool_router]` block with the description from REQ-ARC-050
- [ ] Both tool methods: deserialize input, call `build_response`, serialize result to JSON `Content::text`
- [ ] Tool methods must never return MCP-level errors for compiler failures (REQ-TOL-024); only return `Err(ErrorData)` for truly internal errors (serialization failure)

### Step 6: Run full CI

- [ ] Run `cd compiler && just` to verify compile, tests, coverage, clippy, fmt all pass
- [ ] Fix any issues and re-run until clean

## Verification

1. `cargo build -p ironplc-mcp` succeeds with zero warnings
2. `cargo test -p ironplc-mcp` — all unit tests pass (list_options + common + parse + check)
3. `cd compiler && just` — full CI passes
4. Manual smoke test: pipe MCP `tools/list` request into `ironplcmcp` on stdin, verify `parse` and `check` appear alongside `list_options`; call `parse` with a valid program and verify structured response; call `check` with a semantic error and verify diagnostic with line/col info
