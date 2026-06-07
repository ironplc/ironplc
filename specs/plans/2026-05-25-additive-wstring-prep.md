# Pre-Format-Bump Additive WSTRING Prep Implementation Plan

**Goal:** Land three purely-additive WSTRING-related changes that do not depend on the `STRING_HEADER_BYTES` 4 → 6 / `FORMAT_VERSION` 2 → 3 bump. Each change moves code into its final shape so the upcoming format-bump PR can focus exclusively on the wire-format break.

**Architecture:** Three independent additive moves:

1. The analyzer keeps its local `char_width_for(&StringType) -> CharWidth` helper, which lives where both `StringType` (`ironplc-dsl`) and `CharWidth` (`ironplc-container`) are already in scope. `ironplc-dsl` does **not** depend on `ironplc-container`: the wire-format crate is a lower layer than the syntax tree, so the syntax tree must not depend on it.
2. A `ConstType::WStr = 7` enum variant plus a `ConstEntry::wstring()` constructor, a `ConstType::char_width()` accessor, and read/write/`get_str` support that treats `WStr` the same as `Str` on the wire. The variant is reachable but no codegen path produces one yet, so no v2 file ever contains it — the format version does not move.
3. The `Wide` arm of `encode_string_literal` is implemented as UTF-16LE (`(ch as u16).to_le_bytes()` per char, no surrogate-pair handling per ADR-0016 BMP scope) instead of `unreachable!`. No caller passes `Wide` yet, so the path is dead until the format-bump PR.

## Design Doc Reference

- ADR-0016 — String and WSTRING Character Encoding (UTF-16LE for WSTRING, BMP only)
- ADR-0034 — STRING/WSTRING Distinction via Operand Typing and Runtime Encoding Tags (motivates `ConstType::WStr` carrying encoding via the type tag)

## File Map

| File | Change |
|------|--------|
| `compiler/analyzer/src/intermediates/string.rs` | Keep the local `char_width_for(&StringType) -> CharWidth` helper (no dsl→container dependency) |
| `compiler/container/src/const_type.rs` | Add `WStr = 7` variant; `from_u8` and `as_str` arms; new `char_width(&self) -> Option<CharWidth>` accessor |
| `compiler/container/src/constant_pool.rs` | Add `ConstEntry::wstring()` constructor; extend `bytes()` and `get_str()` and `read_from()` to treat `WStr` like `Str`; update `primitive_le` debug-assert |
| `compiler/codegen/src/compile.rs` | Implement the `Wide` arm of `encode_string_literal` as UTF-16LE; drop the `unreachable!` |

## Tasks

- [ ] Keep `analyzer/intermediates/string.rs` `char_width_for` helper; no dsl→container dependency
- [ ] Add `ConstType::WStr = 7` variant and its `from_u8`/`as_str` arms
- [ ] Add `ConstType::char_width(&self) -> Option<CharWidth>` accessor
- [ ] Add `ConstEntry::wstring(bytes)` constructor
- [ ] Extend `ConstEntry::bytes()` to return `&self.str_value` for `WStr`
- [ ] Extend `ConstantPool::get_str()` to accept both `Str` and `WStr`
- [ ] Extend `ConstantPool::read_from()` to treat `WStr` like `Str`
- [ ] Update `primitive_le` debug-assert to exclude `WStr`
- [ ] Implement `encode_string_literal` `Wide` arm as UTF-16LE
- [ ] Add unit tests for each new code path
- [ ] `cd compiler && just` — full CI green

## Out of Scope

- `STRING_HEADER_BYTES` 4 → 6 / `FORMAT_VERSION` 2 → 3 bump (format-bump PR)
- The reserved-byte → `char_width` byte change in the constant-pool wire layout (format-bump PR — `WStr` here uses the type tag to carry encoding, not the reserved byte)
- Wiring `ConstEntry::wstring()` into codegen (downstream PR)
- Calling `encode_string_literal` with `Wide` from codegen (downstream PR)

## Risks and Mitigations

| Risk | Mitigation |
|------|-----------|
| `ConstType::WStr` written by a future codegen path with no analyzer fence-post check could end up in a v2 container | No code path emits `WStr` in this PR; the codegen-side gate lands in the format-bump PR where `STR_INIT` learns to carry width |
| Existing `ConstEntry::primitive_le` debug-assert that excludes `Str` needs to also exclude `WStr` | Updated explicitly; unit test exercises the wstring constructor path |

## Verification Strategy

- `cargo test -p ironplc-dsl`, `-p ironplc-container`, `-p ironplc-analyzer`, `-p ironplc-codegen` — all pass
- `cd compiler && just` — full pipeline green
