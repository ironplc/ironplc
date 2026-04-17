# Plan: MCP `compile` and `container_drop` Tools

## Context

The MCP server design (`specs/design/mcp-server.md`) defines two milestones. Milestone 1 (validation surface: `parse`, `check`, `format`, `symbols`, etc.) is in progress — `list_options` is implemented, and `parse`/`check` are planned (`specs/plans/2026-04-12-mcp-parse-check-tools.md`). This plan implements the first Milestone 2 tools: `compile` and `container_drop`, along with the container cache that bridges them.

The `compile` tool is the gateway to VM execution. It runs the full pipeline (parse → semantic analysis → codegen), caches the resulting `.iplc` bytes, and returns an opaque `container_id` that the agent later passes to `run`. The `container_drop` tool provides explicit cache eviction for long-running connections.

## Design doc reference

- `specs/design/mcp-server.md` — requirements REQ-TOL-030..036 (compile), REQ-TOL-150..151 (container_drop), REQ-ARC-070..073 (container cache)
- `specs/design/spec-conformance-testing.md` — `#[spec_test]` / `build.rs` enforcement mechanism

## Architecture

### Approach

The `compile` tool reuses the shared infrastructure from the parse/check plan (`tools/common.rs` — `SourceInput`, `validate_sources()`, `parse_options()`, `map_diagnostic()`, `McpDiagnostic`). The pipeline is:

1. Validate `sources` and `options` (shared infra)
2. Build `MemoryBackedProject`, add sources, call `semantic()`
3. Get the analyzed `Library` and `SemanticContext` from the project
4. Call `ironplc_codegen::compile()` to produce a `Container`
5. Serialize the Container to `.iplc` bytes
6. Cache the bytes + task/program metadata in the process-level container cache
7. Return `container_id`, `tasks`, `programs`, `diagnostics`

### Key design decision: Exposing the analyzed Library

The codegen API requires both the analyzed `Library` and the `SemanticContext`:
```rust
pub fn compile(library: &Library, context: &SemanticContext, options: &CodegenOptions) -> Result<Container, Diagnostic>
```

Currently, `MemoryBackedProject::semantic()` discards the analyzed Library (the `_library` in `run_semantic_analysis`'s `Ok((_library, context))` arm). Only the `SemanticContext` is cached.

**Solution:** Extend `MemoryBackedProject` to store the analyzed `Library` alongside the `SemanticContext`, and expose it via an `analyzed_library()` accessor on the `Project` trait. This is a small change to `compiler/project/src/project.rs` and keeps the dependency graph clean — the MCP crate continues to access analysis results through `ironplc-project`.

### Container cache

Hand-rolled LRU cache using `HashMap` + `VecDeque` (no external crate). Dual-bounded by entry count (default 64) and total bytes (default 64 MiB). Thread-safe via `Arc<std::sync::Mutex<...>>` since `rmcp::ServerHandler` requires `Send + Sync`.

Container IDs use a monotonic counter: `c_0`, `c_1`, etc. — opaque, unique within the process, trivially simple.

### Task/program metadata extraction

Metadata is extracted from the `Library`'s `ConfigurationDeclaration` AST (not the low-level container TaskTable):
- `TaskConfiguration` with `interval: Some(...)` → kind `"cyclic"`, `interval_ms` from duration
- `TaskConfiguration` with `single: Some(...)` → kind `"single"`, `interval_ms: null`
- `TaskConfiguration` with neither → kind `"event"`, `interval_ms: null`
- When no `ConfigurationDeclaration` exists, synthesize a default entry matching the builder's freewheeling task behavior

### New crate dependencies

`compiler/mcp/Cargo.toml` gains (on top of what parse/check plan adds):
- `ironplc-codegen = { path = "../codegen", version = "0.192.0" }` — codegen `compile()` function
- `ironplc-container = { path = "../container", version = "0.192.0" }` — `Container::write_to()` for serialization
- `ironplc-analyzer = { path = "../analyzer", version = "0.192.0" }` — `SemanticContext` type needed as parameter to `ironplc_codegen::compile()`. The parse/check plan avoids this, but it is required at the Milestone 2 boundary.
- `base64 = "0.22"` — for `container_base64` response field (already used in `compiler/playground/`)

## File map

| File | Action | Purpose |
|------|--------|---------|
| `compiler/project/src/project.rs` | Modify | Store analyzed `Library` in `MemoryBackedProject`, add `analyzed_library()` to `Project` trait |
| `compiler/mcp/Cargo.toml` | Modify | Add `ironplc-codegen`, `ironplc-container`, `ironplc-analyzer`, `base64` deps |
| `compiler/mcp/src/lib.rs` | Modify | Add `pub mod cache;` |
| `compiler/mcp/src/cache.rs` | New | `ContainerCache` with LRU eviction, `CachedContainer`, `TaskMeta`, `ProgramMeta` |
| `compiler/mcp/src/tools/mod.rs` | Modify | Add `pub mod compile;`, `pub mod container_drop;` |
| `compiler/mcp/src/tools/compile.rs` | New | `CompileResponse`, `CompileInput`, `build_response()`, task/program metadata extraction |
| `compiler/mcp/src/tools/container_drop.rs` | New | `ContainerDropResponse`, `ContainerDropInput`, `build_response()` |
| `compiler/mcp/src/server.rs` | Modify | Add `Arc<Mutex<ContainerCache>>` to `IronPlcMcp`, add `#[tool]` methods for `compile` and `container_drop` |
| `compiler/mcp/src/spec_conformance.rs` | Modify | Convert `#[ignore]` stubs to real tests for REQ-TOL-030..036, REQ-TOL-150..151, REQ-ARC-070..073 |

## Tasks

### Step 1: Extend `MemoryBackedProject` to expose analyzed Library

File: `compiler/project/src/project.rs`

- [ ] Change `run_semantic_analysis()` return type from `(Result<(), Vec<Diagnostic>>, Option<SemanticContext>)` to `(Result<(), Vec<Diagnostic>>, Option<SemanticContext>, Option<Library>)`. In the `Ok` arm, return the library as `Some(library)` instead of discarding `_library`. In the `Err` arm, return `None`.
- [ ] Add `analyzed_library: Option<Library>` field to both `FileBackedProject` and `MemoryBackedProject`. Initialize to `None` in constructors.
- [ ] Add `fn analyzed_library(&self) -> Option<&Library>` to the `Project` trait.
- [ ] In each `Project::semantic()` impl, clear `self.analyzed_library = None` at start, destructure the new 3-tuple, and store the library.
- [ ] Implement `analyzed_library()` for both project types: `self.analyzed_library.as_ref()`.
- [ ] Add tests:
  - `memory_semantic_when_valid_program_then_analyzed_library_available`
  - `memory_semantic_when_syntax_error_then_analyzed_library_none`
  - `memory_semantic_when_semantic_error_then_analyzed_library_available` (the `Ok` path with context diagnostics)
  - `memory_analyzed_library_when_no_analysis_then_none`

### Step 2: Implement container cache

New file: `compiler/mcp/src/cache.rs`

- [ ] Define `CachedContainer`:
  ```
  iplc_bytes: Vec<u8>
  tasks: Vec<TaskMeta>
  programs: Vec<ProgramMeta>
  byte_size: usize   // len of iplc_bytes, cached for bookkeeping
  ```
- [ ] Define `TaskMeta` with `name: String`, `priority: u32`, `kind: String`, `interval_ms: Option<f64>`
- [ ] Define `ProgramMeta` with `name: String`, `task: Option<String>`
- [ ] Define `ContainerCache`:
  ```
  entries: HashMap<String, CachedContainer>
  lru_order: VecDeque<String>
  total_bytes: usize
  next_id: u64
  max_entries: usize   // default 64
  max_bytes: usize     // default 64 MiB
  ```
- [ ] Implement `ContainerCache::new(max_entries, max_bytes)`
- [ ] Implement `insert(&mut self, container: CachedContainer) -> Result<String, InsertError>`:
  - If `container.byte_size > self.max_bytes`, return `InsertError::TooLarge`
  - Evict LRU entries (pop front of `lru_order`, remove from `entries`, subtract from `total_bytes`) until the new entry fits within both count and byte bounds
  - Generate ID `c_{next_id}`, increment counter, insert entry, push ID to back of `lru_order`
- [ ] Implement `get(&mut self, id: &str) -> Option<&CachedContainer>` — touches LRU (find in VecDeque, move to back)
- [ ] Implement `remove(&mut self, id: &str) -> bool` — removes from both `entries` and `lru_order`
- [ ] Write tests:
  - `insert_when_within_limits_then_returns_id`
  - `insert_when_entry_count_at_max_then_evicts_oldest`
  - `insert_when_byte_budget_exceeded_then_evicts_oldest`
  - `insert_when_single_entry_exceeds_budget_then_error`
  - `get_when_existing_then_returns_entry`
  - `get_when_missing_then_returns_none`
  - `get_when_accessed_then_updates_lru_order`
  - `remove_when_existing_then_returns_true_and_frees_bytes`
  - `remove_when_missing_then_returns_false`
  - `eviction_order_when_accessed_then_lru_preserved`

### Step 3: Add crate dependencies

File: `compiler/mcp/Cargo.toml`

- [ ] Add `ironplc-codegen`, `ironplc-container`, `ironplc-analyzer`, `base64` dependencies
- [ ] Add `pub mod cache;` to `compiler/mcp/src/lib.rs`
- [ ] Add `pub mod compile;` and `pub mod container_drop;` to `compiler/mcp/src/tools/mod.rs`
- [ ] Verify `cargo check -p ironplc-mcp` compiles

### Step 4: Implement `compile` tool

New file: `compiler/mcp/src/tools/compile.rs`

- [ ] Define input type:
  ```rust
  #[derive(Debug, Deserialize, JsonSchema)]
  pub struct CompileInput {
      sources: Vec<SourceInput>,
      options: serde_json::Value,
      #[serde(default)]
      include_bytes: bool,
  }
  ```
- [ ] Define response types: `CompileResponse`, `TaskInfo`, `ProgramInfo`
  - `CompileResponse { ok, container_id: Option<String>, container_base64: Option<String>, tasks: Vec<TaskInfo>, programs: Vec<ProgramInfo>, diagnostics: Vec<McpDiagnostic> }`
  - `TaskInfo { name, priority: u32, kind: String, interval_ms: Option<f64> }`
  - `ProgramInfo { name, task: Option<String> }`
- [ ] Implement `build_response(sources, options_value, include_bytes, cache: &Mutex<ContainerCache>)`:
  1. Validate sources (`validate_sources`), return early on failure
  2. Parse options (`parse_options`), return early on failure
  3. Construct `MemoryBackedProject::new(compiler_options)`, load sources
  4. Build source content map for diagnostic mapping
  5. Call `project.semantic()` — collect diagnostics on `Err`
  6. If error-severity diagnostics or no analyzed library available, return `ok: false`
  7. Get `analyzed_library()` and `semantic_context()` from project
  8. Build `CodegenOptions { system_uptime_global: compiler_options.allow_system_uptime_global }`
  9. Call `ironplc_codegen::compile(library, context, &codegen_options)` — map diagnostic on `Err`
  10. Serialize container: `container.write_to(&mut bytes)`
  11. Extract task/program metadata from the Library's `ConfigurationDeclaration`
  12. Build `CachedContainer`, insert into cache. On `TooLarge`, return `ok: false` with diagnostic
  13. If `include_bytes`, base64-encode the bytes
  14. Return `CompileResponse { ok: true, container_id: Some(id), ... }`
- [ ] Implement `extract_task_program_metadata(library: &Library)`:
  - Find `ConfigurationDeclaration` in library elements
  - Walk `resource_decl` → `tasks` → `TaskConfiguration` for task metadata
  - Walk `resource_decl` → `programs` → `ProgramConfiguration` for program metadata
  - When no configuration exists, synthesize a default from the first `ProgramDeclaration`
- [ ] Write tests:
  - `build_response_when_valid_program_then_ok_true`
  - `build_response_when_syntax_error_then_ok_false`
  - `build_response_when_semantic_error_then_ok_false`
  - `build_response_when_valid_then_container_id_present`
  - `build_response_when_valid_then_tasks_populated`
  - `build_response_when_valid_then_programs_populated`
  - `build_response_when_include_bytes_true_then_base64_present`
  - `build_response_when_include_bytes_false_then_base64_null`
  - `build_response_when_no_configuration_then_default_task`
  - `build_response_when_codegen_error_then_ok_false`
  - `build_response_when_invalid_sources_then_error_diagnostic`
  - `build_response_when_invalid_options_then_error_diagnostic`

### Step 5: Implement `container_drop` tool

New file: `compiler/mcp/src/tools/container_drop.rs`

- [ ] Define `ContainerDropInput { container_id: String }`
- [ ] Define `ContainerDropResponse { ok: bool, removed: bool, diagnostics: Vec<McpDiagnostic> }`
- [ ] Implement `build_response(container_id, cache)`:
  - Lock cache, call `cache.remove(container_id)`
  - If removed: `{ ok: true, removed: true, diagnostics: [] }`
  - If not found: `{ ok: false, removed: false, diagnostics: [unknown container diagnostic] }`
- [ ] Write tests:
  - `build_response_when_existing_container_then_removed`
  - `build_response_when_unknown_container_then_not_removed`

### Step 6: Wire tools into the MCP server

File: `compiler/mcp/src/server.rs`

- [ ] Add `Arc<Mutex<ContainerCache>>` field to `IronPlcMcp`
- [ ] Initialize cache in `Default`/`new()` with defaults (64 entries, 64 MiB)
- [ ] Add `#[tool]` method for `compile` with REQ-ARC-050 description: "Only call this when you need a compiled artifact to `run`. For validation, call `check` instead — `check` is faster, produces the same diagnostics, and does not incur codegen cost. A failing `compile` does not give you any information that a failing `check` would not."
- [ ] Add `#[tool]` method for `container_drop` with REQ-ARC-050 description: "Explicitly releases a compiled container from the cache. Not usually necessary — the cache evicts on LRU pressure — but available for long-running connections."
- [ ] Both tool methods follow existing pattern: deserialize input, call `build_response`, serialize to `Content::text(json)`. Never return MCP-level errors for compiler failures.

### Step 7: Spec conformance tests

File: `compiler/mcp/src/spec_conformance.rs`

Convert the following `#[ignore]` stubs (created by the parse/check plan) into real `#[spec_test]` tests:

| Requirement | What to test |
|---|---|
| `REQ_TOL_030` | `compile` returns non-null `container_id` on success |
| `REQ_TOL_031` | `compile` returns `ok: false`, `container_id: null`, diagnostics on failure |
| `REQ_TOL_032` | `compile` returns `tasks` array with `name`, `priority`, `kind`, `interval_ms` |
| `REQ_TOL_033` | `compile` returns `programs` array with `name` and `task` |
| `REQ_TOL_034` | `container_base64` present only when `include_bytes: true` |
| `REQ_TOL_035` | Container stored in cache (compile then verify cache has entry) |
| `REQ_TOL_036` | Cached container is immutable (same sources → different IDs, both valid) |
| `REQ_TOL_150` | `container_drop` returns `ok: true`, `removed: true` for known ID |
| `REQ_TOL_151` | `container_drop` returns `ok: false`, `removed: false` for unknown ID |
| `REQ_ARC_070` | Cache stores bytes + metadata, not sources |
| `REQ_ARC_071` | Cache evicts LRU when entry count or byte limit exceeded |
| `REQ_ARC_072` | Entries are immutable, don't expire on timer |
| `REQ_ARC_073` | Unknown container_id → `ok: false` with diagnostic |

### Step 8: Run full CI

- [ ] Run `cd compiler && just` to verify compile, tests, coverage, clippy, fmt
- [ ] Fix any issues and re-run until clean

## Verification

1. `cargo build -p ironplc-mcp` succeeds with zero warnings
2. `cargo test -p ironplc-project` — new `analyzed_library` tests pass
3. `cargo test -p ironplc-mcp` — all tests pass: cache unit tests, compile tool tests, container_drop tests, and spec conformance tests
4. `cd compiler && just` — full CI passes
5. Manual smoke test: pipe MCP `tools/list` into `ironplcmcp`, verify `compile` and `container_drop` appear; call `compile` with a valid program, verify `container_id` is returned; call `container_drop` with the returned ID, verify `removed: true`
