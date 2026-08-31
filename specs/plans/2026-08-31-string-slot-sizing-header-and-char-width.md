# String slot sizing: correct header size and honour `char_width`

Fixes [#1521](https://github.com/ironplc/ironplc/issues/1521).

## Goal

Make `IntermediateType::slot_count()` size STRING/WSTRING values from the
real data-region layout: the 6-byte ADR-0035 header
(`[max_length:u16][cur_length:u16][char_width:u16]`) plus
`max_length * char_width` payload bytes. Today it hardcodes a 4-byte header
and discards `char_width`, so a `WSTRING[n]` is under-allocated by `n + 2`
bytes the moment strings become legal struct/array-of-struct fields.

## Architecture

The layout formula already exists — `compile::string_region_size()` in the
codegen crate — but the analyzer cannot reach it (codegen depends on the
analyzer, not the other way round), so `slot_count()` grew its own copy that
then drifted. The fix is to move the formula down to `ironplc-container`,
the crate that already owns `STRING_HEADER_BYTES` and `CharWidth` and that
both the analyzer and codegen depend on. `slot_count()` then becomes
`string_region_size(...).div_ceil(8)` with no arithmetic of its own, and
there is one definition of the layout for the whole compiler.

The default maximum length (254, IEC 61131-3 §2.3.3) is duplicated the same
way — a `u128` constant in the analyzer, a `u16` constant in codegen, and
three bare `254` literals in `compile_struct.rs`. It moves next to the
formula.

## Prefactoring

Two behaviour-preserving moves, committed separately, before the fix:

1. Extract `compiler/container/src/string_layout.rs` owning the string
   data-region layout: `STRING_HEADER_BYTES` (moved out of `header.rs`,
   which describes the 256-byte *file* header and is a different concept),
   `DEFAULT_STRING_MAX_LENGTH`, and `string_region_size()`. Public paths
   (`ironplc_container::STRING_HEADER_BYTES`) are unchanged.
2. Point codegen at the container definitions and delete its local copies,
   including the bare `254` literals.

With that in place the actual fix is a four-line change to one match arm,
which is the point.

## Design doc reference

- ADR-0035 — string header carries `char_width`
- `specs/design/vm-performance.md` — verifier bound and copy loop, both of
  which state the pre-ADR-0035 formula and are corrected here

## File map

| File | Change |
| --- | --- |
| `compiler/container/src/string_layout.rs` | New. `STRING_HEADER_BYTES`, `DEFAULT_STRING_MAX_LENGTH`, `string_region_size()` + tests |
| `compiler/container/src/header.rs` | Remove `STRING_HEADER_BYTES` (moved) |
| `compiler/container/src/lib.rs` | Declare and re-export `string_layout` |
| `compiler/codegen/src/compile.rs` | Delete local `string_region_size` / `DEFAULT_STRING_MAX_LENGTH_U16`; re-export container's |
| `compiler/codegen/src/compile_struct.rs` | Replace bare `254` literals with `DEFAULT_STRING_MAX_LENGTH` |
| `compiler/analyzer/src/intermediate_type.rs` | Fix the `String` arm of `slot_count_inner`; fix the three tests that encode 4-byte arithmetic |
| `specs/design/vm-performance.md` | Verifier bound and copy loop scale by `char_width` |

## Tasks

- [ ] Commit the plan
- [ ] Prefactor 1: extract `string_layout` module in `ironplc-container`
- [ ] Prefactor 2: codegen uses the container definitions
- [ ] Fix `slot_count_inner`'s `String` arm to use `string_region_size`
- [ ] Correct the three `slot_count` string tests; add WSTRING coverage
- [ ] Correct `specs/design/vm-performance.md`
- [ ] `cd compiler && just`
- [ ] `git rm` the plan
