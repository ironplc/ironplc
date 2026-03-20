# Structure Memory Layout in the Data Region

status: proposed
date: 2026-03-18

## Context and Problem Statement

The compiler is adding code generation support for user-defined structures (STRUCT). IEC 61131-3 structures contain named fields of heterogeneous types, including primitives, enumerations, other structures (nesting), and arrays. The semantic analyzer already computes field offsets using C-style alignment rules (`analyzer/src/intermediates/structure.rs`), producing byte-level offsets for each field.

The VM's data region (ADR-0017) stores all variable-length data. Arrays and function block instances already use this region with **8-byte (slot-sized) elements** — each array element and each FB field occupies 8 bytes regardless of declared type. This simplifies the VM: every element is at `data_offset + index * 8`.

How should structure fields be laid out in the data region?

## Decision Drivers

* **Consistency** — arrays and FB instances already use 8-byte slots; structures should integrate cleanly with existing patterns, especially for arrays of structures and structures containing arrays
* **VM simplicity** — the VM is no_std for embedded targets (ADR-0010); fewer special cases reduce code size and bug surface
* **Composability** — nested structures, structures containing arrays, and arrays of structures must all work with a single layout model
* **Future-proofing** — packed byte-level layout is desirable for memory efficiency on embedded targets and should be achievable without a redesign
* **Safety** — all memory must be statically determined at compile time; field access must not allow out-of-bounds reads/writes (ADR-0005)

## Considered Options

* **Option A: 8-byte slot per field** — each field occupies one 8-byte slot in the data region, matching the array/FB element model. Fields are addressed by slot index relative to the structure's base offset.
* **Option B: Packed byte-level layout** — use the analyzer's computed byte offsets directly. Each field occupies only the bytes its type requires (e.g., 1 byte for BOOL, 4 bytes for DINT). The VM uses typed load/store operations that read/write the correct number of bytes.
* **Option C: Hybrid** — use 8-byte slots initially but define the layout abstraction such that a future packed mode can be swapped in without changing opcodes or compiler architecture.

## Decision Outcome

Chosen option: **Option C (Hybrid)** — use 8-byte slot-per-field layout now, designed so that migration to packed byte-level layout requires only changing the offset computation in the compiler and the field access implementation in the VM, without changing opcodes, container format, or the overall compilation strategy.

### How It Works

Each structure variable is allocated a contiguous block in the data region:

```
data_region[base_offset + 0*8]   → field 0 (8 bytes, slot-sized)
data_region[base_offset + 1*8]   → field 1 (8 bytes, slot-sized)
...
data_region[base_offset + N*8]   → field N (8 bytes, slot-sized)
```

The **structure size** is `num_fields * 8` bytes. The structure's `data_offset` (stored in its slot table entry) points to the first byte of field 0.

Field access uses the same LOAD/STORE patterns as scalar variables, but targeting data region bytes instead of the slot table. The compiler resolves `myStruct.fieldName` to `base_offset + field_index * 8` at compile time (see ADR-0027).

### Nested Structures

A structure containing another structure stores the inner structure's fields inline (not as a pointer):

```
TYPE Inner :
  STRUCT
    x : INT;
    y : INT;
  END_STRUCT;
END_TYPE

TYPE Outer :
  STRUCT
    a : DINT;
    inner : Inner;
    b : BOOL;
  END_STRUCT;
END_TYPE
```

Layout of an `Outer` variable:

```
Offset   Field
+0       a      (8 bytes, slot)
+8       inner.x (8 bytes, slot)
+16      inner.y (8 bytes, slot)
+24      b      (8 bytes, slot)
```

Total size: 32 bytes (4 fields × 8 bytes). The inner structure is flattened into the outer structure's slot sequence. `outer.inner.x` resolves to `base_offset + 1 * 8`.

### Arrays of Structures

An `ARRAY[1..3] OF Inner` allocates `3 * struct_size_in_slots * 8` bytes:

```
Offset   Element
+0       [1].x  (8 bytes)
+8       [1].y  (8 bytes)
+16      [2].x  (8 bytes)
+24      [2].y  (8 bytes)
+32      [3].x  (8 bytes)
+40      [3].y  (8 bytes)
```

Total: 48 bytes. Each array element is `struct_size_in_slots * 8` bytes. Field access within an element: `base_offset + (flat_index * struct_size_in_slots + field_slot_index) * 8`.

### Structures Containing Arrays

A structure with an array field stores the array elements inline:

```
TYPE WithArray :
  STRUCT
    tag : INT;
    values : ARRAY[1..4] OF REAL;
    count : INT;
  END_STRUCT;
END_TYPE
```

Layout:

```
Offset   Field
+0       tag        (8 bytes, 1 slot)
+8       values[1]  (8 bytes)
+16      values[2]  (8 bytes)
+24      values[3]  (8 bytes)
+32      values[4]  (8 bytes)
+40      count      (8 bytes, 1 slot)
```

Total: 48 bytes (6 slots). The array is embedded at a known slot offset within the structure. Accessing `w.values[i]` resolves to `base_offset + (1 + flat_index) * 8`, where 1 is the slot offset of the `values` field.

### Consequences

* Good, because consistency with arrays and FB instances is maintained — the VM treats all data region content uniformly as 8-byte slots
* Good, because the VM needs no new typed load/store logic — existing `as_i32()`, `as_i64()`, `as_f32()`, `as_f64()` slot accessors work on structure fields identically to array elements
* Good, because composability works by construction — arrays of structures and structures of arrays are both contiguous slot sequences with compile-time-computable offsets
* Good, because the migration path to packed layout is clear: change `field_slot_index * 8` to `field_byte_offset` in the compiler's offset calculation, and use typed byte-width loads/stores in the VM
* Bad, because memory waste is significant for structures with many small fields — a structure with 10 BOOL fields uses 80 bytes instead of 10 bytes (8x overhead)
* Neutral, because PLC programs typically have small numbers of structure instances, so the absolute memory waste is bounded in practice

### Migration Path to Packed Layout

When packed layout is implemented:

1. **Compiler change**: Replace `field_slot_index * 8` with the analyzer's `field.offset` (byte-level offset with alignment). Replace `struct_size_in_slots * 8` with the analyzer's `structure.size_in_bytes()`.
2. **VM change**: Add typed byte-width data region accessors (read 1/2/4/8 bytes at a byte offset). Existing slot-width accessors remain for the slot table.
3. **Container format**: No change — `data_region_bytes` already counts bytes, not slots.
4. **Opcodes**: No change — field access is resolved at compile time (ADR-0027), so the bytecode only sees data region byte offsets.

## More Information

### Relationship to ADR-0017

ADR-0017 defines the unified data region where all variable-length data lives. Structure fields are stored in this region. The `data_offset` in the slot table entry points to the structure's first field.

### Relationship to ADR-0023

For arrays of structures, the flat index bounds check (`0 ≤ flat_index < total_elements`) continues to guarantee memory safety. Each "element" in the array descriptor is a complete structure, so `total_elements` equals the array dimension size, and the element stride is `struct_size_in_slots` slots. The flat index addresses structures, not individual fields — field access within the selected structure element is resolved at compile time.

### Slot-Count Computation

The number of slots for a structure is computed recursively:

- Primitive field (BOOL, INT, DINT, REAL, etc.): 1 slot
- Enumeration field: 1 slot
- Nested structure field: sum of slots of all fields (recursive)
- Array field: `total_elements * element_slots` where `element_slots` is 1 for primitive arrays or the nested structure's slot count for structure arrays

This computation is performed at compile time during structure type resolution. The result is deterministic and bounded by the declared types. The recursive computation includes a depth guard (max 32 nesting levels) as defense-in-depth against type cycles that escape the analyzer's toposort validation (`xform_toposort_declarations.rs`). If the depth limit is exceeded, `slot_count()` returns `None` and the compiler rejects the structure with a diagnostic.

A single structure variable must not exceed **32,768 total slots** (matching the existing array element limit). This ensures flat-index arithmetic in LOAD_ARRAY/STORE_ARRAY stays within safe i32 bounds.

### Element Type Byte

When registering an array descriptor for a structure variable, the element type byte is `SLOT = 6` (a new value distinct from the primitive type bytes 0-5). This distinguishes structure descriptors from typed array descriptors in the bytecode verifier and debug tools, and prevents false descriptor deduplication with unrelated arrays.

### Scope Limitations

This ADR covers structures containing:
- Primitive types (BOOL, INT, DINT, LINT, REAL, LREAL, etc.)
- Enumerations
- Other structures (nesting)
- Arrays of the above

**Not in scope** (deferred to future work):
- Structures containing STRING or WSTRING fields — these require data region sub-allocation for the string's `[max_length][cur_length][data]` layout within the structure's data region block, plus copy semantics for structure assignment
- Structures containing function block instances — these require FB lifecycle management (init/call) for embedded instances
- Structures containing arrays of strings or arrays of function blocks
