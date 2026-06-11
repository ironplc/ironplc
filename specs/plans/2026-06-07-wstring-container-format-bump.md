# Plan: PR B1 — WSTRING container format bump

## Context

This is **PR B1** in the WSTRING split of the (now stale) PR #1050. It
follows PR A (#1082, reserved traps V9014/V9015) and PR B0 (#1095, purely
additive `ConstType::WStr`, `ConstType::char_width()`, `is_string_like()`,
and the codegen UTF-16LE `encode_string_literal` arm). It is the single
**wire-format break** for ADR-0034/0035 WSTRING support, shipped in
isolation so a revert is trivial.

Design source: `specs/plans/2026-05-05-complete-wstring-support.md`
(Phase 1, plus the cross-crate fallout the format break causes).

ADRs: ADR-0034 (operand typing + encoding tags), ADR-0035 (length-and-
encoding-prefixed string layout), ADR-0016 (UTF-16LE for WSTRING).

## Scope and non-goals

In scope — the format shape only:

- String header grows `4 → 6` bytes; the new `u16` is the `char_width`
  field per ADR-0035 (`[max_length][cur_length][char_width]`).
- Container `FORMAT_VERSION` `2 → 3`.
- Constant-pool per-entry **reserved byte → `char_width` byte** on the wire.
- Analyzer `IntermediateType::String::size_in_bytes()` multiplies by
  `char_width` (inert at narrow, where it stays `× 1`).
- Regenerate the `steel_thread.iplc` golden at version 3.
- Bump `format_version == 2 → 3` assertions and 4-byte-header test
  fixtures across crates.
- Design-doc updates (REQ-CF-003 version, ConstEntry `char_width` row).

**Explicitly NOT in this PR** (deferred to PR C — "VM string opcodes with
encoding verification"):

- The VM does **not** populate or read the new `char_width` header field
  yet. Existing header writers (`str_write_header`, `write_string_header`)
  keep writing only `max_length` + `cur_length`; the new 2 bytes are
  reserved-zero in the data region / temp buffers. No `CHAR_WIDTH_OFFSET`
  constant, no `str_read_char_width`, no `TempBufferSlot.encoding`, no
  encoding-mismatch trapping. Those land in PR C.
- No codegen emits a wide string yet (PR D). Every string is still narrow,
  so behavior is byte-for-byte identical to today aside from the +2 header
  bytes and the version number.

The asymmetry is deliberate: the constant pool already *knows* each
entry's width (`ConstType::char_width()` from PR B0), so writing that byte
is free and additive. Populating the data-region header's width requires
threading width through `STR_INIT` operands and every opcode handler —
that is the bulk of PR C.

## Why this is (almost) a one-constant change

The VM already routes every string-header read, data-region span, and
temp-buffer span through `ironplc_container::STRING_HEADER_BYTES`
(`vm/src/string_ops.rs`, `vm/src/vm.rs`), and codegen already computes
`string_region_size(max_length, char_width) = STRING_HEADER_BYTES +
max_length * char_width.byte_width()` and has the UTF-16LE
`encode_string_literal` arm (PR B0). So bumping the constant flows through
allocation and layout automatically; the remaining work is the wire byte,
the version number, and stale 4-byte/`v2` test fixtures.

## File map

| File | Change |
|------|--------|
| `compiler/container/src/header.rs` | `FORMAT_VERSION` 2→3; `STRING_HEADER_BYTES` 4→6; doc the 3-field header layout |
| `compiler/container/src/constant_pool.rs` | `write_to`: reserved byte → `const_type.char_width()` byte; `read_from`: doc the field; new wire-byte test |
| `compiler/container/src/spec_conformance.rs` | REQ-CF-003 assert `== 3` (rename fn); REQ-CF-004 offset-4 bytes `3u16` |
| `compiler/analyzer/src/intermediate_type.rs` | `size_in_bytes` String arm `× char_width.byte_width()`; add a `Wide ⇒ 20` test |
| `compiler/vm/src/string_ops.rs` | Fixtures only (sizes ≥ 6); assertions already keyed to `STRING_HEADER_BYTES` |
| `compiler/project/src/disassemble.rs` | Two `formatVersion == 2 → 3` assertions |
| `compiler/playground/src/lib.rs` | `read_string_value` test fixture `data[4..9] → data[6..11]`; doc "4-byte → 6-byte header" |
| `compiler/codegen/tests/it/end_to_end_string.rs` | hardcoded `258` → `STRING_HEADER_BYTES + 254`; comment |
| `compiler/codegen/tests/it/end_to_end_replace.rs` | stale `258`/`516` comment → 6-byte math |
| `compiler/vm-cli/resources/test/steel_thread.iplc` | regenerate at version 3 |
| `specs/design/bytecode-container-format.md` | REQ-CF-003 "currently 2 → 3" (note ADR-0035); ConstEntry `reserved → char_width` row |

## Tasks

1. **Plan** — commit this file.
2. **Container format** — `header.rs` constants + doc; `constant_pool.rs`
   wire byte + test; `spec_conformance.rs` REQ-CF-003/004.
3. **Analyzer** — `size_in_bytes` `× char_width` + wide test.
4. **Cross-crate fixtures** — `string_ops.rs`, `disassemble.rs`,
   `playground/src/lib.rs`, `end_to_end_string.rs`,
   `end_to_end_replace.rs` comment.
5. **Golden** — regenerate `steel_thread.iplc` at v3 (temporarily wire
   `write_steel_thread_container` into the `generate_golden_files`
   ignored test, run it, revert the wiring).
6. **Docs** — `bytecode-container-format.md`.
7. **CI** — `cd compiler && just` green (compile, coverage ≥ 85%, lint).

## Verification

- Every string-header read/write in the VM stays `STRING_HEADER_BYTES`-
  relative, so allocation and execution of existing narrow-string tests
  must pass unchanged after the bump.
- The regenerated golden round-trips (`run_when_golden_container_file_then_ok`).
- Full `just` pipeline green before push.

## Risks

| Risk | Mitigation |
|------|-----------|
| Hardcoded 4-byte/`258`/`v2` fixtures across crates | Swept (`header.rs`, `string_ops.rs`, `playground`, `disassemble`, `end_to_end_string/replace`); each made `STRING_HEADER_BYTES`/`FORMAT_VERSION`-relative |
| Stale golden fails version check | Regenerated at v3 in this PR |
| Constant-pool reader assumed reserved-zero byte 1 | Round-trip tests don't inspect byte 1; `const_type` stays authoritative on read, so the now-meaningful byte is non-breaking |
| `size_in_bytes × char_width` ripples into data layout | Narrow `× 1` is a no-op today; value unchanged for every existing path |
