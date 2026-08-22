# Plan: Top-Level `ARRAY OF <struct>` Variables

Fixes [#1383](https://github.com/ironplc/ironplc/issues/1383).

## Goal

Allow a variable whose type is an array of a user-defined structure to be
declared and accessed at the top level of a POU, so that

```st
TYPE Item : STRUCT Flag : BOOL; END_STRUCT; END_TYPE

PROGRAM main
VAR
    Arr : ARRAY[1..6] OF Item;
END_VAR
    Arr[1].Flag := TRUE;
END_PROGRAM
```

compiles and runs. Today `ironplcc check` accepts the program but codegen
rejects it at *declaration* time with P9999 "Unsupported array element type",
so neither reads nor writes are reachable.

## Architecture

`register_array_variable` resolves the element type through
`compile_setup::resolve_type_name`, which maps only elementary and generic
type names to a `VarTypeInfo`. A structure type name yields `None`, hence the
diagnostic.

The array-of-struct layout is already solved for the *struct field* case
(`holder.items[i].field`, issue #1376): structures occupy a contiguous run of
slots, so an array of them is a flat slot array, and
`ResolvedAccess::StructFieldArrayElement` addresses it as

```text
slot = base_slot_offset + leaf_offset      (compile time)
     + flat_index * element_slots          (runtime)
```

A top-level array-of-struct has no enclosing struct whose descriptor and data
region it can borrow, so it needs its own allocation path. Once it has one,
the *access* path is identical: register the variable exactly the way a
structure variable is registered — variable slot holds the data-region byte
offset, plus a slot-typed array descriptor sized `total_elements *
element_slots` — and `Arr[i].field` reuses `StructFieldArrayElement` with a
`base_slot_offset` of 0. No new opcode and no new emission path.

Element field *initialization* is deliberately not emitted, matching the
existing array-of-struct-through-a-field behavior
(`initialize_struct_fields` emits nothing for an array-of-struct field): the
data region starts zeroed. Explicit array initializers on an array-of-struct
are rejected with a clear diagnostic rather than silently ignored.

## Design doc reference

None. Extends the layout established in
`specs/plans/struct-array-field-subscript.md` and the array-of-struct element
addressing added for #1376.

## File map

| File | Change |
|------|--------|
| `compiler/codegen/src/compile.rs` | `CompileContext::struct_array_vars` map |
| `compiler/codegen/src/compile_array.rs` | `StructArrayVarInfo`, `register_struct_array_variable`, top-level base in `resolve_struct_array_element_field`, shared element-field access helper |
| `compiler/codegen/src/compile_setup.rs` | Route struct element types to the new registration; store the data offset into the variable slot in `emit_initial_values` |
| `compiler/codegen/src/compile_fn.rs` | Save/restore `struct_array_vars` and re-expose globals to function and FB bodies |
| `compiler/codegen/tests/it/end_to_end_array_of_struct.rs` | New end-to-end file (registered in `tests/it/main.rs`) |

## Tasks

- [ ] Add `StructArrayVarInfo` and `CompileContext::struct_array_vars`
- [ ] Add `register_struct_array_variable` (slot accounting, limits, descriptor)
- [ ] Route inline (`ARRAY[..] OF Item`) and named array types with a
      structure element type to the new registration in `assign_variables`
- [ ] Emit the data-region offset into the variable slot in
      `emit_initial_values`; reject explicit initial values
- [ ] Resolve `Arr[i].field` for a top-level base in
      `resolve_struct_array_element_field`, factoring the tail shared with the
      struct-field base into one helper
- [ ] Give whole-element access (`Arr[i]` without a field) an explanatory
      diagnostic instead of the bare `todo`
- [ ] Save/restore `struct_array_vars` across function and FB body compilation
- [ ] End-to-end tests: literal and variable subscript, distinct elements,
      sibling fields, multi-dimensional, FOR loop, BOOL leaf, read-back in an
      expression, named array type, and a global declaration
- [ ] Unit tests for the registration and resolution error paths
- [ ] `cd compiler && just`
