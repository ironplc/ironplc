# Fix Struct Field Assignment on Function Return Values

## Problem

When a function returns a struct type, assigning to individual fields of the
return value (e.g. `MAKE_POINT.X := px`) fails in codegen with P9999. This is
standard IEC 61131-3 syntax for building struct return values field by field.

**Root cause:** In `compile.rs:752-758`, the `FunctionReturnType::Named(_)`
branch only calls `resolve_type_name()` which handles elementary types. For
struct return types it returns `None` and nothing is registered in
`ctx.struct_vars`, so any field access on the return variable fails.

## Changes

### 1. Thread `TypeEnvironment` into `compile_user_function`

Add `types: &TypeEnvironment` parameter so struct return types can be resolved.

### 2. Save/restore `struct_vars` during function compilation

Add `saved_struct_vars` alongside the existing save/restore of `variables`,
`var_types`, `string_vars`, and `array_vars`.

### 3. Register struct return variable

In the `FunctionReturnType::Named(_)` branch, check
`types.resolve_struct_type()`. If the return type is a struct, call
`allocate_struct_variable()` to allocate data-region space and register the
return variable in `ctx.struct_vars`.

### 4. Track struct return info at call sites

Add `StructReturnInfo { data_offset, total_slots, desc_index }` to
`UserFunctionInfo` so the caller can perform struct-copy after `CALL`.

### 5. Initialize struct return var in function prologue

Store the data-region offset into the return variable slot and zero all struct
slots via `initialize_struct_fields` with empty element inits.

### 6. Whole-struct copy at call site

Before the scalar assignment path, detect struct targets. After `CALL` leaves
the source `data_offset` on the stack, use a temporary-pointer swap protocol:

1. Store source offset into destination var (temporarily re-pointing it)
2. `LOAD_ARRAY` all N slots onto the stack
3. Restore the destination var's own `data_offset`
4. `STORE_ARRAY` all N slots in reverse (LIFO) order

## Test Plan

- End-to-end: function returns POINT, assigns fields, caller verifies values
- End-to-end: two consecutive calls prove independent copy semantics
