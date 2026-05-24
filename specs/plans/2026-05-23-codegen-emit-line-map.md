# Wire Codegen to Emit Line-Map Entries with file_id and BLAKE3 Source File Table

## Goal

Make `codegen::compile()` populate the debug section's `LINE_MAP`
(tag 1) with real `(function_id, bytecode_offset, file_id, source_line,
source_column)` entries derived from AST node spans, and register every
referenced source file in the `SOURCE_FILE_TABLE` (tag 6) with a BLAKE3
content hash. Today both tables are empty in production output — the
container-format scaffolding landed in PR #1074, but no codegen call
site uses it yet.

When this lands, a debugger reading a freshly-compiled `.iplc` can:

1. Look up `(function_id, bytecode_offset) → (file_id, line, column)`
   to highlight the source statement currently executing.
2. Resolve `file_id → (path, BLAKE3 hash)` to open the right file and
   check that it hasn't drifted from what the compiler saw.

## Context

The SOURCE_FILE_TABLE and `file_id`-carrying `LineMapEntry` shipped in
PR #1074. The Emitter's `set_source_position(SourceFileId, SourceLine,
SourceColumn)` records entries; `take_line_map()` drains them; the
`ContainerBuilder` accepts entries via `add_line_map_entry` and source
files via `add_source_file`. **No production caller invokes any of
this yet.**

The pieces we need to assemble already exist in tree:

- `ironplc_dsl::core::SourceSpan` (start, end, file_id) is attached to
  every AST node via the `Located` trait, with concrete impls on
  `StmtKind`, `Assignment`, `If`, `For`, `Case`, etc.
  (`compiler/dsl/src/textual.rs:705-714` for the dispatcher).
- `ironplc_dsl::diagnostic::LineColumn::from_offset(source: &str,
  offset: usize)` already converts byte offsets to 0-indexed
  (line, column) — straightforward to `+1` for our 1-based
  `SourceLine`/`SourceColumn`.
- `codegen::compile::finalize_function` (in `compile.rs`) is the
  single chokepoint where every emitted function's bytecode is
  optimized and finalized. Its doc comment explicitly anticipates
  "future cross-cutting additions (e.g. source-map plumbing)". That's
  where the per-function line map gets harvested.
- `crate::optimize::optimize` already returns a (currently unused)
  `_offset_map` parameter — the pre→post optimization offset
  remap. We must apply it to the line map before storing entries,
  otherwise `bytecode_offset` values point into pre-optimization
  positions and don't land on real instruction boundaries.

The production callers of `compile()` today are:

- `ironplc-cli/src/cli.rs:156` (the build subcommand)
- `ironplc-cli/src/lsp_runner.rs:12`
- `playground/src/lib.rs:18`
- `mcp/src/tools/compile.rs:140`
- `benchmarks/src/lib.rs:6`
- `codegen/tests/it/common/mod.rs:8`

All of them have the original source bytes available at the call site
(they had to, to parse). We just need a uniform way to hand those
bytes to `compile()`.

## Architecture

### Source-bytes lookup

Extend `codegen::compile()` to take a `&dyn SourceLookup` (or a closure
— see Open Questions) that maps a `&FileId` to the source bytes the
parser saw for that file:

```rust
pub trait SourceLookup {
    fn source_bytes(&self, file_id: &FileId) -> Option<&[u8]>;
}
```

Callers wrap their already-loaded source string per `FileId` in a small
adapter. Tests that don't have source bytes pass an empty
implementation that always returns `None` — codegen then registers any
seen `FileId` with an all-zero hash (per the spec, "drift check
unavailable"). This keeps test setup cheap.

`compile()`'s new signature:

```rust
pub fn compile(
    library: &Library,
    context: &SemanticContext,
    options: &CodegenOptions,
    sources: &dyn SourceLookup,
) -> Result<Container, Diagnostic>
```

### Per-function line map collection

Extend `FinalizedFunction` (in `compile.rs`) with the per-function line
map:

```rust
pub(crate) struct FinalizedFunction {
    pub(crate) bytecode: Vec<u8>,
    pub(crate) max_stack_depth: u16,
    pub(crate) line_map: Vec<EmittedLineMapEntry>,
}
```

`finalize_function` becomes:

1. Drain `emitter.take_line_map()` → raw per-statement entries (offsets
   in pre-optimization byte space).
2. Call `optimize::optimize(...)` to get the optimized bytecode and the
   `offset_map: Vec<(u32, u32)>` (or similar) mapping
   pre→post offsets.
3. **Remap each entry's `bytecode_offset` through `offset_map`.**
   Entries whose pre-offsets fall on instructions removed by the
   optimizer get snapped forward to the next surviving instruction.
   Entries past the last surviving instruction are dropped.
4. Store the remapped entries on `FinalizedFunction.line_map`.

The remap is a correctness requirement, not an enhancement — without
it, a breakpoint set on a line could resolve to an offset that's not
the start of any instruction in the optimized stream, and the VM
debug hook would never fire for it.

### Plumbing into the container

After `compile()` finalizes each function, it pairs the per-function
`line_map` with the function's `FunctionId` and feeds each entry to the
builder via `add_line_map_entry`. The builder's existing
`sort_line_map` invariant (sorted by `(function_id, bytecode_offset)`)
is maintained automatically by `ContainerBuilder::build`.

### Source-file registration

A small struct in `compile.rs` (or a sub-module — see Open Questions)
owns the `FileId → SourceFileId` mapping:

```rust
struct SourceFileRegistry<'a> {
    sources: &'a dyn SourceLookup,
    seen: HashMap<FileId, SourceFileId>,
    entries: Vec<SourceFileEntry>,
}

impl<'a> SourceFileRegistry<'a> {
    fn intern(&mut self, file_id: &FileId) -> SourceFileId {
        if let Some(id) = self.seen.get(file_id) { return *id; }
        let bytes = self.sources.source_bytes(file_id);
        let content_hash = match bytes {
            Some(b) => *blake3::hash(b).as_bytes(),
            None => [0u8; 32],
        };
        let entry = SourceFileEntry {
            path: file_id.to_string(), // or whatever FileId::Display gives
            content_hash,
        };
        let id = SourceFileId::new(self.entries.len() as u16);
        self.entries.push(entry);
        self.seen.insert(file_id.clone(), id);
        id
    }
}
```

Each statement's `Located::span().file_id` is interned through this
registry before `set_source_position` is called. The final
`entries` vec is handed to the builder via `add_source_files`.

### Where `set_source_position` is called

Per-statement granularity: at the top of `compile_statement` (in
`compile_stmt.rs`), grab the statement's span, convert
`(start_offset, file_id)` to `(file_id, line, column)`, and call
`emitter.set_source_position(...)`. Per-statement is the right
granularity for v1 — IEC 61131-3 ST is "one statement per line"
idiomatic, and finer granularity (per-expression) buys little for
the cost of plumbing.

`compile_stmts` (which iterates a slice) doesn't need to do anything
itself; `compile_statement` handles each one.

We do **not** call `clear_source_position`. The Emitter dedups on
`(file_id, line, column)`, so leaving a stale position across
statements is fine — the next statement just calls
`set_source_position` again.

### Byte-offset → (line, column) conversion

For each statement that fires `set_source_position`, we need its
1-based `(line, column)` derived from the byte offset in the source
text. Two approaches:

1. **Call `LineColumn::from_offset` directly.** O(offset) per call;
   given hundreds of statements per file, this is O(n × stmt_count)
   per file. For typical PLC programs (a few hundred lines, a few
   dozen statements) it's noise. Adopt this first; profile later.
2. **Precompute a line-start table per file.** O(file_len) once,
   then O(log lines) per offset. Worth doing if (1) shows up in
   profiling — it doesn't today.

Plan: use (1). If a future profile of `cargo bench` shows codegen
spending time in `from_offset`, add a small `LineOffsetTable` cache
keyed by `FileId` inside `SourceFileRegistry`.

## File Map

Modified:

- `compiler/codegen/Cargo.toml` — add `blake3 = "1"` (codegen calls
  it to hash source bytes during compile).
- `compiler/codegen/src/compile.rs` —
  - Add `pub trait SourceLookup`.
  - Add `SourceFileRegistry`.
  - Extend `compile()` signature with `sources: &dyn SourceLookup`.
  - Extend `FinalizedFunction` with `line_map`.
  - Update `finalize_function` to drain the emitter line map and
    apply the optimizer offset remap.
  - At the end of `compile`, feed the registry's entries to the
    builder via `add_source_files`, and feed each function's
    `(function_id, [EmittedLineMapEntry])` pairs to
    `add_line_map_entry`.
- `compiler/codegen/src/compile_stmt.rs` — add the
  `set_source_position` call at the top of `compile_statement`,
  using `SourceFileRegistry` (threaded via `CompileContext` — see
  Open Questions) to intern the file id and
  `LineColumn::from_offset` to convert the byte offset.
- `compiler/codegen/src/optimize.rs` — confirm the existing
  `optimize()` return shape exposes the pre→post offset map in a
  consumable form; tighten the type if it's currently opaque.
  Update its tests if the public shape changes.
- `compiler/ironplc-cli/src/cli.rs` — adopt a tiny `HashMap<FileId,
  String>`-backed `SourceLookup` from the already-loaded sources.
- `compiler/ironplc-cli/src/lsp_runner.rs` — same.
- `compiler/playground/src/lib.rs` — same.
- `compiler/mcp/src/tools/compile.rs` — same.
- `compiler/benchmarks/src/lib.rs` — same.
- `compiler/codegen/tests/it/common/mod.rs` — same; for tests that
  don't care, hand in a no-op implementation.

New:

- (Possibly) `compiler/codegen/src/source_registry.rs` — house
  `SourceLookup` and `SourceFileRegistry` if `compile.rs` is getting
  crowded. See Open Questions.

## Open Questions

1. **Where does `SourceLookup` live?** Two options:
   (a) `ironplc_codegen::SourceLookup` (the closest crate to the
   consumer). (b) `ironplc_container::SourceLookup` (alongside
   `SourceFileEntry` which it logically pairs with). Recommendation:
   **(a)** — only codegen calls it, and the container crate shouldn't
   pull in a trait used only at compile time.
2. **Threading `SourceFileRegistry` into `CompileContext`?** The
   registry is mutable state accumulated across many calls. Either
   make it a `&mut SourceFileRegistry<'_>` parameter on every
   compile-stmt function (intrusive) or stash it on `CompileContext`
   (which is `&mut` everywhere already). Recommendation: **stash it
   on `CompileContext`** — same pattern as `ctx.variables` and the
   other accumulators.
3. **Closure vs trait for `SourceLookup`?** A `&dyn Fn(&FileId) ->
   Option<&[u8]>` is lighter but less greppable. A trait makes the
   intent legible. Lean **trait**.
4. **Per-statement offset choice — `start` or position of the
   keyword?** A statement's `SourceSpan` covers the whole construct
   (e.g., entire IF/END_IF block). For `set_source_position` at the
   top of `compile_statement`, we want the *first* line of the
   statement so the debugger lands at the head of an IF, not its
   END_IF. `span.start` is correct.
5. **Nested statements (IF body, FOR body)?** `compile_stmts`
   recurses into bodies, and `compile_statement` is called for each.
   So a `set_source_position` at the top of `compile_statement` runs
   for both the outer IF and each inner statement. That's the right
   behavior — every visible statement gets its own line-map entry.

## Tasks

Land in two commits on this branch so review can separate the
threading from the actual emission:

**Commit A — plumbing only (no behavioral change).**

- [ ] Add `blake3` to `compiler/codegen/Cargo.toml`.
- [ ] Define `SourceLookup` trait in `compiler/codegen/src/compile.rs`
      (or `source_registry.rs`) with a no-op `EmptyLookup` for tests.
- [ ] Extend `compile()` with `sources: &dyn SourceLookup`.
- [ ] Update every caller of `compile()` to pass an empty/no-op lookup
      (ironplc-cli, lsp_runner, playground, mcp, benchmarks, codegen
      tests). Behavior unchanged; just the signature widens.
- [ ] Run `cd compiler && just` — should pass with no behavioral
      change.

**Commit B — actual line-map and source-file-table emission.**

- [ ] Add `SourceFileRegistry` on `CompileContext`; intern file_ids
      on first use; hash bytes via BLAKE3.
- [ ] Extend `FinalizedFunction` with `line_map`; have
      `finalize_function` drain `emitter.take_line_map()` and apply
      the optimizer offset remap. If `optimize()` doesn't expose the
      remap usably yet, fix that first.
- [ ] Call `emitter.set_source_position(...)` at the top of
      `compile_statement` using the registry + `LineColumn::from_offset`.
- [ ] After each function is finalized, pair its entries with their
      `FunctionId` and call `builder.add_line_map_entry` per entry.
- [ ] At the end of `compile`, call
      `builder.add_source_files(registry.entries)`.
- [ ] Update the real callers (`ironplc-cli`, `lsp_runner`,
      `playground`, `mcp`, `benchmarks`) to hand in a real
      `SourceLookup` backed by their already-loaded sources. Tests
      that don't care keep the no-op lookup from Commit A.
- [ ] End-to-end test in `codegen/tests/it/` or `vm-cli/tests/`:
      compile a real multi-statement `.st` program with the real
      driver, write the container, read it back, verify the line
      map has one entry per statement at the expected `(file_id,
      line, column)` and that the SOURCE_FILE_TABLE has one entry
      with the expected BLAKE3 hash.
- [ ] Offset-remap invariant test: compile a program whose optimizer
      will visibly reorder/delete instructions, verify every
      `bytecode_offset` in the line map lands on an instruction
      boundary in the optimized stream (parse the bytecode and check
      offset is at an opcode start).
- [ ] Run `cd compiler && just` — compile, coverage, lint must all
      pass.

## Out of Scope

- Per-expression source-position granularity. Per-statement is fine
  for v1.
- The DAP server / `DebugHook` integration. Those plans live in
  `specs/design/debugger-support.md` and depend on this work but are
  separate PRs.
- Optimizer's "snap forward" handling of statements that get
  entirely elided (e.g., `IF FALSE THEN ... END_IF`). The remap
  drops entries past the end; per the existing 04-07 plan, snap-
  forward is a follow-up with its own invariant test.
