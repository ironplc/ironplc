# Plan: Finish end-to-end WSTRING (UTF-16LE) support

## Goal

Complete the WSTRING implementation so a hand-written `.st` program can declare
a WSTRING, assign a `"…"` literal, compare, take `LEN`, `CONCAT`, and use
`LEFT/RIGHT/MID/FIND`, plus `ARRAY[1..N] OF WSTRING[k]`, and have it **compile
and execute** on the VM with correct UTF-16LE bytes and code-unit lengths.
`STRING := WSTRING` must be a compile error. Narrow STRING behavior stays
byte-identical.

This is "PR D/E" of the multi-PR WSTRING effort. PR C (VM data-driven width +
encoding verification) is merged on `main`; codegen has width-aware allocation
and tags but never tells the VM the width, so every declared string's header is
written `char_width = 1`.

## Design references

- ADR-0034 (operand typing + runtime encoding tags), ADR-0035 (6-byte
  length+encoding header), ADR-0016 (UTF-16LE for WSTRING).
- `specs/plans/2026-05-05-complete-wstring-support.md` (Phases 4–7).
- `specs/plans/2026-06-07-wstring-vm-encoding-verification.md` (PR C, the
  narrow-only base this builds on).

## Format version decision

`STR_INIT` gains a `char_width` operand (7→8 bytes), a wire-format change. Per
the user's direction this **completes the v3 WSTRING format** that ADR-0035
originally specified (the master plan folded the `STR_INIT` operand into the
single 2→3 bump; the PR split merely landed v3 incrementally). **No
`FORMAT_VERSION` bump.** The frozen golden `.iplc` fixtures contain no strings,
so they remain valid v3 and are not regenerated.

## The keystone gap

`STR_INIT` carries no `char_width` operand, so the VM writes `char_width = 1`
into every declared string header — including WSTRING vars, which already get a
wide-sized data region. The first wide store then traps `EncodingMismatch`
(dest header narrow, wide temp wide), or silently writes narrow bytes into a
wide region. Fix: thread the compiler-known width through `STR_INIT` into the
header.

## File map

| File | Change |
|------|--------|
| `compiler/container/src/opcode.rs` | `instruction_size(STR_INIT)` 7 → 8 |
| `compiler/vm/src/vm.rs` | `STR_INIT` reads+validates `char_width` operand, writes it into header. `STR_INIT_ARRAY`/`STR_LOAD_ARRAY_ELEM`/`STR_STORE_ARRAY_ELEM` derive element width from `desc.element_type` (`FieldType::WString`), scale stride by width |
| `compiler/codegen/src/emit.rs` | `emit_str_init` takes `char_width`, emits the byte |
| `compiler/codegen/src/compile.rs` | `PoolConstant::WStr`; `add_wstr_constant`; builder mapping; `emit_string_literal_load(emitter, ctx, chars, char_width)` helper; `ctx.has_wide_string` flag; `max_temp_buf_bytes` sized wide when any wide string present |
| `compiler/codegen/src/compile_setup.rs` | `emit_str_init` call sites pass `info.char_width`; scalar + array initializer literals encoded at target width; set `has_wide_string` |
| `compiler/codegen/src/compile_string.rs` | `resolve_string_arg` temp-slot `emit_str_init` calls pass `NARROW_CHAR_WIDTH` |
| `compiler/codegen/src/compile_stmt.rs` | scalar + array-element string-literal assignment encoded at target width |
| `compiler/codegen/src/compile_array.rs` | set `has_wide_string` for wide element arrays; `ArrayVarInfo` carries element `char_width` |
| `compiler/codegen/src/optimize.rs` (test) | `str_init` helper → 8 bytes |
| `compiler/codegen/tests/it/common/mod.rs` | `bc::str_init` → 8 bytes |
| `compiler/codegen/tests/it/wire_format.rs` | `STR_INIT` now 8-byte shape |
| `compiler/vm/tests/it/execute_string_ops.rs` | synthetic `STR_INIT` bytecode adds the `char_width` byte |
| `compiler/analyzer/...` | confirm/add `STRING := WSTRING` compile-time rejection (Phase 4) |
| `compiler/codegen/tests/it/end_to_end_wstring.rs` (new) | acceptance tests |
| `compiler/codegen/tests/it/main.rs` | register `mod end_to_end_wstring` |
| `compiler/resources/test/`, `compiler/plc2plc/resources/test/`, `compiler/plc2plc/src/tests.rs` | WSTRING round-trip fixture |

## Tasks

### Phase A — Plan
- [ ] Commit this plan.

### Phase B — STR_INIT char_width operand (keystone)
- [ ] `instruction_size(STR_INIT)` 7 → 8.
- [ ] `emit_str_init(data_offset, max_length, char_width)` emits the byte.
- [ ] VM `STR_INIT`: read operand, `CharWidth::from_u8` (trap `InvalidCharWidth`),
      write into header.
- [ ] Update emit call sites (compile_setup ×3, compile_string narrow temps).
- [ ] Update test encoders: `optimize.rs::str_init`, `bc::str_init`,
      `wire_format.rs`, `execute_string_ops.rs` synthetic bytecode.
- [ ] `cargo test -p ironplc-vm`, `-p ironplc-container` green.

### Phase C — scalar string-literal width
- [ ] `PoolConstant::WStr` + `add_wstr_constant` + builder mapping.
- [ ] `emit_string_literal_load` helper (wide → `add_wstr_constant`/UTF-16LE).
- [ ] Scalar string assignment + initializer literals use target width.
- [ ] `has_wide_string`; `max_temp_buf_bytes` wide-sized when set.

### Phase D — array wide stride + element-literal width
- [ ] VM array handlers derive element width from `desc.element_type`, scale
      stride, write headers with width.
- [ ] Array initializer + element-assignment literals use element width.

### Phase E — analyzer compile-time rejection (Phase 4)
- [ ] Verify `STRING := WSTRING` / `WSTRING := STRING` (and cross-encoding
      compares/args) are rejected at compile time. If missing, add a rule with a
      documented problem code (`docs/compiler/problems/P####.rst`) + tests.

### Phase F — end-to-end tests (acceptance)
- [ ] `tests/it/end_to_end_wstring.rs`: declaration+literal (UTF-16LE bytes,
      `char_width = 2`); WSTRING↔WSTRING assignment; `=`/`<>`; `LEN` (code
      units); `CONCAT`; `LEFT/RIGHT/MID/FIND`; `ARRAY[1..N] OF WSTRING[k]`;
      mixed STRING+WSTRING independence; negative `STRING := WSTRING`.

### Phase G — plc2plc round-trip
- [ ] WSTRING declaration+operation fixture; parse → render → compare.

### Phase H — CI
- [ ] `cd compiler && just` green (compile, coverage ≥ 85%, clippy, fmt).

## Hard constraints
- Narrow STRING behavior byte-identical (`char_width = 1` ⇒ old byte math).
- VM and codegen `STR_INIT` changes land together; never leave emitted bytecode
  and the VM disagreeing on `STR_INIT` length.
- BDD test names; ≤ 1000 lines/module; problem codes documented; no hand-edited
  auto-managed version numbers.

## Out of scope (inherited)
- WSTRING struct fields; surrogate pairs (> U+FFFF); WSTRING literals as
  function-call arguments / comparison operands (function-arg literal width is
  narrow-only; tests use WSTRING variables for these).
