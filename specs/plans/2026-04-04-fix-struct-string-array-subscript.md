# Plan: Fix P9999 for STRING Array Subscript on Struct Field

## Context

Accessing a STRING array field of a struct variable (e.g., `lang.NAMES[row, col]`
where NAMES is `ARRAY[1..2, 1..3] OF STRING[10]`) fails with P9999 "Not implemented"
at codegen. This blocks 2 OSCAT functions (MONTH_TO_STRING, WEEKDAY_TO_STRING).

**Root cause:** `resolve_struct_field_array()` calls `resolve_field_op_type(element_type)`
which returns `None` for STRING (a composite type), causing P9999. The existing
`StructFieldArrayElement` uses slot-based `LOAD_ARRAY`/`STORE_ARRAY`, but STRING
elements need byte-addressed `STR_LOAD_ARRAY_ELEM`/`STR_STORE_ARRAY_ELEM`.

## Approach

Use a **scratch variable** to hold `struct_data_offset + field_byte_offset` and a
STRING-specific array descriptor so existing `STR_LOAD/STORE_ARRAY_ELEM` VM opcodes
work without VM changes.

### Changes

1. **compile_struct.rs** — Add `scratch_var_index` and `string_array_descs` to
   `StructVarInfo`. In `allocate_struct_variable`, detect STRING array fields and
   register a scratch variable + STRING array descriptor for each. In
   `initialize_struct_fields`, emit `STR_INIT` for each STRING array element header.

2. **compile_array.rs** — Add `StructFieldStringArrayElement` variant to
   `ResolvedAccess`. In `resolve_struct_field_array`, detect STRING element type
   and return the new variant with scratch var and string descriptor info.

3. **compile.rs** — Add `allocate_scratch_variable` method. Add read/write dispatch
   arms for `StructFieldStringArrayElement` that compute
   `struct_data_offset + field_byte_offset → scratch`, then emit flat index +
   `STR_LOAD/STORE_ARRAY_ELEM`.

4. **Tests** — End-to-end tests for single-dim, multi-dim, and global struct
   STRING array field access.

## Files

| File | Change |
|------|--------|
| `compiler/codegen/src/compile_struct.rs` | StructVarInfo fields, scratch alloc, STRING array init |
| `compiler/codegen/src/compile_array.rs` | New variant, STRING handling in resolve |
| `compiler/codegen/src/compile.rs` | Scratch alloc method, read/write emission |
| `compiler/codegen/tests/end_to_end_struct.rs` | 3 end-to-end tests |
