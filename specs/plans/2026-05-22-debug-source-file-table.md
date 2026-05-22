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

### Hash algorithm — BLAKE3 everywhere

ADR-0007 originally selected SHA-256 for `content_hash`, `source_hash`,
`debug_hash`, and `layout_hash`. This plan switches **all** integrity
hashes in the container — header hashes and the new per-file hashes
alike — to **BLAKE3**. Both algorithms produce 32-byte digests at
default settings, so the header layout shape is unchanged; only the
algorithm is.

Rationale for BLAKE3:

1. **Performance.** BLAKE3 is several × faster than SHA-256 on
   commodity hardware. The compiler hashes every input source file on
   every build, plus four header-level digests per container — the
   savings compound.
2. **Same security posture, no length-extension.** BLAKE3 has 128-bit
   collision/preimage security at 32-byte output, matching SHA-256, and
   removes SHA-256's length-extension footgun if a caller ever hashes
   structured data.
3. **Consistency.** Mixing algorithms within one container format is a
   small but real maintenance tax (two dependencies, two test
   strategies, two places to get wrong). Picking one and using it
   everywhere is cheaper to reason about.
4. **Pre-1.0.** No installed base depends on the current algorithm
   choice, so the switch costs nothing today and costs more if deferred.

ADR-0007 will be amended (not just annotated) to record BLAKE3 as the
chosen algorithm. The "Decision" and "Consequences" sections both need
updates; the structure of the dual-signature integrity model is
unaffected.

### `header.source_hash` is removed, not repurposed

The single `header.source_hash` field captured "did any source change?"
in a single-file world. With per-file `content_hash` entries in tag 6
this is strictly less informative — you get "something changed" but
not "which file." Since we're already touching the format and there is
no installed base, **`header.source_hash` is removed from the header**
rather than kept as a deprecated zero-filled slot.

Cascading offset shifts in `HEADER_SIZE`:

| Field | Old offset | New offset |
|-------|------------|------------|
| `content_hash` | 8 | 8 |
| `source_hash` | 40 | *(removed)* |
| `debug_hash` | 72 | 40 |
| `layout_hash` | 104 | 72 |

`HEADER_SIZE` shrinks by 32 bytes. `header.rs` write/read logic, the
header-size constant, and every test fixture that depends on offsets
beyond byte 40 need to be updated together.

The per-file `content_hash` entries in tag 6 are the sole source of
truth for source integrity. The `debug_hash` (BLAKE3 over the entire
debug section) transitively protects them.

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

1. `Emitter::set_source_position` is widened to
   `set_source_position(file_id: u16, line: u16, column: u16)` — the
   AST node's `Located::span()` carries all three together, so passing
   them at the same call site keeps the data flow simple and avoids a
   "forgot to update the current file" state-management bug. The dedup
   check in `record_line_map_entry` keys on the full
   `(file_id, line, column)` triple. The Emitter API landed in the
   previous PR but is not yet consumed by anything outside codegen, so
   widening the signature has zero external cost.
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
  but `debug_section.rs` is std-only today). Drop any unused
  `sha2`/`sha-256` dependency once header hashing is migrated.
- `compiler/container/src/header.rs` — remove `source_hash` field;
  shift `debug_hash` and `layout_hash` offsets; update `HEADER_SIZE`;
  update `write_to`/`read_from`; update all header tests.
- Any callers that compute or set `source_hash` (currently the
  builder/cli) — remove those code paths.
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
- `specs/adrs/0007-dual-signature-integrity-model.md` — substantive
  amendment: switch all hash algorithms from SHA-256 to BLAKE3 in the
  Decision and Consequences sections; remove the `source_hash` entry
  and replace it with a reference to the per-file BLAKE3 hashes in the
  debug section's SOURCE_FILE_TABLE.
- Any other ADRs that mention SHA-256 in passing (audit
  `specs/adrs/` for "SHA-256" references and update).

## Resolved Decisions

The three questions raised during plan review are resolved as follows:

1. **`header.source_hash`** → **removed**. Per-file BLAKE3 hashes in
   tag 6 are strictly more informative ("which file changed") than the
   single combined hash. Pre-1.0, no installed base, so the field is
   dropped from the header outright rather than left as a vestigial
   zero-filled slot. See "`header.source_hash` is removed, not
   repurposed" above.
2. **Per-function file mapping** → **not added**. The line map is
   sorted by `(function_id, bytecode_offset)`, so one binary search on
   a `function_id` returns its first entry and the `file_id` falls out
   for free. A separate `(function_id → file_id)` sub-table would only
   accelerate the "give me the file but never the line/column"
   scenario, which doesn't arise in `stackTrace` or any other
   identified consumer.
3. **`set_source_position` signature** → **takes the full triple**.
   The Emitter API has no external consumers yet (only codegen calls
   it, in this same workspace), so we are not paying a backwards-compat
   cost to choose the better shape. `set_source_position(file_id,
   line, column)` lines up with `Located::span()` returning all three
   at once, and removes a class of "forgot to update the current file"
   state bug.

## Tasks

Land in two commits on this branch so review can separate the
algorithm migration from the new sub-table:

**Commit A — BLAKE3 migration and header cleanup.**

- [ ] Add `blake3` dependency to `compiler/container/Cargo.toml`;
      remove the existing SHA-256 dependency once nothing else uses it.
- [ ] Replace SHA-256 with BLAKE3 in every header-hash computation
      (`content_hash`, `debug_hash`, `layout_hash`); update fixtures.
- [ ] Remove `source_hash` from `Header`; shift `debug_hash` and
      `layout_hash` offsets; update `HEADER_SIZE`,
      `Header::write_to`/`read_from`, all header tests, and any
      builder/cli code that previously computed or set `source_hash`.
- [ ] Amend ADR-0007: switch to BLAKE3 throughout; remove the
      `source_hash` row and reference the SOURCE_FILE_TABLE for source
      integrity. Audit `specs/adrs/` for other SHA-256 mentions and
      update.
- [ ] Run `cd compiler && just`; commit.

**Commit B — SOURCE_FILE_TABLE + LineMapEntry file_id + Emitter API.**

- [ ] Define `SourceFileEntry { path: String, content_hash: [u8; 32] }`
      and `TAG_SOURCE_FILE: u16 = 6` in `debug_section.rs`. Implement
      encode/decode with unit tests covering round-trip, malformed-size
      rejection, and forward compat (an older reader that doesn't
      recognize tag 6 still loads the section by skipping the tag).
- [ ] Extend `LineMapEntry` with `file_id: u16`, bump
      `LINE_MAP_ENTRY_SIZE` 8 → 10, update every constructor across
      production code and tests (codegen, builder, xml).
- [ ] Add `ContainerBuilder::add_source_file` /
      `add_source_files` with a builder-level test.
- [ ] Widen `Emitter::set_source_position` to take
      `(file_id, line, column)`; add `file_id` to
      `EmittedLineMapEntry`; update the dedup check.
- [ ] Thread a source-bytes lookup into `codegen::compile()`; update
      `ironplc-cli` and any other caller to provide it.
- [ ] In `compile.rs`, accumulate the `SourceFileTable` from the
      `FileId`s seen during codegen, compute BLAKE3 per file from the
      provided source bytes, and register the entries with the
      `ContainerBuilder`.
- [ ] Update `specs/design/bytecode-container-format.md` (tag 6 row)
      and `specs/design/debugger-support.md` (close gap #7, mark tag 6
      implemented).
- [ ] End-to-end test: compile a two-file project, write the container
      to disk, read it back, verify SOURCE_FILE_TABLE rows in the
      expected order and that every `LineMapEntry.file_id` resolves to
      one of them.
- [ ] Drift-detection test: hash a known source string with BLAKE3 in
      the test, compile that same string, and confirm the stored entry
      hash matches. (DAP-server drift policy is out of scope; this
      only verifies the hash is correct and stable.)
- [ ] Run `cd compiler && just`; commit.
