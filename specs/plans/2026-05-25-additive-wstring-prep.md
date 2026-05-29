# Pre-Format-Bump Additive WSTRING Prep Implementation Plan

**Goal:** Land three purely-additive WSTRING-related changes that do not depend on the `STRING_HEADER_BYTES` 4 → 6 / `FORMAT_VERSION` 2 → 3 bump. Each change moves code into its final shape so the upcoming format-bump PR can focus exclusively on the wire-format break.

**Architecture:** Three independent additive moves:

1. A `StringType::char_width()` method on `ironplc_dsl::common::StringType` returning the `CharWidth` from `ironplc-container`. Lets the analyzer drop its local `char_width_for` helper. Adds an `ironplc-container` dependency on `ironplc-dsl`.
2. A `ConstType::WStr = 7` enum variant plus a `ConstEntry::wstring()` constructor, a `ConstType::char_width()` accessor, and read/write/`get_str` support that treats `WStr` the same as `Str` on the wire. The variant is reachable but no codegen path produces one yet, so no v2 file ever contains it — the format version does not move.
3. The `Wide` arm of `encode_string_literal` is implemented as UTF-16LE (`(ch as u16).to_le_bytes()` per char, no surrogate-pair handling per ADR-0016 BMP scope) instead of `unreachable!`. No caller passes `Wide` yet, so the path is dead until the format-bump PR.

## Design Doc Reference

- ADR-0016 — String and WSTRING Character Encoding (UTF-16LE for WSTRING, BMP only)
- ADR-0034 — STRING/WSTRING Distinction via Operand Typing and Runtime Encoding Tags (motivates `ConstType::WStr` carrying encoding via the type tag)

## File Map

| File | Change |
|------|--------|
| `compiler/dsl/Cargo.toml` | Add `ironplc-container` dependency |
| `compiler/dsl/src/common.rs` | Add `impl StringType { pub fn char_width(&self) -> CharWidth }` |
| `compiler/analyzer/src/intermediates/string.rs` | Replace local `char_width_for` helper with `width.char_width()` |
| `compiler/container/src/const_type.rs` | Add `WStr = 7` variant; `from_u8` and `as_str` arms; new `char_width(&self) -> Option<CharWidth>` accessor |
| `compiler/container/src/constant_pool.rs` | Add `ConstEntry::wstring()` constructor; extend `bytes()` and `get_str()` and `read_from()` to treat `WStr` like `Str`; update `primitive_le` debug-assert |
| `compiler/codegen/src/compile.rs` | Implement the `Wide` arm of `encode_string_literal` as UTF-16LE; drop the `unreachable!` |

## Tasks

- [ ] Add `ironplc-container` dependency to `compiler/dsl/Cargo.toml`
- [ ] Add `StringType::char_width()` method
- [ ] Switch `analyzer/intermediates/string.rs` to use the new method; remove `char_width_for`
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
| Adding `ironplc-container` as a `dsl` dep creates a tighter coupling | `dsl` already depends on `ironplc-problems`; the container crate is `no_std`-friendly and the only borrowed type is the small `CharWidth` enum |
| Existing `ConstEntry::primitive_le` debug-assert that excludes `Str` needs to also exclude `WStr` | Updated explicitly; unit test exercises the wstring constructor path |

## Verification Strategy

- `cargo test -p ironplc-dsl`, `-p ironplc-container`, `-p ironplc-analyzer`, `-p ironplc-codegen` — all pass
- `cd compiler && just` — full pipeline green
