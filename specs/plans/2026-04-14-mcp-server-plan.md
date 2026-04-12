# MCP Server Implementation Plan



## Context



IronPLC needs an MCP (Model Context Protocol) server so AI assistants can drive the compiler as a stateless tool service: validate, format, compile, and simulate IEC 61131-3 programs without touching the filesystem. The design is fully specified in `specs/design/mcp-server.md` with numbered requirements (REQ-STL-001..006, REQ-TOL-010..240, REQ-ARC-001..073).



A placeholder crate already exists at `compiler/mcp/` (binary `ironplcmcp`, depends only on `ironplc-project`). All underlying compiler capabilities exist — the work is wiring them into an MCP tool surface using the `rmcp` SDK.



**Design doc reference:** `specs/design/mcp-server.md`



## Architecture



- **Transport:** stdio JSON-RPC via `rmcp` (official Rust MCP SDK), async with `tokio`

- **Stateless:** every tool call takes `sources: [{name, content}]` + `options: {dialect, ...flags}`, constructs a fresh in-memory `FileBackedProject` (using only `change_text_document`, never `push`/disk IO), runs the tool, discards the project

- **One exception:** process-level container cache for `compile`→`run` handoff (LRU, bounded by count + bytes)

- **Logging:** structured JSON to stderr, per-connection IDs



## Errata



The design doc references problem-code docs at `docs/compiler/problems/P####.rst` but they actually live at `docs/reference/compiler/problems/P####.rst`. Flag this when implementing `explain_diagnostic`.



## File Map



Files to create or modify:



| File | Action |

|------|--------|

| `compiler/mcp/Cargo.toml` | Add deps: `rmcp`, `tokio`, `serde`, `serde_json`, `clap`, `uuid`, `log`, `tracing`/`tracing-subscriber`, compiler crate deps |

| `compiler/mcp/src/main.rs` | Replace placeholder with real MCP server entry point |

| `compiler/mcp/src/lib.rs` | Create — tool handlers, shared infrastructure |

| `compiler/mcp/src/options.rs` | Create — `validate_options()` helper, options→CompilerOptions conversion |

| `compiler/mcp/src/diagnostics.rs` | Create — byte-offset→line/col conversion, Diagnostic→JSON mapping |

| `compiler/mcp/src/sources.rs` | Create — REQ-STL-004 name validation, per-call Project construction |

| `compiler/mcp/src/tools/*.rs` | Create — one module per tool or tool group |

| `compiler/mcp/src/cache.rs` | Create — container cache (phase 8) |

| `compiler/mcp/src/runner.rs` | Create — VM execution logic (phase 10-11) |

| `specs/plans/2026-04-12-mcp-server-implementation.md` | Create — committed copy of this plan |



## Phases



### Phase 1: Bootstrap + Scaffolding



**Goal:** Runnable MCP binary that speaks stdio JSON-RPC with `rmcp`, registers no real tools yet, and has the shared infrastructure skeleton in place.



**Tasks:**

- [ ] Add dependencies to `compiler/mcp/Cargo.toml`: `rmcp` (with `transport-io` feature), `tokio`, `serde`, `serde_json`, `clap`, `uuid`, `tracing`, `tracing-subscriber`, plus `ironplc-parser` (for `CompilerOptions`/`Dialect`)

- [ ] Replace `main.rs` with async entry point: parse CLI args (`clap`), set up `tracing` subscriber writing to stderr (REQ-ARC-043), start `rmcp` stdio server

- [ ] Define CLI `Args` struct with all flags from design doc upfront: `--log-file`, `--log-format` (`json`/`text`), `--log-level`, `--max-cached-containers`, `--max-cached-container-bytes`, `--max-duration-ms`, `--max-fuel`, `--max-wall-clock-ms`, `--max-samples`, `--max-variables-per-run` (most fields unused until later phases but defining them now avoids churn)

- [ ] Emit basic `connection_start` / `connection_end` log events with `connection_id` (UUID) (REQ-ARC-044)

- [ ] Define shared response wrapper enforcing `ok: bool` on every response (REQ-STL-005)

- [ ] Verify: `cargo build -p ironplc-mcp` succeeds, `ironplcmcp` starts and responds to MCP `initialize` handshake



**Key files:** `compiler/mcp/Cargo.toml`, `compiler/mcp/src/main.rs`



**Risk:** Low. Main learning curve is `rmcp`'s async model + tool registration macros.



---



### Phase 2: `list_options` + Options Validation



**Goal:** First real tool. Also establishes the shared `validate_options()` helper that every subsequent tool reuses.



**Tasks:**

- [ ] Implement `list_options` tool (REQ-TOL-060..063): iterate `Dialect::ALL` and `CompilerOptions::FEATURE_DESCRIPTORS`, return `dialects` + `flags` arrays

- [ ] Write shared `validate_options(json) -> Result<CompilerOptions, Diagnostic>` that: requires `dialect`, rejects unknown keys (using `FEATURE_DESCRIPTORS` ids as the allowlist), applies flag overrides on top of `from_dialect()` (REQ-STL-002, REQ-TOL-025..026)

- [ ] Register tool with description from REQ-ARC-050

- [ ] Verify: call `list_options` via MCP client, confirm JSON shape matches design doc example



**Key files:** `compiler/mcp/src/options.rs`, tool handler module  

**Reuses:** `ironplc_parser::options::{Dialect, CompilerOptions, FeatureDescriptor}`



**Risk:** Trivial. Good smoke test for `rmcp` tool registration.



---



### Phase 3: `parse`



**Goal:** First tool that accepts `sources` + `options`. Establishes the per-call Project pattern, name validation, and diagnostic line/col conversion.



**Tasks:**

- [ ] Implement shared name validation (REQ-STL-004): UTF-8, non-empty, <=256 bytes, no NUL/`/`/`\`, reject duplicates

- [ ] Implement shared per-call Project construction: create `FileBackedProject::with_options(options)`, call `change_text_document(FileId::from_string(name), content)` for each source (REQ-ARC-010..012)

- [ ] Implement byte-offset → line/col conversion for MCP diagnostics: 1-indexed lines, 1-indexed Unicode scalar columns, tab = 1 col (REQ-TOL-023). Write fresh — the LSP's `span_to_range` uses 0-indexed UTF-16 columns, different semantics. Dedupe later in phase 13

- [ ] Implement `parse` tool (REQ-TOL-010..013): for each source, call `source.library()` to parse; collect diagnostics; build best-effort `structure` array by walking returned `Library` elements to extract `kind`/`name`/`file`/`start_line`/`end_line`

- [ ] Return `ok`, `structure`, `diagnostics` — `ok: true` only when no error-severity diagnostics

- [ ] Verify: call `parse` with valid ST source, confirm structure array; call with broken syntax, confirm diagnostics with correct line/col



**Key files:** `compiler/mcp/src/sources.rs`, `compiler/mcp/src/diagnostics.rs`, tool handler  

**Reuses:** `FileBackedProject`, `change_text_document`, `Source::library()`, `FileId::from_string`



**Risk:** Medium. The in-memory Project wiring and coordinate conversion are the real work, not parsing itself. Also need to confirm `Library` elements carry enough location info for the `structure` output.



---



### Phase 4: `check`



**Goal:** Primary validation tool — full parse + semantic analysis.



**Tasks:**

- [ ] Implement `check` tool (REQ-TOL-020..026): reuse phase 3 Project construction, call `project.semantic()`, translate all diagnostics (parse + semantic) to JSON

- [ ] Ensure compiler errors are returned as diagnostics, never MCP-level errors (REQ-TOL-024)

- [ ] Verify: call `check` with valid source → `ok: true`, empty diagnostics; call with type error → `ok: false`, diagnostic with correct problem code and location



**Key files:** Tool handler  

**Reuses:** Phase 3 shared infrastructure, `project.semantic()`



**Risk:** Low — thin wrapper once phase 3 lands. The `semantic()` call is identical to what the CLI `check` command does.



---



### Phase 5: `explain_diagnostic`



**Goal:** Problem-code lookup. Embeds `.rst` docs at build time.



**Tasks:**

- [ ] Implement `explain_diagnostic` tool (REQ-TOL-070..072)

- [ ] Embed problem-code `.rst` files from `docs/reference/compiler/problems/` via `include_str!` at build time, keyed by code (e.g. `"P0001"`)

- [ ] Parse RST into plain-text rendering for `title`, `description`, `suggested_fix` fields

- [ ] Handle unknown codes: `ok: false`, `found: false`, diagnostic (REQ-TOL-071)

- [ ] Verify: call with `"P0001"` → populated response; call with `"P9999"` or unknown code → `found: false`



**Key files:** Tool handler, possibly `compiler/mcp/build.rs` for include_str generation  

**Reuses:** Problem code docs at `docs/reference/compiler/problems/P####.rst`



**Risk:** Low. Main subtlety is the RST→plain-text rendering (decide: strip directives or ship raw RST). The design doc path errata needs to be accounted for.



---



### Phase 6: `symbols`



**Goal:** Full symbol table extraction from semantic analysis output.



**Tasks:**

- [ ] Build a `SemanticContext` walker module that extracts programs, functions, function_blocks, and types from the analyzed project

- [ ] Implement variable classification: `direction` (Local/In/Out/InOut/Global/External), `address` (hardware mapping or null), `external` flag per REQ-TOL-051

- [ ] Implement `pou` filter: narrow response to one POU + transitively referenced types (REQ-TOL-054)

- [ ] Implement `max_symbols_response_bytes` cap (default 256 KiB): serialize, measure, return `truncated: true` with diagnostic if over limit (REQ-TOL-055)

- [ ] Handle not-found POU: `ok: false`, `found: false`, diagnostic (REQ-TOL-054)

- [ ] Verify: call on multi-POU source → full symbol table; call with `pou` filter → narrowed response; call on large project → truncation behavior



**Key files:** `compiler/mcp/src/symbols.rs` (walker), tool handler  

**Reuses:** `SemanticContext` (types, functions, symbols fields), `project.semantic_context()`



**Risk:** HIGH. The direction/address/external classification (REQ-TOL-051) requires inspecting variable qualifiers + hardware address mapping + VAR_GLOBAL visibility rules. The 256 KiB cap requires serialize-then-measure. The `pou` filter must walk referenced types transitively.



---



### Phase 7: Context Tools



**Goal:** Five lightweight projections reusing the phase 6 symbols walker.



**Tasks:**

- [ ] `project_manifest` (REQ-TOL-200..201): file names, POU names, UDTs grouped by kind

- [ ] `project_io` (REQ-TOL-210..212): inputs (drivable) and outputs (observable) with fully-qualified names

- [ ] `pou_scope` (REQ-TOL-220..221): all variables visible to a named POU

- [ ] `pou_lineage` (REQ-TOL-230..231): upstream/downstream dependency DAG for a POU

- [ ] `types_all` (REQ-TOL-240): all UDTs with kind-specific detail fields

- [ ] Verify: call each tool on a multi-POU source with types, FBs, programs, verify shapes



**Key files:** Tool handlers (one per tool), reuse `symbols.rs` walker  

**Reuses:** Phase 6 SemanticContext walker



**Risk:** Low for most. `pou_lineage` is the exception — needs the dependency DAG. Check whether `SemanticContext` exposes it or if `xform_toposort_declarations` output is needed.



---



### Phase 8: `compile` + Container Cache



**Goal:** Full pipeline (parse → analyze → codegen) producing an `.iplc` container, stored in a process-level LRU cache.



**Tasks:**

- [ ] Implement container cache: `Arc<Mutex<LruCache>>` with dual bounds (entry count + total bytes), configurable via CLI args from phase 1 (REQ-ARC-070..073)

- [ ] Generate opaque `container_id` strings (e.g. UUID prefix)

- [ ] Implement `compile` tool (REQ-TOL-030..036): parse all sources → merge into single Library → `analyze()` → `codegen::compile()` → serialize container to bytes → store in cache → return `container_id`, `tasks`, `programs`, `diagnostics`

- [ ] Extract task metadata (name, priority, kind, interval_ms) and program metadata (name, task binding) from the compiled container (REQ-TOL-032..033)

- [ ] Support `include_bytes: true` → base64-encode container bytes in response (REQ-TOL-034)

- [ ] Verify: compile valid source → `ok: true`, `container_id` present; compile with errors → `ok: false`, diagnostics; verify cache stores and retrieves



**Key files:** `compiler/mcp/src/cache.rs`, tool handler  

**Add deps:** `ironplc-analyzer`, `ironplc-codegen`, `ironplc-container`, `base64`, `lru`  

**Reuses:** `analyze()`, `codegen::compile()`, `Container::write_to()`, CLI `compile` as reference



**Risk:** Medium. Four moving parts (cache, codegen, metadata extraction, base64) but each is straightforward individually.



---



### Phase 9: `container_drop`



**Goal:** Explicit cache eviction.



**Tasks:**

- [ ] Implement `container_drop` (REQ-TOL-150..151): remove from cache, return `ok`/`removed`

- [ ] Unknown `container_id` → `ok: false`, `removed: false`, diagnostic

- [ ] Verify: compile → drop → verify gone; drop unknown id → `removed: false`



**Risk:** Trivial.



---



### Phase 10: `run` (Minimal)



**Goal:** Basic VM execution: single task, `every_cycle` trace mode, no stimuli. Establishes the VM integration and fully-qualified variable name resolution.



**Tasks:**

- [ ] Implement fully-qualified variable name resolution (REQ-ARC-020..021): `Program.Var`, `Program.FB.Var`, resource-scoped globals, configuration-qualified names. Reject ambiguous/unresolved names with diagnostic

- [ ] Implement basic `run` tool: look up `container_id` in cache (or decode `container_base64`), load into VM via `Container::read_from` + `VmBuffers::from_container` + `Vm::new().load().start()`

- [ ] Execute for `duration_ms` simulated time, derive cycle count from task intervals

- [ ] Return `trace` array (every_cycle mode), `summary` with `final_values` and `completed_cycles`, `terminated_reason: "completed"`

- [ ] Enforce resource limits: `max_duration_ms`, `max_fuel`, `max_wall_clock_ms`, `max_samples`, `max_variables_per_run` (REQ-ARC-030..035)

- [ ] Support `trace_outputs: true` to auto-include all externally observable variables

- [ ] Verify: compile a simple cyclic program → run → trace shows variable changes over cycles



**Key files:** `compiler/mcp/src/runner.rs`, tool handler  

**Add deps:** `ironplc-vm`  

**Reuses:** `lsp_runner.rs` as reference for VM load→start→step pattern



**Risk:** HIGH. Name resolution (REQ-ARC-020) is a small parser on its own with nested FB instance paths. Resource limit enforcement requires wall-clock monitoring across async boundaries.



---



### Phase 11: `run` (Full)



**Goal:** Complete `run` implementation: stimuli, all trace modes, task filtering.



**Tasks:**

- [ ] Implement `stimuli` schedule (REQ-TOL-042): validate sort order, validate target variables are inputs (match `project_io` classification), type-check values against declared types

- [ ] Implement JSON↔PLC value conversion for all IEC types (REQ-TOL-043): BOOL↔boolean, integers↔number, LINT/ULINT↔string, REAL/LREAL↔number with NaN/Infinity strings, TIME/DATE↔IEC literal strings, enums↔qualified strings, arrays↔JSON arrays, structs↔JSON objects (recursive)

- [ ] Implement trace modes (REQ-TOL-044): `every_ms` (with `interval_ms`), `on_change`, `final_only`

- [ ] Implement `tasks` filter (REQ-TOL-045): only emit trace from named tasks

- [ ] Implement trace cap (REQ-TOL-046): `min(trace.max_samples, server_max_samples)`

- [ ] Implement `limits` override validation: caller can tighten but not loosen (REQ-ARC-031)

- [ ] Verify: run with stimuli driving inputs → observe output changes; test each trace mode; test limit enforcement and `terminated_reason` values



**Risk:** HIGH. The type-checked JSON↔PLC value conversion (REQ-TOL-043) covers every IEC type including nested aggregates. Stimulus validation must exactly match `project_io` input classification.



---



### Phase 12: `format`



**Goal:** Canonical re-rendering via `plc2plc`.



**Tasks:**

- [ ] Implement `format` tool (REQ-TOL-080..084): parse each source via `source.library()`, call `plc2plc::write_to_string(library)`, return formatted content

- [ ] Per-entry tracking: when parse fails, return original content with `formatted: false` and per-entry diagnostics (REQ-TOL-081)

- [ ] Aggregate diagnostics in top-level `diagnostics` array

- [ ] `ok: true` only when all entries formatted successfully

- [ ] Verify: format valid source → same canonical form as plc2plc; format broken source → original content preserved; format is idempotent (REQ-TOL-082): format(format(x)) == format(x)



**Key files:** Tool handler  

**Add deps:** `ironplc-plc2plc`  

**Reuses:** `plc2plc::write_to_string()` (same as CLI `echo` command, but with different error-handling — per-entry success tracking that `echo` doesn't do)



**Risk:** Low. `write_to_string` already works. The per-entry error handling is the only new logic.



---



### Phase 13: Observability Polish



**Goal:** Complete the structured logging requirements that go beyond the basic stderr logging from phase 1.



**Tasks:**

- [ ] Add per-tool summary fields to log entries (REQ-ARC-041): `source_count`, `source_total_bytes`, `dialect`, `diagnostic_count`, `error_count`, `warning_count`, `problem_codes`, tool-specific fields

- [ ] Implement payload hashing (REQ-ARC-042): SHA-256 first 12 hex chars over canonical JSON, replacing source text / stimulus values / trace values in logs. Add `sha2` dep

- [ ] Implement `--log-file <path>` redirect (REQ-ARC-043)

- [ ] Implement `--log-format json|text` (REQ-ARC-043)

- [ ] Implement `--log-level=debug` payload logging with stderr warning (REQ-ARC-042)

- [ ] Verify log stream answers the five questions in REQ-ARC-045



**Risk:** Low. Mechanical once all tools exist.



---



## Verification



After all phases, the full CI pipeline must pass:



```bash

cd compiler && just

```



End-to-end test: start `ironplcmcp` as a subprocess, send MCP `initialize`, then exercise the agentic verification loop from the design doc:

1. `list_options` → get dialect info

2. `parse` with draft ST source → confirm structure

3. `check` → get diagnostics, fix, re-check until `ok: true`

4. `explain_diagnostic` on any unfamiliar code

5. `symbols` / `pou_scope` / `project_io` → understand the program

6. `compile` → get `container_id`

7. `run` with stimuli → verify trace matches expected behavior

8. `format` → get canonical source

9. `container_drop` → clean up



Each phase should also have its own focused tests (unit tests for shared infrastructure, integration tests for each tool handler).

