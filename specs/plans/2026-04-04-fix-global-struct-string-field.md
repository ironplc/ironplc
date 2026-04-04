# Fix: Global struct variable with STRING field (P9999)

## Problem

When a `TYPE STRUCT` containing a `STRING` field is instantiated as a
`VAR_GLOBAL` (or `VAR`) variable, codegen fails with P9999 "Structure
contains unsupported field types (STRING, WSTRING, or FunctionBlock)".

Root cause: `IntermediateType::slot_count()` returns
`Err(UnsupportedFieldType)` for STRING, which blocks
`allocate_struct_variable()`.

OSCAT impact: globals `LANGUAGE` and `SETUP` contain STRING fields,
blocking all 294 functions in full mode.

## Approach

Give STRING fields a proper slot count so structs can allocate them in
the data region. Each STRING occupies `ceil((4 + max_len) / 8)` 8-byte
slots (4 = header bytes). During struct initialization, emit `STR_INIT`
for each STRING field using the computed byte offset.

Scope: allocation and initialization only. Reading/writing STRING fields
via struct field access is a follow-up.

## Changes

### 1. `compiler/analyzer/src/intermediate_type.rs`

- `slot_count_inner()`: compute slot count for `IntermediateType::String`
  using `ceil((4 + max_len_or_254) / 8)`.

### 2. `compiler/codegen/src/compile_struct.rs`

- `StructFieldInfo` and `FieldInitInfo`: add `string_max_length: Option<u16>`.
- `build_struct_fields()`: populate `string_max_length` for STRING fields.
- `initialize_struct_fields()`: add `struct_data_offset: u32` parameter;
  emit `STR_INIT` for STRING fields.

### 3. `compiler/codegen/src/compile.rs`

- `emit_initial_values()`: pass `data_offset` to `initialize_struct_fields`.
- `allocate_struct_variable()`: update `max_string_capacity` for STRING
  fields in structs.

### 4. Tests

- Update existing `build_struct_fields_when_string_field_then_returns_error`.
- Add `slot_count` test for STRING in intermediate_type.rs.
- Add end-to-end codegen test for the minimal repro.
