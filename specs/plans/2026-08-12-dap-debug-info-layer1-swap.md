# Plan: DAP Layer-1 Debug-Info Swap

## Context

The DAP server's only Layer-1-coupled module,
`compiler/vm-cli/src/dap/debug_info.rs`, still ships the documented
**passthrough** resolvers: a DAP breakpoint `line` is interpreted as a raw
bytecode offset in the scan function, variables render as `var[0]`, `var[1]`…
with no type, and `stack_trace_body` (`dap/server.rs`) names frames
`function {id}` with the bytecode offset as the "line" and no source.

The container already carries everything needed (`compiler/container/src/
debug_section.rs`): the line map (Tag 1, with `lookup_source_location`),
VAR_NAME (Tag 2), FUNC_NAME (Tag 3), STRING layouts (Tag 4), and the source
file table (Tag 6). The design's §"How DAP Uses the Debug Info"
(`specs/design/debugger-support.md`) traces each DAP request to the sub-table
it should read. This change performs that swap — the passthrough module docs
call the two function signatures "the stable seam" for exactly this.

User-visible effect: breakpoints are set by real source line (and snap to the
nearest executable line), the Variables pane shows `counter : DINT = 42`
instead of `var[0] = 42`, and the call stack shows the POU name with the
paused source line highlighted in the editor.

## Design

All debug-section knowledge stays in `dap/debug_info.rs`; `server.rs` keeps
speaking in resolved values only.

### Breakpoints: `resolve_breakpoint`

New signature (the passthrough's return type cannot report the snapped line):

```rust
pub struct ResolvedBreakpoint {
    /// The 1-based source line the breakpoint actually bound to.
    pub line: i64,
    /// Bytecode locations to arm — one per function with code on that line.
    pub locations: Vec<(FunctionId, usize)>,
}

pub fn resolve_breakpoint(
    debug: Option<&DebugSection>,
    source_path: &str,
    line: i64,
) -> Option<ResolvedBreakpoint>
```

Resolution:

1. **File filter.** If the debug section has a source file table, restrict
   line-map entries to `file_id`s whose recorded path matches the requested
   path — exact string match first, then file-name (basename) match to absorb
   absolute-vs-relative and separator differences. No table → no filtering
   (single-source container from an older compiler). A table with no matching
   entry → `None` (unverified: the breakpoint is in a file this container
   was not compiled from).
2. **Snap.** Among the filtered entries, keep those with
   `source_line >= line`; the snapped line is the smallest such
   `source_line`. No candidates (line past the end of the code) → `None`.
3. **Arm.** For each `function_id` with entries on the snapped line, arm the
   smallest `bytecode_offset` on that line (the statement start). Multiple
   functions on one line each get one location.

`server.rs::set_breakpoints` echoes `resolved.line` (not the requested line)
in the verified `Breakpoint` response so the editor moves the dot to where
the breakpoint actually bound.

### Variables: `render_variables`

New signature — STRING values live in the data region, not the slot:

```rust
pub fn render_variables(
    debug: Option<&DebugSection>,
    values: &[u64],
    data_region: &[u8],
) -> Vec<Variable>
```

- Build a `var_index → &VarNameEntry` map once per call (entries are
  index-unique across the flat variable table; function locals carry their
  owning `function_id` but occupy distinct indices).
- Named slot: `name` and `type` from the entry, `value` from
  `ironplc_container::debug_format::format_variable_value(raw, iec_type_tag)`
  — the shared helper already used by the CLI dump and playground.
- `STRING`: locate the layout in `string_layouts` by `var_index`, read
  `[max_len u16][cur_len u16][bytes…]` at `data_offset`, render as a quoted
  literal. Out-of-bounds offset/length (corrupt debug info) renders
  `<invalid>` rather than trapping.
- `WSTRING`: `<not available>` (same v1 cut as the playground).
- Slot with no VAR_NAME entry (or no debug section): keep the passthrough
  `var[i]` / signed-i32 fallback so the pane never goes blank.

### Stack frames: new `resolve_frame`

```rust
pub struct FrameInfo {
    pub name: String,
    pub line: i64,
    pub column: i64,
    pub source: Option<(String, String)>, // (file name, path)
}

pub fn resolve_frame(
    debug: Option<&DebugSection>,
    function_id: FunctionId,
    pc: usize,
) -> FrameInfo
```

- `name`: FUNC_NAME lookup, falling back to `function {id}`.
- Location: `DebugSection::lookup_source_location(function_id, pc)` (largest
  entry `<= pc`, the standard enclosing-line lookup). On a hit, `line` /
  `column` come from the entry and `source` from
  `source_files[file_id]` (file name + recorded path). No hit → `line: 0`,
  no source — VS Code shows the frame name without jumping anywhere.

`server.rs::stack_trace_body` gains a `debug` parameter and maps `FrameInfo`
into the existing `StackFrame` type (`Source { name, path }`).

## Non-goals (follow-ups, per the design)

- **Scope grouping** (Locals/Inputs/Outputs/Globals from `var_section`) and
  per-frame filtering by `function_id` — the single flat "Variables" scope
  stays; every named slot is listed.
- **Enum value names** (`enum_defs`) — enum slots render as ordinals.
- **WSTRING rendering** — placeholder only.
- **Source drift detection** via the source-file `content_hash`.
- `evaluate`, logpoints, trap→`exception`, `ironplc/stepScan` /
  `ironplc/scanCount` — separate Phase 4 follow-ups.

## Testing

- Unit (`debug_info.rs`): snap-to-next-line, exact-line hit, line past end →
  `None`, basename path matching, non-matching file with a source table →
  `None`, no line map → `None`; variable rendering for BOOL/DINT/REAL,
  STRING from the data region, out-of-bounds STRING layout, missing entry
  fallback; frame resolution with and without FUNC_NAME / line-map hits.
- Integration (`server.rs`): fixture containers gain line-map + source-file +
  FUNC_NAME entries; `setBreakpoints` by source line verifies with the
  snapped line echoed; `stopped` → `stackTrace` carries the POU name, source
  path, and line; `variables` carries names, types, and formatted values.
  Existing tests that set breakpoints by raw offset are updated to set them
  by line against the fixture's line map.
- `cd compiler && just` (compile, coverage ≥ 85%, clippy, fmt) must pass.
