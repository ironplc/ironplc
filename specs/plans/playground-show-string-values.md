# Plan: Show STRING values in the playground variables panel

## Problem

The playground variables panel currently shows `0` (or `<STRING>` after a
stop-gap placeholder) for STRING variables. STRING values do not live in
the variable table — the slot returned by `vm.read_variable_raw(VarIndex)`
is unused for STRING. The actual bytes live in the VM data region at a
`data_offset` hardcoded into the bytecode (used by `STR_INIT` /
`STR_LOAD_VAR` / `STR_STORE_VAR` opcodes) with the layout
`[max_len: u16][cur_len: u16][bytes…]`. Neither the playground nor the
shared `format_variable_value` helper has access to that offset, so the
content cannot be rendered.

## Approach

Add a new debug section sub-table that records, per STRING variable, the
`data_offset` and `max_length`. The sub-table is purely additive: existing
readers already skip unknown tags (`debug_section.rs` lines 227–231), so
older `ironplcc` binaries can still load containers with the new section.

Render STRING values in the playground by combining the layout entry with
`vm.data_region()` to read the bytes and decode as UTF-8.

WSTRING is out of scope for this PR — the codegen path currently allocates
WSTRING using the same `String` initializer kind but the wide-character
encoding work hasn't been done. Continue showing a `<WSTRING>` placeholder
for type-tag `WSTRING`.

## Changes

1. **`compiler/container/src/debug_section.rs`**
   - Add `TAG_STRING_LAYOUT: u16 = 4`.
   - Add `pub struct StringLayoutEntry { pub var_index: VarIndex, pub data_offset: u32, pub max_length: u16 }` (8 bytes serialized).
   - Add `pub string_layouts: Vec<StringLayoutEntry>` to `DebugSection`.
   - Implement `write_string_layouts` / `read_string_layouts`, payload size,
     and directory wiring (mirroring the existing var_names pattern).
   - Round-trip test.

2. **`compiler/container/src/lib.rs`**
   - Re-export `StringLayoutEntry`.
   - `ContainerBuilder::add_string_layout(entry)`.

3. **`compiler/codegen/src/compile.rs`**
   - Add `pub(crate) debug_string_layouts: Vec<StringLayoutEntry>` to
     `CompileContext`; flush into the builder alongside `debug_var_names`.

4. **`compiler/codegen/src/compile_setup.rs`**
   - In the `InitialValueAssignmentKind::String` arm, after the `data_offset`
     is allocated, push a `StringLayoutEntry { var_index: index, data_offset,
     max_length }` into `ctx.debug_string_layouts`.

5. **`compiler/container/src/debug_format.rs`**
   - Add `pub fn format_string_value(data_region: &[u8], data_offset: u32) -> String`
     that reads the cur_len from the header and decodes the bytes as UTF-8
     (lossy where needed). Returns `'<text>'` (single-quoted, IEC literal
     style).
   - Existing `format_variable_value` stays unchanged so the
     `(raw, tag)` callers still work.
   - Add `pub fn build_string_layout_map(container: &Container) -> HashMap<u16, u32>`
     that returns var_index → data_offset.

6. **`compiler/playground/src/lib.rs`**
   - Build the string layout map at run/step time alongside the existing
     debug map.
   - Replace the `<STRING>` placeholder: when tag is `STRING` and a layout
     exists, call `format_string_value(vm.data_region(), offset)`.
   - Update tests: replace the placeholder unit test with one that runs a
     program assigning a string literal and asserts the rendered value.

7. **`compiler/vm-cli/src/cli.rs`**
   - Optional in this PR — skip if it expands scope. Only the playground is
     in the user's request. Document follow-up if skipped.

## Acceptance

- A program `s : STRING := 'hello';` shows `'hello'` in the playground
  variables panel.
- A program assigning `s := 'world'` shows `'world'` after a scan.
- Empty strings render as `''`.
- Old containers without the new sub-table still load (backward compat
  test added).
- `cd compiler && just` passes (compile, coverage ≥ 85%, lint).

## Out of scope

- WSTRING rendering (still shows `<WSTRING>`).
- VM-CLI string rendering (separate PR).
- Truncation/escaping of non-printable characters in the displayed
  string (separate concern; UTF-8 lossy decode is sufficient for now).
