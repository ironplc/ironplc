# Plan: Complete WSTRING Support

## Goal

Finish the WSTRING (UTF-16LE) implementation that is currently parsed but not realized in codegen or VM. After this change, an IEC 61131-3 program can declare, assign, compare, and call standard string functions on WSTRING variables, with the same correctness, safety, and performance guarantees that STRING enjoys today.

## Architecture

Per ADR-0034 (supersedes ADR-0004) and ADR-0035 (supersedes ADR-0015):

- **Single STR_\* opcode family** for both STRING and WSTRING. Width is data-driven, not opcode-driven. No new opcodes.
- **6-byte string header** `[max_length: u16][cur_length: u16][char_width: u16]`. Lengths stay in code units; byte spans are computed as `length * char_width`. Format version bumps 2 → 3.
- **Three runtime encoding tag sites** (defense-in-depth): string header (data region), per-entry tag in the constant pool, per-slot tag in the temp buffer pool.
- **Width-parameterized helpers.** Each string operation has one body parameterized by `char_width: usize`. STR_* opcode handlers stay short.
- **Encoding mismatch traps.** Every string op compares expected to actual encoding and traps on mismatch. The runtime check is mandatory.

## Out of Scope

- **Bytecode verifier (ADR-0034 Layer 1).** Static operand-source-typing is deferred. Layer 2 (analyzer-level type checking) plus Layer 3 (runtime trap) is the safety chain delivered here. The trap is therefore defense-in-depth against codegen bugs and tampering, not against missing static checks.
- WSTRING in struct fields (matches existing STRING-in-struct status: not supported by codegen). Tracked separately if needed.
- Surrogate-pair-aware character iteration (code points above U+FFFF). ADR-0016 explicitly limits WSTRING to BMP semantics; this plan inherits that scope.
- BUILTIN-dispatch reorganization for string functions (ADR-0004 originally specified, current implementation uses dedicated opcodes; reconciling the two is a separate concern).
- Migration of any deployed bytecode at format_version 2. ADR-0015 was `proposed` with no deployed users.
- Performance benchmarking of the per-opcode `char_width` read+compare. Per ADR-0034 it is below the cost of the operation itself; revisit only if the e2e tests show a regression.
- The pre-existing inconsistency in `compiler/plc2plc/src/renderer.rs:459-462` (declaration emits swapped quote chars vs. initializer at `:717-720`). Out of scope; track separately.

## Design Doc Reference

- ADR-0034 — STRING/WSTRING Distinction via Operand Typing and Runtime Encoding Tags
- ADR-0035 — Length-and-Encoding-Prefixed String Memory Layout
- ADR-0016 — String and WSTRING Character Encoding (UTF-16LE for WSTRING)
- ADR-0017 — Unified Data Region for Variable-Length Types

## File Map

| File | Change |
|------|--------|
| `compiler/container/src/header.rs` | `STRING_HEADER_BYTES` 4 → 6; `FORMAT_VERSION` 2 → 3 |
| `compiler/container/src/constant_pool.rs` | Constant pool entries carry `char_width` (per-entry tag); on-disk layout adds 1 byte per string entry |
| `compiler/container/src/const_type.rs` | **Add `ConstType::WStr` variant** (locked decision; do not extend `Str`) |
| `compiler/container/src/spec_conformance.rs` | REQ-CF-003 / REQ-CF-004 expectations updated for format version 3 |
| `compiler/vm/src/string_ops.rs` | Header read/write helpers handle `char_width` field; `TempBufferSlot` carries encoding tag; `TempBufAllocator::alloc` records encoding |
| `compiler/vm/src/vm.rs` | Every STR_* opcode handler reads width from typed source, verifies match, scales offsets by width; delegates body to width-parameterized helpers |
| `compiler/vm/src/error.rs` | Add `Trap::EncodingMismatch` |
| `compiler/vm/resources/problem-codes.csv` | Add `V9014,EncodingMismatch,...` |
| `compiler/vm-cli/resources/problem-codes.csv` | Mirror of above |
| `compiler/problems/resources/problem-codes.csv` | Mirror of above |
| `docs/reference/runtime/problems/V9014.rst` | New "internal error / report as bug" page styled after `V9009.rst` |
| `compiler/analyzer/src/intermediate_type.rs` | `IntermediateType::String` gains `char_width: u8` field; update `size_in_bytes()` |
| `compiler/analyzer/src/intermediates/string.rs` | `from()` / `from_decl()` set `char_width` from `StringInitializer.width` / `StringDeclaration.width` |
| `compiler/codegen/src/compile.rs` | `STRING_CHAR_WIDTH` constant becomes `NARROW_CHAR_WIDTH = 1` and `WIDE_CHAR_WIDTH = 2`; `encode_string_literal` learns UTF-16LE for `char_width = 2`; `string_region_size(max_length, char_width)` scales by width |
| `compiler/codegen/src/compile_setup.rs` | Read width from `StringInitializer.width`; **set `char_width` directly into the data-region header at allocation time (no `STR_INIT` operand)**; `iec_type_tag::WSTRING` for wide strings (already handled) |
| `compiler/codegen/src/compile_expr.rs` | String literal width derived from operand context, not hardcoded |
| `compiler/codegen/src/compile_string.rs` | Same; comparison ops route width per operand |
| `compiler/codegen/src/compile_fn.rs` | `FunctionReturnType::WString` → `char_width = 2` for return slot; `StringParamInfo` construction sites at `compile_fn.rs:132, 197, 250, 256, 556` populate `char_width` from parameter type, not the hardcoded `STRING_CHAR_WIDTH` |
| `compiler/codegen/src/compile_array.rs` | `ArrayElementType::WString` sets element width to 2 |
| `compiler/codegen/src/compile_struct.rs` | (No change in scope: STRING fields in structs are already unsupported; WSTRING fields match.) |
| `compiler/vm/src/string_ops.rs` (tests) | Update fixtures from 4- to 6-byte headers |
| `compiler/codegen/tests/end_to_end_wstring.rs` | New: declaration, assignment, comparison, `LEN`, `CONCAT`, array-of-WSTRING, type-mismatch rejection |
| `compiler/codegen/tests/common/mod.rs` | Add helper to inspect WSTRING from VM buffers if needed |
| `compiler/plc2plc/resources/test/wstrings_rendered.st` | New: round-trip fixture mirroring `strings_rendered.st` (declaration, assignment, concat, LEN over WSTRING) |
| `specs/design/bytecode-container-format.md` | REQ-CF-003 description updated for version 3 |
| `specs/design/bytecode-instruction-set.md` | Prose references to `FORMAT_VERSION = 2` updated to 3 |
| `specs/design/vm-performance.md` | Add `char_width` field to documented string-header layout |

## Tasks

### Phase 0 — Plan and ADRs

- [x] ADR-0034 (supersedes ADR-0004)
- [x] ADR-0035 (supersedes ADR-0015)
- [ ] Commit this plan

### Phase 1 — Container format & VM layout helpers (single coherent PR; tree must stay green)

This phase merges what would otherwise be two phases. The constant bumps and the
helper updates ship together so `cargo test` passes after the commit.

- [ ] **Test-fixture sweep first**: grep for hardcoded `4` near string headers,
  hardcoded `2` near `format_version`, and any literal byte arrays representing
  string headers. Sites to update at minimum:
  - `compiler/container/src/header.rs:284, 300, 325, 333, 342` — `FORMAT_VERSION` test sites and byte-level `2u16` literals
  - `compiler/container/src/spec_conformance.rs:60` — rename `container_spec_req_cf_003_format_version_is_2` → `_is_3`
  - `compiler/container/src/spec_conformance.rs:74-75` — REQ-CF-004 byte assertion `&2u16.to_le_bytes()` → `&3u16.to_le_bytes()`
  - `compiler/vm/src/string_ops.rs` tests — convert 4-byte header fixtures to 6 bytes, add `char_width` field
- [ ] `STRING_HEADER_BYTES`: 4 → 6 in `compiler/container/src/header.rs`
- [ ] `FORMAT_VERSION`: 2 → 3 in `compiler/container/src/header.rs`
- [ ] Spec text: `specs/design/bytecode-container-format.md:65` — REQ-CF-003 description
  "currently 2; bumped from 1 by ADR-0033 …" → "currently 3; bumped from 2 by ADR-0035 …"
- [ ] Spec text: `specs/design/bytecode-instruction-set.md:111, 747` — prose reference to `FORMAT_VERSION = 2`
- [ ] Spec text: `specs/design/vm-performance.md:50-51, 89-90` — annotate `[max_length][cur_length][char_width]` layout
- [ ] `ConstType::WStr` variant added to `compiler/container/src/const_type.rs`
- [ ] Constant pool: each string entry carries `char_width`. Both the in-memory `ConstantPool` API and the on-disk per-entry byte layout add 1 byte for the encoding tag (Str → tag=1, WStr → tag=2). Wire-format change is part of FORMAT_VERSION 3.
- [ ] Update offset constants: `MAX_LEN_OFFSET = 0`, `CUR_LEN_OFFSET = 2`, `CHAR_WIDTH_OFFSET = 4`
- [ ] `read_string_header(buf, offset)` returns `(cur_len, data_start, char_width)` (or split into a dedicated `read_char_width` helper)
- [ ] `write_string_header(buf, buf_start, max_len, result_len, char_width)` writes the new field
- [ ] `str_read_char_width(buf, offset) -> u16` helper
- [ ] `TempBufferSlot` gains `encoding: u8`; `TempBufAllocator::alloc` accepts and records the encoding
- [ ] Run `cd compiler && just compile && cargo test -p ironplc-container -p ironplc-vm string_ops` — must pass before phase 2

### Phase 2 — VM string opcode handlers

- [ ] `Trap::EncodingMismatch` added to `compiler/vm/src/error.rs` with `Display` impl, plus `V9014` rows added to all three `problem-codes.csv` files
- [ ] **`STR_INIT`**: no width operand. Codegen writes `char_width` directly into the data-region header at allocation; the opcode just initializes `cur_length = 0`. Single source of truth: the slot's declared type at codegen time, materialized as the header's `char_width` byte at runtime.
- [ ] `LOAD_CONST_STR`: read width from constant pool entry; write into temp buffer header; tag temp buffer slot encoding
- [ ] `STR_STORE_VAR`: verify source temp buffer encoding matches dest header `char_width`; trap on mismatch; copy `cur_len * char_width` bytes
- [ ] `STR_LOAD_VAR`: read width from data region header; tag temp buffer slot; copy `cur_len * char_width` bytes
- [ ] `LEN_STR`: returns `cur_length` (unchanged — already in code units)
- [ ] `FIND_STR`, `REPLACE_STR`, `INSERT_STR`, `DELETE_STR`, `LEFT_STR`, `RIGHT_STR`, `MID_STR`, `CONCAT_STR`: read each source's `char_width`; verify all sources match; delegate to width-parameterized helper that scales byte offsets
- [ ] `STR_INIT_ARRAY`, `STR_LOAD_ARRAY_ELEM`, `STR_STORE_ARRAY_ELEM`: same width handling as scalar variants
- [ ] Helper functions extracted: `do_str_store_var(char_width, ...)`, `do_str_load_var(char_width, ...)`, `do_find_str(char_width, ...)`, etc., so the dispatch arms are one-liners
- [ ] Run `cargo test -p ironplc-vm` — must pass before phase 3

### Phase 3 — Analyzer width tracking

- [ ] `IntermediateType::String { max_len, char_width: u8 }` — add the field
- [ ] `intermediates/string.rs::from()` reads `StringInitializer.width` and sets `char_width`
- [ ] `intermediates/string.rs::from_decl()` reads `StringDeclaration.width` and sets `char_width`
- [ ] `IntermediateType::String::size_in_bytes()` returns `STRING_HEADER_BYTES + max_len * char_width`
- [ ] **Pattern-match ripple — exhaustive update list** (35 sites destructure `{ max_len }`, 15 use `{ .. }` and are safe). Production sites:
  - `compiler/analyzer/src/intermediate_type.rs:261, 385, 633` (production code)
  - `compiler/analyzer/src/type_environment.rs:205, 206` (built-in type registrations: STRING → `char_width: 1`, WSTRING → `char_width: 2`)
  - All test sites in `intermediate_type.rs` (lines `776, 780, 887, 935, 936, 1171, 2035, 2042, 2049`), `intermediates/string.rs` (lines `57, 80, 104`), `intermediates/array.rs` (lines `37, 535, 579`), `intermediates/structure.rs` (lines `810, 838, 864, 869`), `intermediates/subrange.rs` (lines `248, 472`), `type_environment.rs` (lines `596, 639`), `type_category.rs:66` — add `char_width: 1` (or `2` where the test name implies WSTRING)
- [ ] Update existing test `apply_when_wstring_type_declaration_then_creates_string_type` to assert `char_width == 2`
- [ ] Reject `STRING := WSTRING` and `WSTRING := STRING` in type checker (compile error, not runtime trap) — verify existing type-checking covers this; add explicit test if it does not
- [ ] Run `cargo test -p ironplc-analyzer` — must pass before phase 4

### Phase 4 — Codegen width emission

- [ ] `compile.rs`: `STRING_CHAR_WIDTH` removed; constants `NARROW_CHAR_WIDTH = 1`, `WIDE_CHAR_WIDTH = 2`
- [ ] `compile.rs::encode_string_literal(chars, char_width)`: `char_width = 2` arm encodes as UTF-16LE (`(ch as u16).to_le_bytes()` per char), no surrogate-pair handling — characters above U+FFFF are out of scope per ADR-0016
- [ ] `compile.rs::string_region_size(max_length, char_width)`: returns `STRING_HEADER_BYTES + max_length * char_width`
- [ ] `compile_setup.rs`: read `StringInitializer.width`, pass derived `char_width` to allocation; **write `char_width` directly into the data-region header at variable initialization** (no operand on `STR_INIT`). `iec_type_tag::WSTRING` is already wired (`compile_setup.rs:324`).
- [ ] `compile_expr.rs`, `compile_string.rs`: route width through string-literal encoding based on operand type (read from analyzer's `IntermediateType::String.char_width`)
- [ ] `compile_fn.rs`: `FunctionReturnType::WString` produces a return slot with `char_width = 2`. Function-parameter sites at `compile_fn.rs:132, 197, 250, 256, 556` (`StringParamInfo` construction) set `char_width` from the parameter's declared type; do not hardcode `STRING_CHAR_WIDTH`.
- [ ] `compile_array.rs`: `ArrayElementType::WString` produces array elements with `char_width = 2`
- [ ] Constant pool registration: pass `char_width` so each pool entry is tagged; choose `ConstType::Str` vs `ConstType::WStr` from operand context
- [ ] Run `cargo test -p ironplc-codegen` — must pass before phase 5

### Phase 5 — End-to-end tests

- [ ] `compiler/codegen/tests/end_to_end_wstring.rs` (new file):
  - WSTRING variable declaration with literal initializer; verify data region contains correct UTF-16LE bytes
  - WSTRING-to-WSTRING assignment
  - WSTRING comparison (`=`, `<>`)
  - `LEN(wstring_var)` returns code-unit count
  - `CONCAT(ws1, ws2)` produces correct UTF-16LE result
  - `LEFT`, `RIGHT`, `MID` index by code unit
  - `ARRAY[1..N] OF WSTRING[20]` declared and assigned
  - Mixed STRING + WSTRING in same program: independent, no interference
- [ ] Negative test: `STRING := WSTRING` rejected by analyzer (compile error)
- [ ] Synthetic-bytecode runtime test: hand-crafted bytecode that pairs a WSTRING-tagged constant with a STRING-tagged destination — confirm `Trap::EncodingMismatch` (defense-in-depth)
- [ ] plc2plc round-trip: add `compiler/plc2plc/resources/test/wstrings_rendered.st` mirroring `strings_rendered.st` but exercising WSTRING declarations, initializers, and operations. The renderer already handles WSTRING (`renderer.rs:101-104, 450-462, 700-727`); only the fixture is new.

### Phase 6 — Final CI and documentation

- [ ] `docs/reference/runtime/problems/V9014.rst` — new page in `V9009.rst` style: classifies `EncodingMismatch` as an internal error ("should not occur during normal operation; report as bug"), since with the analyzer rejecting cross-encoding assignments the trap is reachable only via compiler bugs, tampered `.iplc` files, or synthetic test bytecode.
- [ ] `docs/reference/runtime/problems/index.rst` — add `V9014` entry
- [ ] `cd compiler && just` — full pipeline (compile + coverage + lint) passes
- [ ] Verify clippy is clean (no `#[allow(...)]` workarounds)
- [ ] Verify coverage stays ≥ 85%

## Risks and Mitigations

| Risk | Mitigation |
|------|-----------|
| Test fixtures with hardcoded 4-byte string headers break across multiple crates | Phase 1 starts with an explicit fixture-sweep task before any constant bump; explicit `STRING_HEADER_BYTES` reference, not magic 4 |
| Encoding-mismatch trap fires unexpectedly (false positive) | Add unit test for each opcode that pairs equal-encoding operands and confirms no trap |
| Constant pool API change cascades into many call sites | Phase 1 keeps the change additive (default to `char_width = 1` for existing call sites, then update them per phase) |
| Analyzer change to `IntermediateType::String` ripples into pattern matches across the codebase | Phase 3 lists the 35 destructuring sites explicitly; the 15 `{ .. }` sites need no change |
| Format version bump breaks any existing test bytecode files | Phase 1 fixture sweep updates all `format_version = 2` literals to 3 |
| Two sources of width truth on `STR_INIT` | Resolved: the opcode does not take a width operand; codegen writes the width into the data-region header at allocation time, and runtime ops read it back from the header |
| ADR-0034 Layer 1 verifier missing | Out of scope; trap is the safety net. Analyzer-level rejection of cross-encoding ops is the static check we ship |

## Verification Strategy

- Each phase ends with the relevant test scope passing (`cargo test -p <crate>`)
- The full `cd compiler && just` pipeline runs at the end of Phase 6
- End-to-end tests in Phase 5 are the user-facing acceptance criteria
- The synthetic-mismatch bytecode test in Phase 5 is the safety acceptance criterion (defense-in-depth works)
