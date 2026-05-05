# Plan: Complete WSTRING Support

## Goal

Finish the WSTRING (UTF-16LE) implementation that is currently parsed but not realized in codegen or VM. After this change, an IEC 61131-3 program can declare, assign, compare, and call standard string functions on WSTRING variables, with the same correctness, safety, and performance guarantees that STRING enjoys today.

## Architecture

Per ADR-0034 (supersedes ADR-0004) and ADR-0035 (supersedes ADR-0015):

- **Single STR_\* opcode family** for both STRING and WSTRING. Width is data-driven, not opcode-driven. No new opcodes.
- **6-byte string header** `[max_length: u16][cur_length: u16][char_width: u16]`. Lengths stay in code units; byte spans are computed as `length * char_width`. Format version bumps 2 → 3.
- **Three runtime encoding tag sites** (defense-in-depth): string header (data region), per-entry tag in the constant pool, per-slot tag in the temp buffer pool.
- **Width-parameterized helpers.** Each string operation has one body parameterized by `char_width: usize`. STR_* opcode handlers stay short.
- **Static verification** via operand-source-typing: each operand resolves to a typed source (slot table, pool entry, temp buffer slot). Local lookup per opcode — no abstract interpretation, no eBPF-style verifier complexity.
- **Encoding mismatch traps.** Every string op compares expected to actual encoding and traps on mismatch. The runtime check is mandatory.

## Design Doc Reference

- ADR-0034 — STRING/WSTRING Distinction via Operand Typing and Runtime Encoding Tags
- ADR-0035 — Length-and-Encoding-Prefixed String Memory Layout
- ADR-0016 — String and WSTRING Character Encoding (UTF-16LE for WSTRING)
- ADR-0017 — Unified Data Region for Variable-Length Types

## File Map

| File | Change |
|------|--------|
| `compiler/container/src/header.rs` | `STRING_HEADER_BYTES` 4 → 6; `FORMAT_VERSION` 2 → 3 |
| `compiler/container/src/constant_pool.rs` | Constant pool entries carry `char_width` (per-entry tag) |
| `compiler/container/src/const_type.rs` | Either extend `ConstType::Str` semantics or add `ConstType::WStr` |
| `compiler/vm/src/string_ops.rs` | Header read/write helpers handle `char_width` field; `TempBufferSlot` carries encoding tag; `TempBufAllocator::alloc` records encoding |
| `compiler/vm/src/vm.rs` | Every STR_* opcode handler reads width from typed source, verifies match, scales offsets by width; delegates body to width-parameterized helpers |
| `compiler/vm/src/error.rs` | Add `Trap::EncodingMismatch` |
| `compiler/analyzer/src/intermediate_type.rs` | `IntermediateType::String` gains `char_width: u8` field; update `size_in_bytes()` |
| `compiler/analyzer/src/intermediates/string.rs` | `from()` / `from_decl()` set `char_width` from `StringInitializer.width` / `StringDeclaration.width` |
| `compiler/codegen/src/compile.rs` | `STRING_CHAR_WIDTH` constant becomes `NARROW_CHAR_WIDTH = 1` and `WIDE_CHAR_WIDTH = 2`; `encode_string_literal` learns UTF-16LE for `char_width = 2`; `string_region_size(max_length, char_width)` scales by width |
| `compiler/codegen/src/compile_setup.rs` | Read width from `StringInitializer.width`; emit `STR_INIT` with `char_width` operand; set `iec_type_tag::WSTRING` for wide strings |
| `compiler/codegen/src/compile_expr.rs` | String literal width derived from operand context, not hardcoded |
| `compiler/codegen/src/compile_string.rs` | Same; comparison ops route width per operand |
| `compiler/codegen/src/compile_fn.rs` | `FunctionReturnType::WString` → set `char_width = 2` for return slot |
| `compiler/codegen/src/compile_array.rs` | `ArrayElementType::WString` sets element width to 2 |
| `compiler/codegen/src/compile_struct.rs` | (No change in scope: STRING fields in structs are already unsupported; WSTRING fields match.) |
| `compiler/vm/src/string_ops.rs` (tests) | Update fixtures from 4- to 6-byte headers |
| `compiler/codegen/tests/end_to_end_wstring.rs` | New: declaration, assignment, comparison, `LEN`, `CONCAT`, array-of-WSTRING, type-mismatch rejection |
| `compiler/codegen/tests/common/mod.rs` | Add helper to inspect WSTRING from VM buffers if needed |

## Tasks

### Phase 0 — Plan and ADRs

- [x] ADR-0034 (supersedes ADR-0004)
- [x] ADR-0035 (supersedes ADR-0015)
- [ ] Commit this plan

### Phase 1 — Container format changes

- [ ] `STRING_HEADER_BYTES`: 4 → 6 in `compiler/container/src/header.rs`
- [ ] `FORMAT_VERSION`: 2 → 3 in `compiler/container/src/header.rs`
- [ ] Constant pool: each string entry carries `char_width` (chosen mechanism: extend `ConstantPool` API to accept a width on registration, store alongside bytes)
- [ ] Update spec conformance tests for the new header size and format version
- [ ] Run `cd compiler && just compile` — should succeed; tests will fail in next phases until VM helpers catch up

### Phase 2 — VM string layout helpers

- [ ] Update offset constants: `MAX_LEN_OFFSET = 0`, `CUR_LEN_OFFSET = 2`, `CHAR_WIDTH_OFFSET = 4`
- [ ] `read_string_header(buf, offset)` returns `(cur_len, data_start, char_width)` (or split into a dedicated `read_char_width` helper)
- [ ] `write_string_header(buf, buf_start, max_len, result_len, char_width)` writes the new field
- [ ] `str_read_char_width(buf, offset) -> u16` helper
- [ ] `TempBufferSlot` gains `encoding: u8`; `TempBufAllocator::alloc` accepts and records the encoding
- [ ] Update `string_ops.rs` tests to use 6-byte fixtures
- [ ] Run `cargo test -p ironplc-vm string_ops` — must pass before phase 3

### Phase 3 — VM string opcode handlers

- [ ] `Trap::EncodingMismatch` added to `compiler/vm/src/error.rs`
- [ ] `STR_INIT`: gains `char_width: u16` operand; writes width into header
- [ ] `LOAD_CONST_STR`: read width from constant pool entry; write into temp buffer header; tag temp buffer slot encoding
- [ ] `STR_STORE_VAR`: verify source temp buffer encoding matches dest header `char_width`; trap on mismatch; copy `cur_len * char_width` bytes
- [ ] `STR_LOAD_VAR`: read width from data region header; tag temp buffer slot; copy `cur_len * char_width` bytes
- [ ] `LEN_STR`: returns `cur_length` (unchanged — already in code units)
- [ ] `FIND_STR`, `REPLACE_STR`, `INSERT_STR`, `DELETE_STR`, `LEFT_STR`, `RIGHT_STR`, `MID_STR`, `CONCAT_STR`: read each source's `char_width`; verify all sources match; delegate to width-parameterized helper that scales byte offsets
- [ ] `STR_INIT_ARRAY`, `STR_LOAD_ARRAY_ELEM`, `STR_STORE_ARRAY_ELEM`: same width handling as scalar variants
- [ ] Helper functions extracted: `do_str_store_var(char_width, ...)`, `do_str_load_var(char_width, ...)`, `do_find_str(char_width, ...)`, etc., so the dispatch arms are one-liners
- [ ] Run `cargo test -p ironplc-vm` — must pass before phase 4

### Phase 4 — Analyzer width tracking

- [ ] `IntermediateType::String { max_len, char_width: u8 }` — add the field
- [ ] `intermediates/string.rs::from()` reads `StringInitializer.width` and sets `char_width`
- [ ] `intermediates/string.rs::from_decl()` reads `StringDeclaration.width` and sets `char_width`
- [ ] `IntermediateType::String::size_in_bytes()` returns `STRING_HEADER_BYTES + max_len * char_width`
- [ ] Update existing test `apply_when_wstring_type_declaration_then_creates_string_type` to assert `char_width == 2`
- [ ] Reject `STRING := WSTRING` and `WSTRING := STRING` in type checker (compile error, not runtime trap) — verify existing type-checking covers this; add explicit test if it does not
- [ ] Run `cargo test -p ironplc-analyzer` — must pass before phase 5

### Phase 5 — Codegen width emission

- [ ] `compile.rs`: `STRING_CHAR_WIDTH` removed; constants `NARROW_CHAR_WIDTH = 1`, `WIDE_CHAR_WIDTH = 2`
- [ ] `compile.rs::encode_string_literal(chars, char_width)`: `char_width = 2` arm encodes as UTF-16LE (`(ch as u16).to_le_bytes()` per char), no surrogate-pair handling — characters above U+FFFF are out of scope per ADR-0016
- [ ] `compile.rs::string_region_size(max_length, char_width)`: returns `STRING_HEADER_BYTES + max_length * char_width`
- [ ] `compile_setup.rs`: read `StringInitializer.width`, pass derived `char_width` to allocation, emit `STR_INIT` with width operand, set `iec_type_tag::WSTRING` for wide strings
- [ ] `compile_expr.rs`, `compile_string.rs`: route width through string-literal encoding based on operand type (read from analyzer's `IntermediateType::String.char_width`)
- [ ] `compile_fn.rs`: `FunctionReturnType::WString` produces a return slot with `char_width = 2`
- [ ] `compile_array.rs`: `ArrayElementType::WString` produces array elements with `char_width = 2`
- [ ] Constant pool registration: pass `char_width` so each pool entry is tagged
- [ ] Run `cargo test -p ironplc-codegen` — must pass before phase 6

### Phase 6 — End-to-end tests

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
- [ ] plc2plc round-trip test for a program using WSTRING (if `plc2plc` already renders WSTRING declarations, just add the resource files; otherwise a separate plan task)

### Phase 7 — Final CI

- [ ] `cd compiler && just` — full pipeline (compile + coverage + lint) passes
- [ ] Verify clippy is clean (no `#[allow(...)]` workarounds)
- [ ] Verify coverage stays ≥ 85%

## Out of Scope

- WSTRING in struct fields (matches existing STRING-in-struct status: not supported by codegen). Tracked separately if needed.
- Surrogate-pair-aware character iteration (code points above U+FFFF). ADR-0016 explicitly limits WSTRING to BMP semantics; this plan inherits that scope.
- BUILTIN-dispatch reorganization for string functions (ADR-0004 originally specified, current implementation uses dedicated opcodes; reconciling the two is a separate concern).
- Migration of any deployed bytecode at format_version 2. ADR-0015 was `proposed` with no deployed users.

## Risks and Mitigations

| Risk | Mitigation |
|------|-----------|
| Test fixtures with hardcoded 4-byte string headers break across multiple crates | Phase 1 + 2 search-and-replace pass before any opcode logic changes; explicit `STRING_HEADER_BYTES` reference, not magic 4 |
| Encoding-mismatch trap fires unexpectedly (false positive) | Add unit test for each opcode that pairs equal-encoding operands and confirms no trap |
| Constant pool API change cascades into many call sites | Phase 1 keeps the change additive (default to `char_width = 1` for existing call sites, then update them per phase) |
| Analyzer change to `IntermediateType::String` ripples into pattern matches across the codebase | Compiler errors will pinpoint each one; fix in Phase 4 before moving on |
| Format version bump breaks any existing test bytecode files | Search for hardcoded format version `2` in tests; update to `3` |

## Verification Strategy

- Each phase ends with the relevant test scope passing (`cargo test -p <crate>`)
- The full `cd compiler && just` pipeline runs at the end of Phase 7
- End-to-end tests in Phase 6 are the user-facing acceptance criteria
- The synthetic-mismatch bytecode test in Phase 6 is the safety acceptance criterion (defense-in-depth works)
