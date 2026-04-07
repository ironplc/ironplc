# Source Map in Debug Section + DebugHook Trait

## Context

Currently the bytecode debug section (`compiler/container/src/debug_section.rs`) carries variable and function name metadata (sub-tables 2 and 3) but no mapping from bytecode offsets back to source line/column positions. This blocks line-level breakpoints, runtime error reporting tied to source, and any future stepping debugger.

We need to (a) extend the debug section with a source map sub-table associating bytecode offsets with `(file_id, line, column)`, (b) emit those entries from codegen using spans already available on AST nodes, (c) ensure the bytecode optimizer (`optimize.rs`) correctly remaps offsets when it deletes instructions, and (d) add a `DebugHook` trait the VM calls before each instruction so debuggers can observe execution. The hook must be zero-cost when no debugger is attached.

The optimizer already rewrites bytecode (removes redundant load/store, identity arithmetic) and shifts jump targets via an old→new offset map. The source map MUST be remapped through that same map; this is the highest-risk part of the change and needs the most test coverage.

## Approach

### 1. Extend `DebugSection` with a SourceMap sub-table

File: `compiler/container/src/debug_section.rs`

- Add `TAG_SOURCE_MAP: u16 = 4`.
- Add struct:
  ```rust
  pub struct SourceMapEntry {
      pub function_id: FunctionId,
      pub bytecode_offset: u32,   // offset within the function's bytecode
      pub file_id: u16,
      pub line: u32,
      pub column: u32,
  }
  ```
- Add `source_map: Vec<SourceMapEntry>` field to `DebugSection`.
- Sort by `(function_id, bytecode_offset)` on write so consumers can binary-search.
- Encode payload: `count(u32) | [function_id u16 | offset u32 | file_id u16 | line u32 | col u32]*`.
- Extend `write_to`, `read_from`, `section_size`. Unknown-tag skip path already supports forward compatibility.
- Add `lookup(function_id, offset) -> Option<&SourceMapEntry>` returning the greatest entry with `offset <= pc` (binary search).

### 2. Builder additions

File: `compiler/container/src/builder.rs`

- Add `debug_source_map: Vec<SourceMapEntry>`.
- Add `add_source_map_entry(entry)` and bulk `add_source_map_entries(iter)`.
- Wire into the built `DebugSection`.

### 3. Thread spans through codegen

Files: `compiler/codegen/src/emit.rs`, `compile_expr.rs`, `compile_stmt.rs`, `compile_fn.rs`, `compile.rs`

- Add to `Emitter`:
  ```rust
  current_span: Option<(FileId, u32, u32)>,
  source_map: Vec<(u32 /*offset*/, FileId, u32, u32)>,
  ```
- Add `set_current_span(span: &SourceSpan, file_table: &FileTable)`. Use the existing line/column resolver if present, otherwise add a small helper that converts byte offsets to (line, col) using the source text registered for `file_id`. (Investigate `compiler/dsl` for existing helper before adding one.)
- In `emit_opcode`, if `current_span` is set AND the previous source map entry's tuple differs, push `(self.bytecode.len() as u32, file_id, line, col)` BEFORE writing the opcode byte. This dedupes consecutive instructions sharing a span.
- Update `compile_stmt`/`compile_expr` call sites to call `set_current_span` at each statement and at top-level expressions. Statements are the natural breakpoint granularity; finer expression-level entries are optional but cheap.
- `Emitter::finalize` returns the source map alongside bytecode.

### 4. Survive optimization (CRITICAL)

File: `compiler/codegen/src/optimize.rs`

The current `optimize` returns only `Vec<u8>`. Change the call sites in `compile.rs` (and any others that call `optimize`) to use a new function:

```rust
pub fn optimize_with_source_map(
    bytecode: &[u8],
    constants: &[PoolConstant],
    source_map: &[(u32, FileId, u32, u32)],
) -> (Vec<u8>, Vec<(u32, FileId, u32, u32)>)
```

Implementation reuses the existing decode/mark/remap/rebuild pipeline (lines 313-376). After the offset remap table is built (lines 345-353):

1. For each source map entry `(old_offset, …)`:
   - If `old_offset` maps to a kept instruction, translate using the offset map → push `(new_offset, …)`.
   - If `old_offset` belonged to a removed instruction, snap forward to the next kept instruction's new offset (so the source line is still attached to executable code). Skip duplicates.
2. Return remapped vec sorted/deduped.

Keep the original `optimize` as a thin wrapper that calls the new function with an empty map for any callers that don't need source info.

### 5. DebugHook trait + VM integration

File: `compiler/vm/src/vm.rs`

- Define in a new module `compiler/vm/src/debug_hook.rs`:
  ```rust
  pub trait DebugHook {
      fn before_instruction(&mut self, function_id: FunctionId, pc: usize, opcode: u8);
  }
  pub struct NoopDebugHook;
  impl DebugHook for NoopDebugHook {
      #[inline(always)]
      fn before_instruction(&mut self, _: FunctionId, _: usize, _: u8) {}
  }
  ```
- Make `execute` generic over `H: DebugHook` (monomorphization → the no-op call inlines and disappears, preserving zero-cost). Provide a public wrapper `execute(...)` that passes `NoopDebugHook` and `execute_with_hook<H>(..., hook: &mut H)` for debugger consumers.
- Inside the dispatch loop (vm.rs ~line 610), immediately after `let op = bytecode[pc]; pc += 1;`, insert:
  ```rust
  hook.before_instruction(current_function_id, pc - 1, op);
  ```
  (Pass `current_function_id` from the call frame; if not currently tracked in the loop frame, add it — the function ID is already known at call entry.)
- Verify with `cargo asm` or a release build benchmark that the noop variant produces identical machine code to the current loop. If not, gate the call behind `if H::IS_ACTIVE` const.

Debugger consumers can resolve `(function_id, pc)` → source location via `DebugSection::lookup` over the source map sub-table.

## Files to Modify

- `compiler/container/src/debug_section.rs` — new entry, tag, encode/decode, lookup
- `compiler/container/src/builder.rs` — builder API
- `compiler/codegen/src/emit.rs` — span tracking + source map emission
- `compiler/codegen/src/compile_stmt.rs` — `set_current_span` per statement
- `compiler/codegen/src/compile_expr.rs` — optional finer-grained spans
- `compiler/codegen/src/compile_fn.rs` / `compile.rs` — wire optimizer + builder
- `compiler/codegen/src/optimize.rs` — `optimize_with_source_map`
- `compiler/vm/src/debug_hook.rs` — new trait + noop
- `compiler/vm/src/vm.rs` — generic dispatch loop, hook call
- `compiler/vm/src/lib.rs` — re-export `DebugHook`

## Testing (test thoroughly — optimizer is the risk)

Inline unit tests in `debug_section.rs`:
- `debug_section_write_read_when_source_map_then_roundtrips`
- `debug_section_write_read_when_all_three_tables_then_roundtrips`
- `debug_section_lookup_when_offset_between_entries_then_returns_predecessor`
- `debug_section_read_when_unknown_and_known_tags_mixed_then_skips_unknown`

External tests under `compiler/codegen/tests/`:
- `compile_source_map_when_simple_assignment_then_entry_per_statement.rs`
- `compile_source_map_when_multiple_statements_then_offsets_increase.rs`
- `compile_source_map_when_if_then_branch_targets_have_entries.rs`
- `optimize_source_map_when_redundant_load_store_removed_then_offsets_remap.rs` — emit a known sequence whose load/store pair gets eliminated; assert that the surviving entries point to the correct shifted offsets and that no entry points into the middle of an instruction.
- `optimize_source_map_when_add_zero_removed_then_entry_snaps_forward.rs`
- `optimize_source_map_when_jump_target_preserved_then_entry_unchanged.rs`
- `optimize_source_map_when_multiple_removals_then_all_entries_consistent.rs` (stress / property-style: random programs, assert every remapped offset is on an instruction boundary in the optimized stream and resolves back to the same source location as the unoptimized version).

External tests under `compiler/vm/tests/`:
- `debug_hook_when_executing_then_called_per_instruction.rs` — count calls equals decoded instruction count.
- `debug_hook_when_noop_then_results_match_plain_execute.rs`
- `debug_hook_when_resolving_pc_then_returns_expected_source_line.rs` — end-to-end: compile a small POU, run with a recording hook, look up each PC in `DebugSection`, assert the recorded line sequence matches the source program.

End-to-end verification:
- `cd compiler && just` — full CI pipeline (compile, coverage ≥85%, clippy, fmt) must pass before PR.
- Run an example program with a recording hook and print the line trace; manually confirm against the source.
- Quick microbench (criterion or `just bench` if available) comparing `execute` vs `execute_with_hook<NoopDebugHook>` to confirm zero overhead.

## Out of scope

- Actual breakpoint UI / DAP integration (this PR only provides the primitives).
- Inline expression-level spans beyond statements (can be added incrementally; format already supports it).
- Stepping over/into semantics — those live in the debugger consumer of `DebugHook`.
