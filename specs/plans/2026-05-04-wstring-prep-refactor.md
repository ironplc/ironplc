# WSTRING Prep Refactor

## Goal

Land mechanical, behavior-preserving refactors that put width-dependent string logic behind clean seams, so a future WSTRING change becomes a value substitution rather than a structural rewrite.

This change does **not** add WSTRING support. It only prepares the codebase for it.

## Architecture

Currently STRING is byte-oriented throughout codegen and VM. The width assumption (`1 byte per character`) is hard-coded at many call sites. WSTRING (16-bit characters) will need to thread a `char_width` value through the same code paths.

The prep work consolidates each pattern at one site:

1. **String region sizing** — three call sites compute `STRING_HEADER_BYTES + max_length` inline. Replace with a `string_region_size(max_length)` helper.
2. **String literal encoding** — four call sites do `lit.value.iter().map(|&ch| ch as u8).collect()`. Replace with an `encode_string_literal(chars, char_width)` helper that today only supports `char_width = 1`.
3. **String info structs** — `StringVarInfo`, `StringParamInfo`, `StringReturnInfo` carry `data_offset` and `max_length`. Add a `char_width: u16` field (always `1` today) so the value flows through codegen without further plumbing later.
4. **VM string-header offsets** — `string_ops.rs` uses magic `+ 2`, `+ 3` to access the `cur_length` field. Replace with named constants (`MAX_LEN_OFFSET`, `CUR_LEN_OFFSET`).

Each refactor is a 1:1 substitution preserving exact byte output and runtime behavior.

## File Map

Modified:

- `compiler/codegen/src/compile.rs` — define the helpers and add `char_width` to the three info structs.
- `compiler/codegen/src/compile_string.rs` — call `string_region_size` and `encode_string_literal`.
- `compiler/codegen/src/compile_expr.rs` — call `encode_string_literal`.
- `compiler/codegen/src/compile_setup.rs` — call `encode_string_literal`; populate `char_width` at struct construction.
- `compiler/codegen/src/compile_fn.rs` — populate `char_width` at struct construction.
- `compiler/vm/src/string_ops.rs` — introduce named header offset constants.

Not modified: parser, analyzer, opcodes, container format, plc2plc.

## Tasks

- [ ] Add `char_width: u16` to `StringVarInfo`, `StringParamInfo`, `StringReturnInfo` and populate at all construction sites with `1`.
- [ ] Add `string_region_size(max_length: u16) -> u32` helper in `compile.rs` and use at three call sites in `compile_string.rs`.
- [ ] Add `encode_string_literal(chars: &[char], char_width: u16) -> Vec<u8>` helper in `compile.rs` and use at four call sites.
- [ ] Add `MAX_LEN_OFFSET`, `CUR_LEN_OFFSET` constants in `string_ops.rs` and use them in place of magic offsets.
- [ ] Run `cd compiler && just` and verify all checks pass.

## Verification

Behavior is preserved iff:

- All existing tests pass.
- Coverage stays at or above the prior baseline (no untested branches added).
- `cargo clippy` and `cargo fmt` pass.

No new tests are required — these are internal refactors with no observable behavior change.
