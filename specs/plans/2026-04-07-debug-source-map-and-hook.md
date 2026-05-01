# Source Map in Debug Section (Line Map Emission)

## Status

This plan covers a single slice of the larger debugger work: **emitting line-map entries from codegen and surviving the optimizer**. The full debugger architecture — iterative dispatch, FrameStack, extended DebugHook trait, breakpoints, stepping, DAP server — lives in [`specs/design/debugger-support.md`](../design/debugger-support.md). Variable/function name emission lives in [`specs/design/debug-info-in-iplc-container.md`](../design/debug-info-in-iplc-container.md). Both are out of scope here.

### What's already in place (do not re-do)

- `DebugSection` Tag 1 `LineMapEntry { function_id, bytecode_offset: u16, source_line: u16, source_column: u16 }` with read/write/lookup (`compiler/container/src/debug_section.rs:90`, `lookup_source_location` at `:305`).
- `ContainerBuilder::add_line_map_entry` per-entry insertion (`compiler/container/src/builder.rs:206`).
- Minimal `DebugHook` trait + `NoopDebugHook` + dispatch-loop call site (`compiler/vm/src/debug_hook.rs`, `vm.rs:687`). The trait is `before_instruction(pc, op)` with no `FunctionId` and no return value; that signature is intentionally retired and replaced by the version in the debugger design spec. **Don't extend it in this plan**.
- Optimizer returns its old→new offset map (PR #1013): `optimize(bytecode, constants) -> (Vec<u8>, OffsetMap)`. The remap table at `compiler/codegen/src/optimize.rs:214` is now exposed.
- `finalize_function` helper (PR #1014) is the single seam where every emitted function passes through optimize → bytecode (`compile.rs:279`). Used by init, scan, user functions, and FB bodies.
- `Located` impl on `StmtKind` plus `span: SourceSpan` fields on `Assignment`, `If`, `Case`, `For`, `While`, `Repeat`, `FbCall` (PR #1017). `compile_statement` can now ask any statement for its span without reaching into the variants.
- `LineColumn::from_offset(source, offset)` byte-offset → line/column helper (`compiler/dsl/src/diagnostic.rs:45`). It's O(n) per call; this plan adds a precomputed index alongside it.

### What's left for this plan

1. Precompute a per-source line index (replace the O(n) per-call helper).
2. Track the active span on the `Emitter` and record one map entry per statement's first instruction.
3. Plumb the source map through `finalize_function`, remap it through the optimizer's `OffsetMap`, and hand it to the builder.
4. Add a bulk `add_line_map_entries` to the builder.
5. Wire it all together at the four `finalize_function` call sites.

## Approach

### 1. `LineIndex` for fast byte-offset → line/column lookup

File: `compiler/dsl/src/diagnostic.rs`

Add a `LineIndex` that builds a `Vec<u32>` of newline byte offsets once per source file and binary-searches on lookup. `LineColumn::from_offset` stays as a convenience but the codegen path uses `LineIndex` to avoid re-scanning the source per statement.

```rust
pub struct LineIndex {
    line_starts: Vec<u32>,  // line_starts[0] = 0, [i] = byte offset of line i (1-based output)
}
impl LineIndex {
    pub fn from_source(source: &str) -> Self { ... }
    /// Returns 1-based (line, column). Column is in code points.
    pub fn line_column(&self, byte_offset: usize) -> (u32, u32) { ... }
}
```

Tests: empty source, single line, trailing newline, offset past end (clamp), mid-line.

### 2. Span tracking on `Emitter`

File: `compiler/codegen/src/emit.rs`

```rust
pub struct Emitter {
    // ... existing fields ...
    current_span: Option<(u16, u16)>,                // (line, column) — function-scoped
    line_map: Vec<(u32 /*offset*/, u16 /*line*/, u16 /*column*/)>,
}

impl Emitter {
    /// Records that the *next* emitted opcode begins a new statement at this
    /// source position. Subsequent emits within the same statement do not
    /// advance the map. Calling with the same (line, col) as the last
    /// recorded entry is a no-op.
    pub fn mark_source_position(&mut self, line: u16, column: u16) { ... }

    pub fn line_map(&self) -> &[(u32, u16, u16)] { &self.line_map }
}
```

Implementation: in `emit_opcode` (the single funnel — see the existing `emit_opcode` at `emit.rs:139`), if `current_span` is set and differs from the last recorded entry, push `(self.bytecode.len() as u32, line, col)` *before* writing the opcode byte, then clear `current_span` so it doesn't fire on the next byte of the same instruction. The peephole load/store bypasses go through `emit_opcode` for the actual write, so they're covered.

`Emitter::default()` adds the new fields as `None` / empty.

### 3. Codegen: call `mark_source_position` per statement

File: `compiler/codegen/src/compile_stmt.rs`

`compile_statement` (line ~59) is the single dispatch point. Before the `match stmt`, ask the statement for its span via the new `Located` impl, convert via `LineIndex` (carried on `CompileContext`), and call `emitter.mark_source_position(line, col)`.

```rust
fn compile_statement(emitter: &mut Emitter, ctx: &mut CompileContext, stmt: &StmtKind) -> Result<(), Diagnostic> {
    let span = stmt.span();
    if !span.is_builtin() {
        if let Some(idx) = ctx.line_index.as_ref() {
            let (line, col) = idx.line_column(span.start);
            emitter.mark_source_position(line.min(u16::MAX as u32) as u16,
                                          col.min(u16::MAX as u32) as u16);
        }
    }
    match stmt { ... }
}
```

`CompileContext` gains an `Option<LineIndex>` field, populated once by `compile()` from the source text. (See item 6 — `compile()`'s signature must accept the source.) Synthesized statements with `SourceSpan::default()` produce no entry.

Statement-level granularity is sufficient for v1 stepping (per the design spec). Sub-statement spans can be added incrementally later; the format already supports it.

### 4. Plumb the source map through `finalize_function`

File: `compiler/codegen/src/compile.rs`, `compiler/codegen/src/optimize.rs`

`FinalizedFunction` grows a `line_map: Vec<LineMapEntry>` field. Inside `finalize_function`:

```rust
pub(crate) fn finalize_function(
    emitter: &mut Emitter,
    ctx: &CompileContext,
    function_id: FunctionId,
) -> FinalizedFunction {
    let raw_map: &[(u32, u16, u16)] = emitter.line_map();
    let (bytecode, offset_map) = crate::optimize::optimize(emitter.bytecode(), &ctx.constants);
    let line_map = remap_line_map(raw_map, &offset_map, function_id);
    let max_stack_depth = emitter.max_stack_depth();
    FinalizedFunction { bytecode, max_stack_depth, line_map }
}
```

The new `remap_line_map` (in `optimize.rs` next to `OffsetMap`):

For each `(old_offset, line, col)`:
- If `offset_map[old_offset]` exists and points at a **kept** instruction, emit `LineMapEntry { function_id, bytecode_offset: new_offset, source_line: line, source_column: col }`.
- If the original instruction was removed by the optimizer, snap forward to the next kept instruction's new offset (look up the next greater old offset that's in `offset_map` and points at a kept instruction). This preserves the source attribution rather than dropping it.
- After collection, sort by `bytecode_offset` and dedupe consecutive entries that share `(line, col)`.

`offset_map` already includes a one-past-the-end sentinel (`optimize.rs:222`), which is enough to make "snap forward" total.

The `function_id` argument flows from each call site in `compile.rs` and `compile_fn.rs` — the IDs are already known at those sites.

### 5. Bulk insertion on the builder

File: `compiler/container/src/builder.rs`

```rust
pub fn add_line_map_entries(mut self, entries: impl IntoIterator<Item = LineMapEntry>) -> Self {
    self.debug_line_map.extend(entries);
    self
}
```

The per-entry method stays for tests and incremental constructions.

### 6. Wire-up at the four `finalize_function` call sites

Files: `compiler/codegen/src/compile.rs:477,486`, `compiler/codegen/src/compile_fn.rs:317,548`

Each call passes its `FunctionId`. After all four functions are finalized, the program-level builder gathers their `line_map` vectors and calls `builder = builder.add_line_map_entries(...)` once. `compile.rs::compile_program_with_functions` already iterates the compiled functions for the same `add_function` plumbing (`compile.rs:467-507`).

`compile()` (the top-level entry) currently takes `(&Library, &SemanticContext, &CodegenOptions)`. To populate `LineIndex` it needs source text. Two options:

- **(a) Pass source text alongside the AST** — `compile(&library, &context, &options, &source_map)` where `source_map: &HashMap<FileId, String>`.
- **(b) Stash the index on `CodegenOptions`** — caller builds it, codegen consumes it. Lower-impact for existing callers; passes through to `CompileContext`.

Recommend (b): the LSP already has `get_source(file_id)` (`compiler/project/src/project.rs:155`); the CLI does too via `FileBackedProject`. Both build a `LineIndex` per file before invoking codegen.

If `CodegenOptions.line_indices` is empty, `compile_statement` skips `mark_source_position` and the line map ends up empty — same shape as a release build with debug stripped.

## Files to Modify

- `compiler/dsl/src/diagnostic.rs` — `LineIndex`
- `compiler/codegen/src/emit.rs` — `mark_source_position`, `line_map()`, span tracking inside `emit_opcode`
- `compiler/codegen/src/compile.rs` — `CompileContext.line_index`, `finalize_function` returns `line_map`, wire entries into builder, `CodegenOptions.line_indices`
- `compiler/codegen/src/compile_fn.rs` — pass `function_id` to `finalize_function`, propagate line maps
- `compiler/codegen/src/compile_stmt.rs` — call `mark_source_position` at the top of `compile_statement`
- `compiler/codegen/src/optimize.rs` — `remap_line_map` helper using the existing `OffsetMap`
- `compiler/container/src/builder.rs` — `add_line_map_entries` bulk method
- Callers that construct `CodegenOptions` (`ironplc-cli/src/cli.rs`, `ironplc-cli/src/lsp_runner.rs`, `benchmarks/src/lib.rs`) — populate `line_indices` from already-available source text

## Testing

`compiler/dsl/src/diagnostic.rs` (inline):
- `line_index_when_empty_source_then_single_line`
- `line_index_when_offset_at_newline_then_advances_line`
- `line_index_when_offset_past_end_then_clamps`
- `line_index_when_multibyte_then_columns_count_codepoints`

`compiler/codegen/src/emit.rs` (inline):
- `emitter_when_mark_source_position_then_records_entry_at_next_opcode_offset`
- `emitter_when_two_emits_share_position_then_single_entry`
- `emitter_when_position_unchanged_between_marks_then_no_duplicate_entry`

`compiler/codegen/src/optimize.rs` (inline; this is the highest-risk part — exercise it):
- `remap_line_map_when_no_optimization_then_offsets_unchanged`
- `remap_line_map_when_redundant_load_store_removed_then_entry_snaps_forward`
- `remap_line_map_when_add_zero_removed_then_entry_snaps_forward`
- `remap_line_map_when_jump_target_preserved_then_entry_unchanged`
- `remap_line_map_when_multiple_removals_then_all_entries_consistent` — random programs (proptest); assert every remapped offset lands on an instruction boundary in the optimized stream and resolves to the same source line as the unoptimized version.

`compiler/codegen/tests/` (end-to-end):
- `compile_line_map_when_simple_assignment_then_entry_per_statement`
- `compile_line_map_when_multiple_statements_then_offsets_increase`
- `compile_line_map_when_if_then_branch_targets_have_entries`
- `compile_line_map_when_no_line_indices_then_empty_map` (release-build behavior)
- `compile_line_map_when_optimized_then_offsets_remap_through_finalize_function`

End-to-end:
- `cd compiler && just` — compile, coverage ≥85%, clippy, fmt must all pass.
- Compile a small ST program; load the container; confirm `DebugSection.lookup_source_location(SCAN, pc)` returns the expected lines for each statement's first PC.

## Out of scope (covered elsewhere)

- **Iterative VM dispatch / FrameStack / pausable execution** — `specs/design/debugger-support.md` Layer 2 Part A. The current recursive `execute_with_hook` stays; this plan only writes data into the container.
- **Extending the `DebugHook` trait** (`HookAction`, `before_call`/`after_return`, `FunctionId` parameter) — `specs/design/debugger-support.md` Layer 2 Part B. The minimal existing trait is unchanged here.
- **Variable name and function name debug emission** — `specs/design/debug-info-in-iplc-container.md`.
- **Breakpoints, stepping, DAP server, VS Code integration** — `specs/design/debugger-support.md` Layers 3–4.
- **Sub-statement (expression-level) source positions** — format already permits, but stepping granularity is at statement level for v1.
- **Source-text embedding in the container** — separate flag, separate sub-table.
