# Implement Debug Sub-table Tag 6 — Source File Table (BLAKE3-hashed)

## Goal

Add the SOURCE_FILE_TABLE sub-table (tag 6) reserved by
`specs/design/debugger-support.md`, so that `LineMapEntry` and stack-frame
file attribution can resolve a numeric `file_id` to (a) a source path and
(b) a content hash for drift detection. Per-file content hashes use
**BLAKE3**.

This closes gap #7 ("No source file table — v1 assumes single file per
container") in `specs/design/debugger-support.md` and turns the
`(function_id, bytecode_offset) → (line, column)` mapping landed in the
04-07 source-map plan into a navigable `(file, line, column)` mapping
the DAP server can hand to a debugger client.

## Context

The container format spec
(`specs/design/bytecode-container-format.md`) and the debugger-support
spec (`specs/design/debugger-support.md`) already reserve the relevant
tags and call out the gap:

- `Tag 0 = SOURCE_TEXT` — embedded source text (UTF-8). Out of scope for
  this plan; it's a separate `--debug`-gated feature.
- `Tag 6 = SOURCE_FILE_TABLE` — reserved for the multi-file case. **In
  scope.**
- The header carries `source_hash: [u8; 32]` (SHA-256 per ADR-0007). This
  field captures a single-file world; for the multi-file world the per-file
  hashes live in the new tag-6 table, and `header.source_hash` remains as
  a coarse "did anything change at all" digest (see Open Questions).

The current implementation state is partial:

- `compiler/container/src/debug_section.rs` defines tags 1–4 and 9.
  Tags 5, 6, 7, 8 are unused; **6 is reserved for this work** per the
  design spec.
- `LineMapEntry` is `function_id(2) + bytecode_offset(2) + source_line(2)
  + source_column(2)` — 8 bytes. It does **not** yet carry `file_id`.
- `Emitter::set_source_position(line, column)` (in
  `compiler/codegen/src/emit.rs`) tracks position only — no file.
- `compiler/dsl::core::FileId` is a path-based enum (`File(Arc<str>)`),
  not a numeric index. Codegen has access to it via `Located::span()`.

We previously had no way to ask, in the debug section, "what file does
offset N in function F come from?" This plan supplies the answer in two
steps: extend `LineMapEntry` with `file_id`, and define tag 6 as the
table that resolves it.

### Hash algorithm choice — BLAKE3

ADR-0007 selected SHA-256 for the four header-level integrity hashes
(`content_hash`, `source_hash`, `debug_hash`, `layout_hash`). Those four
stay on SHA-256 in this plan; we do not amend ADR-0007.

The new per-file hash in tag 6 uses **BLAKE3** because:

1. BLAKE3 is several × faster than SHA-256 on commodity hardware, and the
   compiler hashes every input source file on every build. SHA-256 only
   hashed the concatenated source once.
2. The header hashes are 32-byte fixed-format fields embedded in a
   wire-stable header; changing those is an ADR-level decision with
   migration cost. The tag-6 entries are brand new, so we get to pick
   freely.
3. BLAKE3 output is 32 bytes (configurable XOF, but we use the default
   length), matching the storage shape we already use for the header
   hashes. No layout differences.
4. BLAKE3 has no length-extension weakness, removing a class of footgun
   if a caller ever hashes structured data into the same digest.

We will record this departure as a note in ADR-0007 ("per-file source
hashes in the debug section use BLAKE3; the four header-level hashes
remain SHA-256") rather than a new ADR — the choice is local to one
sub-table and the rationale fits in a paragraph.

## Architecture

### On-disk format — tag 6 SOURCE_FILE_TABLE

```text
sub-table payload:
  count: u16
  entries: [SourceFileEntry; count]

SourceFileEntry:
  path_len:     u16     (UTF-8 bytes; u16 because absolute paths can exceed 255)
  path:         [u8; path_len]
  content_hash: [u8; 32] (BLAKE3-256 of the file's UTF-8 source bytes)
```

A reader looks up `path` and `content_hash` by index — the `file_id` in
`LineMapEntry` is that index. `file_id = 0` is a valid file id (the
first registered file); there is no sentinel "unknown file" — entries
that have no real file (synthetic/built-in code) simply do not emit a
line-map entry.

### `LineMapEntry` grows to 10 bytes

Add `file_id: u16` to `LineMapEntry`:

```rust
pub struct LineMapEntry {
    pub function_id: FunctionId,   // u16
    pub bytecode_offset: u16,
    pub file_id: u16,              // new — index into SOURCE_FILE_TABLE
    pub source_line: u16,
    pub source_column: u16,
}
```

Wire layout: `function_id(2) + bytecode_offset(2) + file_id(2) +
source_line(2) + source_column(2) = 10 bytes`. Update
`LINE_MAP_ENTRY_SIZE` (currently 8) and the payload-size helpers.

This is a wire-format change to tag 1. Tag 1 has been emitted since the
04-07 source-map plan landed (commit `5efeff8` etc.), so any
already-shipped `.iplc` files with debug info will be skipped by the new
reader on size mismatch — acceptable given debug info is opt-in,
release builds don't carry it, and the format is pre-1.0.

### Codegen plumbing

1. `Emitter::set_source_position` grows a `file_id: u16` parameter (or
   we add a sibling `set_current_file(file_id)` and keep position
   per-token). I recommend folding both into one call —
   `set_source_position(file_id, line, column)` — so the dedup check in
   `record_line_map_entry` keys on the full triple.
2. `EmittedLineMapEntry` (in `emit.rs`) gains a matching `file_id`
   field.
3. `compile.rs` accumulates a `SourceFileTable` during compilation:
   - When the compiler first encounters a `FileId` (via
     `Located::span().file_id()`), it registers the path and computes
     `blake3::hash(source_bytes)` for it, returning a `u16` index.
   - Subsequent references to the same `FileId` reuse the index.
   - The accumulated table is written into the `DebugSection` via a new
     `ContainerBuilder::add_source_file(SourceFileEntry)` method.
4. Source bytes for hashing must be the **exact bytes the parser saw**
   (no normalization, no trailing-newline fixup). The container reader
   recomputes BLAKE3 on the user-provided source text and compares to
   the stored hash; any normalization would produce false drift
   warnings.

### Where source bytes come from

The compiler driver in `compiler/ironplc-cli` already reads files from
disk and hands the bytes to `parse_program`. We need to thread those
bytes (or a `&HashMap<FileId, &[u8]>`) into `codegen::compile()` so
codegen can hash them. The simplest shape is a closure:

```rust
fn compile(
    ...
    source_lookup: impl Fn(&FileId) -> Option<&[u8]>,
) -> Result<CompiledArtifact, ...>
```

If a `FileId` resolves to `None` (e.g., a synthetic id from a
programmatic test), codegen records the path with an all-zero hash. The
reader treats all-zero hash as "drift check unavailable" rather than
"hash mismatch."

## File Map

Modified:

- `compiler/container/Cargo.toml` — add `blake3 = "1"` (std-only;
  enable `default-features = false` if no_std builds touch this code,
  but `debug_section.rs` is std-only today).
- `compiler/container/src/debug_section.rs` —
  - add `TAG_SOURCE_FILE: u16 = 6`;
  - add `SourceFileEntry { path: String, content_hash: [u8; 32] }`;
  - add `source_files: Vec<SourceFileEntry>` field to `DebugSection`;
  - extend `LineMapEntry` with `file_id: u16`, bump
    `LINE_MAP_ENTRY_SIZE` from 8 to 10;
  - implement `write_source_files` / `read_source_files` matching the
    other tag handlers;
  - update `section_size`, `num_sub_tables`, `write_to`, `read_from`,
    and all existing tests that construct `LineMapEntry` (set
    `file_id: 0`).
- `compiler/container/src/builder.rs` — add
  `add_source_file(SourceFileEntry)` / `add_source_files(impl
  IntoIterator<...>)`; update the builder's debug-section test.
- `compiler/codegen/src/emit.rs` —
  - add `file_id: u16` to `EmittedLineMapEntry`;
  - change `set_source_position(line, column)` →
    `set_source_position(file_id, line, column)`;
  - update dedup check to compare the full triple.
- `compiler/codegen/src/compile.rs` —
  - thread a source-bytes lookup into `compile()`;
  - register each unique `FileId` seen during codegen into a
    `SourceFileTable`, computing BLAKE3 hashes;
  - forward the table into `ContainerBuilder::add_source_files`;
  - update every `set_source_position` call site to also pass `file_id`
    (derived from the AST node's `Located::span().file_id()`).
- `compiler/ironplc-cli/src/...` (and any other callers of
  `codegen::compile`) — pass a closure that returns the in-memory
  source bytes for each `FileId` already loaded by the driver.
- `specs/design/bytecode-container-format.md` — fill in the
  SOURCE_FILE_TABLE row in the tag table with the entry layout above.
- `specs/design/debugger-support.md` — close gap #7 (line ~113) and
  remove "v1 assumes single file per container"; update the tag table
  near line 251 to mark tag 6 implemented.
- `specs/adrs/0007-dual-signature-integrity-model.md` — append a short
  note that per-file source hashes in the debug section use BLAKE3
  while the four header-level hashes remain SHA-256.

## Open Questions

1. **`header.source_hash` for multi-file containers.** Three options:
   (a) keep it as SHA-256 over the concatenation of all source files
   (legacy behavior); (b) redefine it as BLAKE3 over the canonical
   concatenation of `(path || 0x00 || content) || ...`; (c) deprecate
   it for multi-file containers (zero it out) and rely on the per-file
   tag-6 hashes plus `debug_hash`. Recommendation: **(a) for now** — no
   header layout change, no ADR-0007 amendment beyond the BLAKE3 note,
   and the per-file hashes cover the granular case. Revisit if (a) ever
   blocks something.
2. **Per-function file mapping.** A function may span only one file in
   IEC 61131-3 (POUs aren't split across files), so a debugger's
   `stackTrace` can derive a frame's file from its first
   `LineMapEntry`. We don't need a separate `(function_id → file_id)`
   sub-table. Confirming this matches reviewer intuition before
   shipping.
3. **`set_source_position` signature.** Threading a `file_id` through
   every span-emitting call site is mechanical but touches many
   statement-emit functions. An alternative is a `set_current_file`
   helper called once per POU (POUs are one-file-only per #2), with
   `set_source_position(line, column)` continuing to take only the
   pair. Lower diff cost, slightly more state on `Emitter`. Leaning
   toward the per-POU helper.

## Tasks

- [ ] Add `blake3` dependency to `compiler/container/Cargo.toml`.
- [ ] Define `SourceFileEntry`, `TAG_SOURCE_FILE`, and the
      encode/decode pair in `debug_section.rs` with unit tests
      (round-trip, malformed-size rejection, forward compat — older
      reader without the tag still loads the section).
- [ ] Extend `LineMapEntry` with `file_id`, bump
      `LINE_MAP_ENTRY_SIZE`, update every constructor in tests and
      production code (codegen + builder + xml).
- [ ] Add `ContainerBuilder::add_source_file` /
      `add_source_files` and a builder-level test.
- [ ] Extend `EmittedLineMapEntry` and `Emitter` with `file_id`
      tracking — pick between `set_current_file` (preferred per OQ #3)
      or a longer `set_source_position` signature.
- [ ] Thread a source-bytes lookup into `codegen::compile()`; update
      `ironplc-cli` and any other caller to provide it.
- [ ] In `compile.rs`, build the `SourceFileTable` from the set of
      `FileId`s seen during codegen, compute BLAKE3 per file, register
      with the builder.
- [ ] Update `specs/design/bytecode-container-format.md`,
      `specs/design/debugger-support.md`, and ADR-0007 per the File
      Map.
- [ ] End-to-end test: compile a two-file project, write the container
      to disk, read it back, verify the SOURCE_FILE_TABLE has the
      expected `(path, hash)` rows in the expected order, and verify
      every `LineMapEntry.file_id` resolves to one of those rows.
- [ ] Drift-detection test: compile a file, mutate the source on disk,
      recompute BLAKE3 in the test, confirm it differs from the stored
      hash (the actual drift-handling policy in the DAP server is out
      of scope here — this test only verifies the hash is correct and
      stable).
- [ ] Run `cd compiler && just` — compile, coverage, lint must all
      pass before opening the PR.
