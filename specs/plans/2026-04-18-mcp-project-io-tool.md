# Plan: MCP `project_io` Tool

## Context

The MCP server (parent plan: `specs/plans/2026-04-14-mcp-server-plan.md`) has
completed Phases 1–6 plus Phases 8–9. Phase 6 shipped `symbols` (#928).

Phase 7 of the parent plan groups five **context tools** — lightweight
projections on the semantic context that let an agent answer a narrow
structural question cheaply without paying the response-size cost of `symbols`.
Phase 7 is being delivered one tool per PR. The first, `project_manifest`
(`specs/plans/2026-04-17-mcp-project-manifest-tool.md`), has already landed
(#931). **Four context tools remain**: `project_io`, `pou_scope`,
`pou_lineage`, `types_all`.

This plan covers `project_io` — the first of the four remaining context
tools, listed next in the parent plan's Phase 7 task list and in the design
doc section ordering (`specs/design/mcp-server.md:301`). It is a direct
prerequisite for Phase 10 (`run`), since agents use `project_io` to discover
which variables they can drive in `stimuli` and which they should trace.

Requirements in `specs/design/mcp-server.md`:
- **REQ-TOL-210** — classify inputs (drivable from outside)
- **REQ-TOL-211** — classify outputs (observable from outside)
- **REQ-TOL-212** — each entry has `name`, `type`, `address` (or `null`)
- **REQ-ARC-020** — fully-qualified variable names

## Approach

Mirror `project_manifest.rs` one-for-one. A single new file
`compiler/mcp/src/tools/project_io.rs` adds a free `build_response()`
function; `server.rs` registers a `#[tool]` handler; subprocess tests live
in `tests/cli.rs`. Reuse every shared helper we already have.

### Response shape

```json
{
  "ok": true,
  "inputs":  [{ "name": "Main.Start",    "type": "BOOL", "address": "%IX0.0" }],
  "outputs": [{ "name": "Main.MotorRun", "type": "BOOL", "address": "%QX0.0" }],
  "diagnostics": []
}
```

### Classification rules (REQ-TOL-210 / REQ-TOL-211)

Walk `SemanticContext` and bucket each variable by role. A single variable
can appear in both arrays (e.g. `VAR_IN_OUT`, non-addressed globals).

| Source | Inputs | Outputs |
|---|---|---|
| Program `VAR_INPUT` | yes | — |
| Program `VAR_OUTPUT` | — | yes |
| Program `VAR_IN_OUT` | yes | yes |
| `VAR_EXTERNAL` (any scope) | yes | — |
| Global, no address | yes | yes |
| Any variable with `%I*` address | yes | — |
| Any variable with `%Q*` address | — | yes |
| Any variable with `%M*` address | — | — (explicitly excluded) |

Direction derives from `SymbolInfo.variable_type` exactly as in
`symbols.rs:256-268`. Address prefix (`%I` / `%Q` / `%M`) is derived from
`SymbolInfo.address: Option<String>` — the analyzer has already formatted it.

### Name qualification (REQ-ARC-020)

For the first pass, produce the two most common forms:
- `<program>.<variable>` for variables declared inside a program
- Bare `<variable>` for top-level `VAR_GLOBAL`

Nested function-block-instance qualification (`Program.FB.Var`) and
configuration/resource-qualified names are deferred. Those forms are needed
for Phase 10 (`run`) name resolution; the nested-instance walker will be
introduced once there and retrofitted into this tool.

### Sort + determinism

Emit both arrays lexicographically by `name`, same rationale as
`project_manifest.rs:111-112`.

### Error handling

Copy `project_manifest.rs:57-103` verbatim, swapping the response type:
1. `validate_sources()` errors → early return with `ok: false`, `P8001`.
2. `parse_options()` errors → early return with `ok: false`, `P8001`.
3. `project.semantic()` errors → collect diagnostics, still attempt to
   populate partial response from `project.semantic_context()` if present.
   `ok: false` whenever any diagnostic has `severity == "error"`.

## Files

| File | Action |
|------|--------|
| `compiler/mcp/src/tools/project_io.rs` | **Create** — `build_response()`, response structs, unit tests |
| `compiler/mcp/src/tools/mod.rs` | Modify — add `pub mod project_io;` |
| `compiler/mcp/src/server.rs` | Modify — add `#[tool(name = "project_io", ...)]` handler |
| `compiler/mcp/tests/cli.rs` | Modify — add subprocess rows to the existing `rstest` table |

## Critical reuse points

- **Shared input envelope:** `tools::common::ParseCheckInput` —
  same `{sources, options}` as `project_manifest` and `check`.
- **Shared helpers:** `validate_sources`, `parse_options`,
  `serialize_diagnostics` (`compiler/mcp/src/tools/common.rs`).
- **Variable-direction derivation:** replicate the `match` at
  `compiler/mcp/src/tools/symbols.rs:256-268`. Not promoted to
  `common.rs` yet — wait until a third tool needs it.
- **Scope iteration:**
  - Programs: `context.symbols().get_programs()`
  - Per-program variables: `context.symbols().get_variables_in_scope(&ScopeKind::Named(program_id.clone()))`
  - Globals: `context.symbols().get_variables_in_scope(&ScopeKind::Global)`
- **Project construction:** `MemoryBackedProject::new(options)` +
  `add_source` + `semantic` + `semantic_context` — sequence at
  `project_manifest.rs:75-99`.

## Verification

**Unit tests in `project_io.rs`** (colocated, BDD naming):
- program with `VAR_INPUT` listed in inputs
- program with `VAR_OUTPUT` listed in outputs
- program with `VAR_IN_OUT` listed in both
- variable with `AT %IX0.0` populates `address` and is in inputs
- variable with `AT %QX0.0` populates `address` and is in outputs
- variable with `AT %MX0.0` appears in neither
- global without address appears in both
- global with input address appears in inputs only
- `VAR_EXTERNAL` appears in inputs
- multiple variables sorted lexicographically
- semantic error → `ok:false` with diagnostics
- empty source name → `P8001`
- missing dialect → `P8001`

**Subprocess tests** added to the `rstest` table in
`compiler/mcp/tests/cli.rs`:
- success: program with one `VAR_INPUT` → stdout contains expected name
- semantic-error path → `"ok":false` with a diagnostic
- validation path: empty source name → `"P8001"`

**CI:** `cd compiler && just` must pass (compile, coverage ≥ 85 %, clippy).

## Out of scope (follow-ups)

- `pou_scope`, `pou_lineage`, `types_all` — one PR each, same template.
- Nested FB-instance name qualification — introduced with `run`.
- Configuration/resource-qualified globals — same.
