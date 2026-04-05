# Fix FIND with global struct array subscript

## Problem

Calling `FIND(g.names[1], 'World')` where `g` is a global struct with an
`ARRAY OF STRING` field fails at compile time because:

1. `resolve_string_arg` only handled simple named variables in `string_vars`;
   complex variable expressions (struct field array subscripts) fell through
   to an unsupported-code path.
2. `allocate_struct_variable` did not detect `ARRAY OF STRING` fields inside
   structures or register the helper variables / array descriptors needed for
   string-array element access.
3. `initialize_struct_fields` did not emit `STR_INIT` headers for string
   array elements within structs.
4. `resolve_struct_field_array` did not redirect STRING array fields to the
   pre-registered `ArrayVarInfo` entries, so the runtime could not resolve
   them via the normal `ArrayElement` path.

## Changes

### compile.rs

- **`alloc_aux_variable`** -- new method on `CompileContext` to allocate
  synthetic variable slots.
- **`resolve_string_arg`** -- fall through from the `Variable` arm to a
  general expression path when the variable is not a simple named string var.
  Allocates a temporary data-region slot, compiles the expression, and stores
  the result so FIND/MID/etc. can read it.
- **`emit_initial_values`** -- after `initialize_struct_fields`, iterate
  `string_array_fields` and emit `STORE_VAR_I32` + `STR_INIT_ARRAY` for each
  helper variable.

### compile_struct.rs

- **`StructStringArrayFieldInfo`** -- new struct holding helper var index,
  descriptor index, max string length, and total element count.
- **`StructVarInfo`** -- add `string_array_fields` field.
- **`allocate_struct_variable`** -- after tracking max STRING capacity,
  iterate fields looking for `IntermediateType::Array` with a STRING element
  type. For each, allocate a synthetic helper variable, register an array
  descriptor, pre-register an `ArrayVarInfo` in `ctx.array_vars`, and store
  metadata in `string_array_fields`.
- **`initialize_struct_fields`** -- add an arm for `IntermediateType::Array`
  with STRING element type that emits `STR_INIT` for every element.

### compile_array.rs

- **`dimensions_from_intermediate_pub`** -- public wrapper so
  `compile_struct` can call the private `dimensions_from_intermediate`.
- **`resolve_struct_field_array`** -- before calling `resolve_field_op_type`,
  check whether the field is in `string_array_fields`; if so, look up the
  synthetic `ArrayVarInfo` and return `ResolvedAccess::ArrayElement` directly.

### Tests

- New end-to-end test `end_to_end_when_find_with_struct_array_field` in
  `end_to_end_find.rs` that declares a global struct with `ARRAY[1..3] OF
  STRING`, assigns a value, and verifies `FIND` returns the correct position.
