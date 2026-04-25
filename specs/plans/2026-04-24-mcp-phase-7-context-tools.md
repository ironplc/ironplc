# MCP Phase 7: Remaining Context Tools (`pou_scope`, `pou_lineage`, `types_all`)

## Context

`specs/plans/2026-04-14-mcp-server-plan.md` splits MCP server work into
13 phases. Phase 7 covers five "context tools"; three are already wired
into the server (`project_manifest`, `project_io`) — this plan completes
the remaining three:

- `pou_scope` (REQ-TOL-220, REQ-TOL-221)
- `pou_lineage` (REQ-TOL-230, REQ-TOL-231)
- `types_all` (REQ-TOL-240)

All three are thin projections over the `SemanticContext` and analyzed
`Library` already produced by `MemoryBackedProject::semantic()`. No new
workspace crate dependencies are required.

**Design doc reference:** `specs/design/mcp-server.md` sections
"Context Tools" (`pou_scope`, `pou_lineage`, `types_all`).

## Architecture

Each tool follows the same shape as the existing context tools
(`compiler/mcp/src/tools/project_manifest.rs`,
`compiler/mcp/src/tools/project_io.rs`):

1. Validate `sources` and `options` (reuse `tools::common`).
2. Build a `MemoryBackedProject`, call `project.semantic()`.
3. Inspect `project.semantic_context()` and/or `project.analyzed_library()`.
4. Return a typed response that the `server.rs` handler serialises
   to JSON.

### `pou_scope`

- Inputs: `sources`, `options`, `pou` (string).
- Resolution order: Program → Function → FunctionBlock (REQ-TOL-221).
- Variables come from the matching `ProgramDeclaration` / `FunctionDeclaration` /
  `FunctionBlockDeclaration` in `analyzed_library()`. The AST carries
  the `VarDecl.initializer`, which the symbol table does not.
- For each variable emit `{name, type, direction, initial_value}`.
  - `type` is rendered from the `InitialValueAssignmentKind`'s type name
    where available (falls back to `""` when the type is not a simple
    named type).
  - `direction` is computed from `VariableType` (mirror `project_io::direction_of`).
  - `initial_value` is a best-effort opaque string for primitives
    (`Simple(SimpleInitializer { initial_value: Some(ConstantKind) })`,
    `EnumeratedType`, `Reference null/ref`, `String`), `null` otherwise.
- Not found: `ok=false, found=false, variables=[]`, plus P8001 diagnostic (REQ-TOL-221).

### `pou_lineage`

- Inputs: `sources`, `options`, `pou` (string).
- Build a directed graph of POU → POU dependencies from `analyzed_library()`:
  - Edge `A → B` when POU `A` calls function `B` (scan `ExprKind::Function`
    in bodies), invokes a function-block method `B` (scan `FbCall`, resolve
    the variable name to its FB type), or declares a VAR of FB type `B`
    (scan `VarDecl.initializer = FunctionBlock{type_name: B}`).
  - POU names are the set of Programs + user-defined Functions + user-defined
    FunctionBlocks.
- `upstream(pou)` = transitive closure following outgoing edges
  (who does this POU depend on).
- `downstream(pou)` = transitive closure following incoming edges
  (who depends on this POU).
- Both arrays returned sorted, deduped, with the queried POU excluded
  from both (REQ-TOL-230 — "directly or transitively" — does not include self).
- Not found: `ok=false, found=false, upstream=[], downstream=[]`, P8001 diagnostic (REQ-TOL-231).

### `types_all`

- Inputs: `sources`, `options`.
- Iterate `context.types().iter_user_defined()` (same iterator the
  `project_manifest` and `symbols` tools use).
- For each `IntermediateType` variant emit the kind-specific fields listed in REQ-TOL-240:
  - `Enumeration` → `kind:"enum"`, `values:[…]` — resolved via
    `symbols().get_enumeration_values_for_type()` (already used elsewhere
    in the analyzer and codegen).
  - `Structure` → `kind:"struct"`, `fields:[{name, type}]` from
    `IntermediateStructField` with `field_type` rendered as a short string.
  - `Array` → `kind:"array"`, `element_type`, `bounds:[{lower, upper}]`.
  - `Subrange` → `kind:"subrange"`, `base_type`, `low`, `high`.
  - `String` → `kind:"string"`, `length` (nullable).
  - `Reference` → `kind:"reference"`, `target_type`.
  - Otherwise → `kind:"alias"`, `target_type` (best-effort string).
- FunctionBlock / Function entries are excluded — they are POUs, not types.
- Response is a `types` array sorted by name.

## File Map

| File | Action |
|------|--------|
| `compiler/mcp/src/tools/mod.rs` | Declare three new modules. |
| `compiler/mcp/src/tools/pou_scope.rs` | Create — `pou_scope` tool. |
| `compiler/mcp/src/tools/pou_lineage.rs` | Create — `pou_lineage` tool. |
| `compiler/mcp/src/tools/types_all.rs` | Create — `types_all` tool. |
| `compiler/mcp/src/server.rs` | Register three new tools with descriptions from `specs/design/mcp-server.md` REQ-ARC-050. |
| `compiler/mcp/src/spec_conformance.rs` | Replace the five `#[ignore]` placeholders (REQ_TOL_220, 221, 230, 231, 240) with real unit tests. |
| `compiler/mcp/tests/cli.rs` | Add CLI-level happy-path / error-path cases for each new tool. |
| `specs/plans/2026-04-14-mcp-server-plan.md` | No change — the Phase 7 task list remains the system-of-record for phase-wide status. |

## Phases

### 7a — `types_all`

Smallest and fully self-contained. Good first landing to validate the
module-registration pattern before the heavier tools.

### 7b — `pou_scope`

Requires per-POU AST lookup and initial-value rendering. Shares the
direction-helper with `project_io` and `symbols`.

### 7c — `pou_lineage`

Depends on a dependency-graph walker that does not currently exist in
the MCP crate. Kept last so that changes are concentrated.

Each phase lands implementation + unit tests + spec-conformance tests +
CLI integration tests together.

## Verification

- `cd compiler && just test -p ironplc-mcp` must pass.
- `cd compiler && just` must pass (clippy, fmt, coverage).
- Manual smoke: `cargo run -p ironplc-mcp --bin ironplcmcp` + the
  three-message MCP handshake from `tests/cli.rs`, then `tools/call`
  each new tool.
