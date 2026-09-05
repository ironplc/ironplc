# Plan: Field Access on Array-of-Struct Elements

## Context

Accessing a field of an element of an array-of-struct fails in codegen with
P9999 "Not implemented" at `compile_struct.rs` L308:

```st
MyBay.Devices.MeterQRScanner[i].Trigger := TRUE;
```

Reported from the playground in
[#1376](https://github.com/ironplc/ironplc/issues/1376).

The AST for the target is:

```
Structured {
    record: Array {
        subscripted_variable: Structured { ... MyBay.Devices.MeterQRScanner },
        subscripts: [i],
    },
    field: Trigger,
}
```

`walk_struct_chain` handles `Named` and `Structured` records but not `Array`,
so it falls through to the `_ =>` todo. The chain cannot be resolved to a
single compile-time `SlotIndex` because a runtime subscript sits in the middle.

### What already works

- The analyzer needs no change: `IntermediateType::slot_count()` already
  computes array-of-struct layout as `total_elements * element_slots`.
- `ResolvedAccess::StructFieldArrayElement` already resolves
  `struct.arrayField[i]` for **primitive** element types, emitting
  `flat_index + field_slot_offset` followed by the struct's
  `LOAD_ARRAY`/`STORE_ARRAY`.
- `emit_flat_index` computes `sum((s_k - l_k) * stride_k)` from
  `DimensionInfo.stride`, while bounds checks use the unscaled
  `lower_bound`/`size`.

## Approach

### Key insight

Because `emit_flat_index` multiplies each subscript by a per-dimension
`stride`, scaling every stride by the element's slot count makes the emitted
flat index *already* a slot offset within the array field. The leaf field's
offset within the element struct is a compile-time constant that folds into
the existing `field_slot_offset` operand.

```
slot = array_field_offset + leaf_field_offset          (compile-time constant)
     + flat_index(subscripts, strides * element_slots) (runtime)
```

So no new `ResolvedAccess` variant, no new opcode, and no emission changes are
required — only a new resolver that builds a `StructFieldArrayElement` with
scaled strides and a combined constant offset. Bounds checking is preserved
because `try_constant_flat_index` validates against `lower_bound`/`size`,
which stay unscaled.

### Changes

1. **`compile_array.rs`** — add `resolve_struct_array_element_field()`:
   walk the array chain to collect subscripts, require a `Structured` base,
   resolve it via `walk_struct_chain` to the array field's slot offset and
   `IntermediateType::Array`, require an `IntermediateType::Structure`
   element, look the leaf field up with `find_field_in_type`, and return a
   `StructFieldArrayElement` with scaled strides and combined offset.

2. **`compile_array.rs`** — in `resolve_access()`, dispatch
   `Structured` whose `record` is an `Array` to the new resolver.

3. **`compile_expr.rs` / `compile_stmt.rs`** — add a match guard to the
   existing `Structured` arms so this shape is *not* claimed by the
   fixed-offset struct-field path, and falls through to the generic
   `resolve_access` dispatch that already handles `StructFieldArrayElement`
   for both read and write.

4. **Tests** — codegen unit tests plus end-to-end execution tests for read,
   write, literal and variable subscripts, and multi-dimensional arrays.

## Scope

Leaf fields that are **primitive** (BOOL/INT/REAL/enum/subrange/…). Two
neighbouring gaps are explicitly out of scope and tracked separately:

- **STRING leaf fields** (`MeterQRScanner[i].LastCode := '...'`). The VM
  derives a string array's element stride from the descriptor
  (`STRING_HEADER_BYTES + element_extra`), so a string strided by the
  *enclosing struct's* element size cannot be expressed by the current
  8-byte `ArrayDescriptor`. Resolving this needs either an explicit stride
  field in the descriptor (a container format change) or per-element
  unrolled `STR_INIT` plus fold-the-address-into-scratch addressing (bytecode
  growth, and loses the runtime subscript bounds trap). That is a design
  decision, not a mechanical extension.

- **Top-level `ARRAY OF <struct>` variables**, which fail earlier at
  declaration time because `resolve_type_name` in `compile_setup.rs` resolves
  only elementary type names.

Because the reported program assigns to `MeterQRScanner[i].LastCode`, #1376
is **not** fully closed by this change; the STRING follow-up is required.

## Files

| File                                | Change                                    |
| ----------------------------------- | ----------------------------------------- |
| `compiler/codegen/src/compile_array.rs` | New resolver + `resolve_access` dispatch |
| `compiler/codegen/src/compile_expr.rs`  | Match guard on the `Structured` read arm |
| `compiler/codegen/src/compile_stmt.rs`  | Match guard on the `Structured` write arm |
| `compiler/codegen/tests/`               | Unit + end-to-end tests                  |
