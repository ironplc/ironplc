# Plan: MCP Server Scaffold + `list_options` Tool

## Goal

Get the `ironplc-mcp` crate building as a real MCP server that handles the protocol handshake over stdio, then implement the `list_options` tool (the simplest real tool — takes no inputs, returns dialect and feature-flag metadata). This is the first incremental slice of the MCP server design in `specs/design/mcp-server.md`.

## Step 1: Extend `FeatureDescriptor` and `Dialect` in the parser crate

**File:** `compiler/parser/src/options.rs`

Add `option_key: &'static str` field to `FeatureDescriptor`. In the `define_compiler_options!` macro, populate it via `stringify!($vendor_field)` — this produces the exact Rust field name (e.g. `"allow_c_style_comments"`), which is the MCP option key per REQ-TOL-063.

Add two methods to `Dialect`:
- `display_name(&self) -> &'static str` — `"IEC 61131-3 Ed. 2"`, `"IEC 61131-3 Ed. 3"`, `"RuSTy-compatible"`
- `description(&self) -> &'static str` — the prose summaries currently hardcoded inside `describe_dialects()`

Update `describe_dialects()` to call the new methods instead of duplicating strings.

Add tests:
- `feature_descriptors_when_called_then_option_key_matches_field_name`
- `feature_descriptors_when_called_then_all_option_keys_start_with_allow`
- `dialect_display_name_when_ed2_then_human_readable`
- `dialect_description_when_ed2_then_contains_edition_2`

## Step 2: Scaffold the MCP server (zero tools, passes handshake)

### Cargo.toml changes

**File:** `compiler/mcp/Cargo.toml`

Replace contents. Key dependency changes:
- Remove `ironplc-project` (not imported until a source-accepting tool is added)
- Add `ironplc-parser` (for `Dialect`, `CompilerOptions`, `FeatureDescriptor` in Step 3)
- Add `rmcp = { version = "1.4", features = ["server", "transport-io"] }` — the official Rust MCP SDK
- Add `tokio = { version = "1", features = ["rt", "macros", "io-std"] }` — required by rmcp
- Add `serde = { version = "1", features = ["derive"] }` and `serde_json = "1"`
- Add `env_logger = "0.10.0"` and `log = "0.4.20"` — same logging crates the CLI uses; `env_logger` writes to stderr by default (REQ-ARC-043), and rmcp's internal `tracing` output is automatically bridged to the `log` facade

### New files

**`compiler/mcp/src/main.rs`** — Minimal entry point:
- `#[tokio::main(flavor = "current_thread")]` (single-connection stdio server)
- Calls `ironplc_mcp::logging::init()` then `ironplc_mcp::run_server().await`
- Returns `Result<(), String>` (matching CLI convention)

**`compiler/mcp/src/lib.rs`** — Module tree + `pub async fn run_server()`:
- Creates `IronPlcMcp` instance
- Creates stdio transport via `rmcp::transport::io::stdio()`
- Calls `.serve(transport).await` then `.waiting().await`

**`compiler/mcp/src/server.rs`** — Server handler:
- `pub struct IronPlcMcp` with `tool_router: ToolRouter<Self>` field
- `#[tool_router(server_handler)]` impl block (initially empty — zero tools)
- Overrides `get_info()` to return server name, version, and instructions string

**`compiler/mcp/src/logging.rs`** — `pub fn init()`:
- Configures `env_logger` writing to stderr (the default; stdout is the JSON-RPC channel)
- Same pattern as `compiler/ironplc-cli/src/logger.rs` but simpler (no verbosity levels or file output yet)

### CLI args: deferred

No `clap` dependency yet. The design doc's `--log-file`, `--log-format`, `--max-cached-containers` etc. are not needed until later slices. Adding them now would be dead code.

### Tests

- `server_when_get_info_then_returns_server_name` — unit test on `get_info()`

## Step 3: Implement `list_options` tool

### New files

**`compiler/mcp/src/tools/mod.rs`** — `pub mod list_options;`

**`compiler/mcp/src/tools/list_options.rs`** — Response types + builder:

```
ListOptionsResponse { dialects: Vec<DialectInfo>, flags: Vec<FlagInfo> }
DialectInfo { id, display_name, description }
FlagInfo { id, flag_type (serialized as "type"), default: serde_json::Value, description, allowed_values: Option }
```

`pub fn build_response() -> ListOptionsResponse`:
- Iterates `Dialect::ALL`, calls `display_name()` and `description()` for each
- Prepends the special `allow_iec_61131_3_2013` flag (not in `FEATURE_DESCRIPTORS`, but a real `CompilerOptions` field — surfacing it satisfies REQ-TOL-063)
- Iterates `CompilerOptions::FEATURE_DESCRIPTORS`, using `.option_key` for the `id`
- All flags are `type: "bool"`, `default: false`, no `allowed_values`
- Total: 3 dialects, 15 flags (1 special + 14 vendor)

### Modified files

**`compiler/mcp/src/lib.rs`** — add `pub mod tools;`

**`compiler/mcp/src/server.rs`** — add `#[tool]` method inside the `#[tool_router]` block:
- Name: `"list_options"`
- Description: exact text from REQ-ARC-050
- No parameters
- Returns `build_response()` serialized as JSON text content

### Tests (in `tools/list_options.rs`)

- `build_response_when_called_then_returns_all_dialects` — count == 3
- `build_response_when_called_then_dialect_ids_match_display_format` — contains ed2, ed3, rusty
- `build_response_when_called_then_contains_all_flags` — count == 15
- `build_response_when_called_then_all_flags_are_bool_type`
- `build_response_when_called_then_all_defaults_are_false`
- `build_response_when_called_then_contains_c_style_comments_flag`
- `build_response_when_called_then_contains_iec_2013_flag`
- `build_response_when_called_then_serialized_json_is_valid`
- `build_response_when_called_then_each_dialect_has_display_name_and_description`
- `build_response_when_called_then_each_flag_has_nonempty_description`

## Step 4: Run CI

- Run `cd compiler && just` to verify everything passes (compile, tests, coverage, clippy, fmt)

## Verification

1. `cargo build -p ironplc-mcp` succeeds with zero warnings
2. `cargo test -p ironplc-mcp` — all unit tests pass
3. `cargo test -p ironplc-parser` — existing + new tests pass
4. `cd compiler && just` — full CI passes
5. Manual smoke test: pipe an MCP `initialize` request into `ironplcmcp` on stdin, verify JSON-RPC response on stdout with `serverInfo`; then send `tools/list` and verify `list_options` appears; then call `list_options` and verify 3 dialects + 15 flags in the response

## Key files

| File | Action |
|------|--------|
| `compiler/parser/src/options.rs` | Modify: add `option_key` to `FeatureDescriptor`, add `Dialect::display_name/description`, update `describe_dialects`, add tests |
| `compiler/mcp/Cargo.toml` | Rewrite: new dependencies |
| `compiler/mcp/src/main.rs` | Rewrite: tokio entry point |
| `compiler/mcp/src/lib.rs` | New: module tree + `run_server()` |
| `compiler/mcp/src/server.rs` | New: `IronPlcMcp` + `ServerHandler` impl |
| `compiler/mcp/src/logging.rs` | New: stderr env_logger setup |
| `compiler/mcp/src/tools/mod.rs` | New: tool module index |
| `compiler/mcp/src/tools/list_options.rs` | New: response types, `build_response()`, tests |

## Tasks

- [ ] Step 1: Extend `FeatureDescriptor` and `Dialect` in `compiler/parser/src/options.rs`
- [ ] Step 2: Scaffold the MCP server (Cargo.toml, main.rs, lib.rs, server.rs, logging.rs)
- [ ] Step 3: Implement `list_options` tool (tools/mod.rs, tools/list_options.rs, wire into server.rs)
- [ ] Step 4: Run full CI pipeline (`cd compiler && just`)
