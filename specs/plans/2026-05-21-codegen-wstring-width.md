# Codegen WSTRING Width Plumbing Implementation Plan

**Goal:** Replace codegen's raw `u16` / `STRING_CHAR_WIDTH = 1` representation with a typed `CharWidth` carried on `StringVarInfo`, `StringParamInfo`, `StringReturnInfo`, and `ArraySpec`. Source the actual encoding from `StringDeclaration` / `StringInitializer` / `FunctionReturnType` / `ArrayElementType` so WSTRING declarations now flow through codegen with `CharWidth::Wide` instead of being silently collapsed to STRING. The container format is unchanged; the runtime data-region layout grows for WSTRING (header still 4 bytes, payload doubled) — safe because there are no WSTRING tests on main.

**Architecture:** Typed surface change in `compiler/codegen/`. The four metadata structs gain typed `CharWidth` fields (or change existing `u16` fields to `CharWidth`). A new `char_width_for_string_type(&StringType) -> CharWidth` helper maps DSL widths to the container enum. Construction sites in `compile_fn.rs`, `compile_setup.rs`, `compile_struct.rs`, and `compile_array.rs` read the width from the AST at declaration time. `encode_string_literal` retains its panic on `Wide` — string literals can't yet be encoded as UTF-16LE because the constant pool wire format doesn't carry the per-entry encoding tag.

**Context:** Third slice of PR #1050 (WSTRING support). Builds on the `CharWidth` enum (#1070) and the analyzer-side width tracking (#1073). Codegen now knows when a variable/return/parameter/array element is WSTRING vs STRING, but the bytecode it emits still uses STRING-shaped headers and Latin-1 payload — that flip happens with the container format bump.

**Tech Stack:** Rust, `ironplc-codegen` crate (already depends on `ironplc-container`)

---

### Task 1: Type up the char_width constants and helper

**Files:**
- Modify: `compiler/codegen/src/compile.rs`

Replace `pub(crate) const STRING_CHAR_WIDTH: u16 = 1` with two typed constants `NARROW_CHAR_WIDTH: CharWidth = CharWidth::Narrow` and `WIDE_CHAR_WIDTH: CharWidth = CharWidth::Wide`. Add a `pub(crate) fn char_width_for_string_type(width: &StringType) -> CharWidth` helper that maps `StringType::String` → `Narrow`, `StringType::WString` → `Wide`.

### Task 2: Change `char_width` field types to `CharWidth`

**Files:**
- Modify: `compiler/codegen/src/compile.rs`

`StringVarInfo`, `StringParamInfo`, and `StringReturnInfo` change `char_width: u16` to `char_width: CharWidth`. `encode_string_literal(chars, char_width: u16)` → `CharWidth` (kept panicking on `Wide` via match exhaustion). `string_region_size(max_length, char_width: u8)` → `CharWidth`; the body uses `char_width.as_usize() as u32`.

### Task 3: Source widths from declarations at construction sites

**Files:**
- Modify: `compiler/codegen/src/compile_fn.rs`
- Modify: `compiler/codegen/src/compile_setup.rs`

At each `InitialValueAssignmentKind::String(string_init)` arm, compute `let char_width = char_width_for_string_type(&string_init.width);` and pass it into `string_region_size` and the `StringVarInfo` / `StringParamInfo` it constructs. For function returns, match on `FunctionReturnType::String` → Narrow vs `WString` → Wide.

### Task 4: Add `string_char_width` to `ArraySpec` and populate it

**Files:**
- Modify: `compiler/codegen/src/compile_array.rs`

`ArraySpec` gains a `pub string_char_width: Option<CharWidth>` field next to the existing `string_max_len`. Populate it from `ArrayElementType::String → Some(Narrow)`, `ArrayElementType::WString → Some(Wide)`, `_ => None`. All construction sites updated.

### Task 5: Update remaining destructuring/construction sites

**Files:**
- Modify: `compiler/codegen/src/compile_struct.rs`

Test construction sites that currently pass `char_width: CharWidth::Narrow` may also need touching if they use the renamed constants. Verify all `STRING_CHAR_WIDTH` references compile.

---

## Deliberately out of scope

- `FORMAT_VERSION` bump, header layout change, `STRING_HEADER_BYTES` 4→6
- `encode_string_literal` actually producing UTF-16LE bytes for `Wide`
- Per-constant-pool-entry encoding tag in the container wire format
- VM-side encoding-mismatch traps (V9014/V9015)
- `iec_type_tag::WSTRING` in debug names being set from the new field
- End-to-end WSTRING tests

## Verification

`cd compiler && just` must pass: compile, coverage ≥ 85%, clippy, fmt, dupes. Existing STRING tests continue to work because their declarations resolve to `CharWidth::Narrow`, identical to the prior `STRING_CHAR_WIDTH = 1` value. There are no WSTRING tests on main, so the new behavior (WSTRING variables sized for 2-byte code units) doesn't trip any assertions.
