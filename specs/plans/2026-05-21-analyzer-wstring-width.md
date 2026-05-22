# Analyzer WSTRING Width Tracking Implementation Plan

**Goal:** Plumb a typed `char_width: CharWidth` field through `IntermediateType::String` so the analyzer can distinguish STRING (Latin-1) from WSTRING (UTF-16LE) values end-to-end. No on-disk format changes; no analyzer size math changes — the field is tracked but not yet consumed by `size_in_bytes` or `slot_count`.

**Architecture:** Additive field on one enum variant. `intermediates/string.rs` seeds `char_width` from `StringDeclaration.width` / `StringInitializer.width` (a `StringType` enum already exposed by `ironplc-dsl`). `type_environment.rs` registers the elementary STRING and WSTRING types with `CharWidth::Narrow` and `CharWidth::Wide`. All destructuring sites — across analyzer, codegen, and mcp — are updated to either bind the new field or use `..`. Existing size and slot calculations remain unchanged: they treat all strings as Latin-1 today, which is correct because no downstream consumer (container, VM, codegen) yet emits WSTRING bytes.

**Context:** Second slice of PR #1050 (WSTRING support). Depends on the `CharWidth` enum added in the previous slice. Lets the analyzer record string encoding without committing the runtime to a new container format — that bump happens in a later slice once analyzer, codegen, and VM are all ready to interoperate on the new layout.

**Tech Stack:** Rust, `ironplc-analyzer` crate (now depending on `ironplc-container` for `CharWidth`)

---

### Task 1: Add `ironplc-container` dependency to analyzer

**Files:**
- Modify: `compiler/analyzer/Cargo.toml`

Add `ironplc-container = { path = "../container", version = "0.214.0" }` to `[dependencies]`. Container is the natural home of `CharWidth` (it's the on-disk encoding boundary).

### Task 2: Add `char_width: CharWidth` to `IntermediateType::String`

**Files:**
- Modify: `compiler/analyzer/src/intermediate_type.rs`

Change the `String` variant from `String { max_len: Option<u128> }` to `String { max_len: Option<u128>, char_width: CharWidth }`. Update destructuring patterns in `size_in_bytes`, `slot_count`, `has_explicit_size`, and any other method that destructures the variant — bind the new field with `..` (or `char_width: _` when the existing match arm already pulls `max_len` by name).

**Deliberately leave `slot_count` and `size_in_bytes` math unchanged.** Header size stays at 4 bytes; payload is not multiplied by `char_width`. The math will be updated when the container format bumps.

### Task 3: Seed `char_width` from `StringDeclaration` / `StringInitializer`

**Files:**
- Modify: `compiler/analyzer/src/intermediates/string.rs`

`from()` and `from_decl()` read `StringType` from the DSL (`StringType::String` → `CharWidth::Narrow`, `StringType::WString` → `CharWidth::Wide`) via a small `char_width_for(&StringType) -> CharWidth` helper. Update the existing tests that destructure `{ max_len: ..., char_width: ... }`.

### Task 4: Register STRING and WSTRING with their widths in `type_environment`

**Files:**
- Modify: `compiler/analyzer/src/type_environment.rs`

`ELEMENTARY_TYPES_LOWER_CASE` currently registers both `"string"` and `"wstring"` as `IntermediateType::String { max_len: None }`. Differentiate them: `"string"` → `char_width: CharWidth::Narrow`, `"wstring"` → `char_width: CharWidth::Wide`.

### Task 5: Update remaining analyzer construction/destructuring sites

**Files:**
- Modify: `compiler/analyzer/src/intermediates/array.rs` — `ArrayElementType::String` / `WString` construct with the matching width
- Modify: `compiler/analyzer/src/intermediates/structure.rs` — test construction sites
- Modify: `compiler/analyzer/src/intermediates/subrange.rs` — test construction sites
- Modify: `compiler/analyzer/src/type_category.rs` — test construction site

### Task 6: Update downstream destructurings in codegen and mcp

**Files:**
- Modify: `compiler/codegen/src/compile_struct.rs` — `{ max_len }` → `{ max_len, .. }` at four sites; test constructions add `char_width: CharWidth::Narrow`
- Modify: `compiler/codegen/src/compile_array.rs` — `{ max_len }` → `{ max_len, .. }`
- Modify: `compiler/mcp/src/tools/types_all.rs` — `{ max_len }` → `{ max_len, .. }`

These crates already depend on `ironplc-container`, so `CharWidth` is in scope.

---

## Deliberately out of scope

- `FORMAT_VERSION` bump, header layout change, `STRING_HEADER_BYTES` 4→6
- `size_in_bytes` / `slot_count` updates to account for WSTRING payload doubling
- Codegen actually emitting WSTRING-aware bytecode (consumes `char_width` from `IntermediateType::String`)
- VM-side encoding-mismatch traps (V9014/V9015)
- Type-checker rejection of `STRING := WSTRING` (ADR-0034)

## Verification

`cd compiler && just` must pass: compile, coverage ≥ 85%, clippy, fmt, dupes. Existing string tests continue to work; new construction sites use `CharWidth::Narrow` (matching today's behavior). The WSTRING test in `intermediates/string.rs` confirms the new field flows through for `WSTRING` declarations.
