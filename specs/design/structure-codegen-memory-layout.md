# Structure Code Generation — Memory Layout Design

## Overview

This document defines the memory layout and access patterns for user-defined structures (STRUCT) in the IronPLC bytecode VM. It covers simple structures, nested structures, structures containing arrays, and arrays of structures.

**Scope**: Structures containing primitive types (BOOL, INT, DINT, LINT, REAL, LREAL, etc.), enumerations, other structures (nesting), and arrays of these types.

**Out of scope** (deferred — marked with TODO in implementation):
- Structures containing STRING or WSTRING fields
- Structures containing function block instances
- Whole-structure assignment (`s1 := s2`)
- Structure literals in expressions

**Prerequisite reading**: ADR-0026 (structure memory layout), ADR-0027 (compile-time field offset resolution), ADR-0017 (unified data region), ADR-0023 (array bounds safety).

---

## 1. Storage Model

### 1.1 Data Region Allocation

Each structure variable occupies a contiguous block in the unified data region (ADR-0017). Each field occupies one 8-byte slot, regardless of declared type (ADR-0026). The variable's slot table entry holds the `data_offset` — the byte offset in the data region where the structure's first field begins.

```
Slot Table                          Data Region
┌──────────┐                        ┌──────────────────┐
│ var_idx  │──data_offset──────────>│ field 0 (8 bytes) │ offset + 0
├──────────┤                        ├──────────────────┤
│  ...     │                        │ field 1 (8 bytes) │ offset + 8
└──────────┘                        ├──────────────────┤
                                    │ field 2 (8 bytes) │ offset + 16
                                    ├──────────────────┤
                                    │  ...              │
                                    └──────────────────┘
```

### 1.2 Slot-Count Computation

The number of 8-byte slots a structure type requires is computed recursively from its `IntermediateType::Structure`:

| Field type | Slots |
|---|---|
| Primitive (BOOL, INT, DINT, LINT, REAL, LREAL, etc.) | 1 |
| Enumeration | 1 |
| Subrange | 1 |
| Nested structure | sum of slots of all nested fields (recursive) |
| Array of primitives | `total_elements` |
| Array of structures | `total_elements * struct_slots` |

Example:

```
TYPE Point : STRUCT
    x : REAL;    (* 1 slot *)
    y : REAL;    (* 1 slot *)
END_STRUCT; END_TYPE
(* Point = 2 slots = 16 bytes *)

TYPE Rect : STRUCT
    topLeft  : Point;         (* 2 slots, inline *)
    botRight : Point;         (* 2 slots, inline *)
    color    : INT;           (* 1 slot *)
END_STRUCT; END_TYPE
(* Rect = 5 slots = 40 bytes *)
```

### 1.3 Structure Size Limit

The maximum structure size is bounded by the data region limit (`data_region_bytes: u32`, max 4 GiB). Practically, the `data_offset` is stored as `i32` in a slot (matching the array pattern), limiting effective addressing to 2 GiB. The compiler must assert `data_region_offset <= i32::MAX as u32` during allocation.

---

## 2. Field Access Patterns

All field accesses are resolved to data region byte offsets at compile time (ADR-0027). No structure-specific opcodes are needed.

### 2.1 Simple Field Access: `myStruct.field`

Given:
```
VAR
    pt : Point;   (* data_offset = D *)
END_VAR

pt.x := 3.14;
value := pt.y;
```

**Read `pt.y`**: Field `y` is at slot index 1 within Point.
- Byte offset in data region: `D + 1 * 8`
- Bytecode: Load the constant offset `D + 8`, use it to read from the data region

**Write `pt.x`**: Field `x` is at slot index 0.
- Byte offset: `D + 0 * 8 = D`
- Bytecode: Compute value `3.14`, store to data region at offset `D`

The compiler resolves the access chain at compile time. The emitted bytecode is identical to an array element load/store with a constant index — there is no indication in the bytecode that a structure is involved.

### 2.2 Nested Field Access: `outer.inner.field`

Given:
```
VAR
    r : Rect;   (* data_offset = D *)
END_VAR

r.botRight.x := 10.0;
```

Resolution:
1. `r` → `data_offset = D`
2. `botRight` is field index 2-3 in Rect (starts at slot 2, because `topLeft` occupies slots 0-1)
3. `x` is field index 0 within Point
4. Absolute slot offset: `2 + 0 = 2`
5. Byte offset: `D + 2 * 8`

Any depth of nesting follows the same recursive resolution. The compiler walks the `StructuredVariable` AST chain:
- `StructuredVariable { record: StructuredVariable { record: Named("r"), field: "botRight" }, field: "x" }`
- Resolves `r` → struct type `Rect`, `data_offset = D`
- Resolves `botRight` in `Rect` → slot offset 2, field type is `Point`
- Resolves `x` in `Point` → slot offset 0
- Final: `D + (2 + 0) * 8`

### 2.3 Array-of-Struct Access: `arr[i].field`

Given:
```
VAR
    points : ARRAY[1..3] OF Point;   (* data_offset = D, 3 elements, 2 slots each *)
END_VAR

points[2].y := 5.0;
```

Each `Point` element occupies 2 slots (stride = 2). For `points[2].y`:
1. Normalize subscript: `flat_index = 2 - 1 = 1` (0-based)
2. Field `y` is slot index 1 within Point
3. Byte offset: `D + (flat_index * 2 + 1) * 8 = D + (1 * 2 + 1) * 8 = D + 24`

**With constant subscript**: The entire offset is computed at compile time. Emit a single constant-offset data region store.

**With variable subscript**: The compiler emits:
1. Compute `flat_index` from the subscript expression (subtract lower bound)
2. Multiply by stride: `flat_index * 2` (using i64 arithmetic per ADR-0023)
3. Add field offset: `+ 1`
4. Multiply by 8 (or fold into stride: `flat_index * 16 + 8`)
5. Add base: `+ D`
6. Use this as the data region byte offset for the load/store

**Bounds checking**: The array descriptor records `total_elements = 3`. The VM checks `0 ≤ flat_index < 3` via the existing LOAD_ARRAY/STORE_ARRAY bounds check. The field offset within the selected element is a compile-time constant and cannot be out of bounds (the compiler knows the structure layout).

**Implementation note**: For arrays of structures, the "element" from the VM's perspective is the whole structure. The array descriptor's total_elements counts structures, not individual fields. The element stride in the flat-index computation is `struct_slots`, not 1. After bounds-checking the structure-level index, the field offset is added. This means the existing LOAD_ARRAY/STORE_ARRAY opcodes may not directly apply since they read/write a single 8-byte slot at `data_offset + flat_index * 8`. For arrays of structures, the access pattern is `data_offset + (struct_index * struct_stride + field_offset) * 8`. Two approaches:

- **Option A**: Treat the array as if it has `total_elements * struct_slots` flat elements. Bounds check is against the total slot count. The flat index for `arr[i].field` is `(i - lower) * struct_slots + field_slot`. This reuses LOAD_ARRAY/STORE_ARRAY unchanged but the bounds check is coarser (it prevents out-of-array-bounds access but not out-of-struct-bounds access within an element).
- **Option B**: Use direct data region byte offset computation without LOAD_ARRAY/STORE_ARRAY. Emit the bounds check as compiler-generated code (compare flat_index against total_elements, jump to trap on failure), then compute the byte offset and use a generic data region read/write. This is more explicit but requires the compiler to emit bounds-check code.

**Recommendation**: Option A — treat arrays of structures as flat slot arrays with `total_elements = array_size * struct_slots`. The `flat_index` for `arr[i].field` becomes `(i - lower) * struct_slots + field_slot_offset`. This reuses the existing array infrastructure entirely. The trade-off (coarser bounds granularity) is identical to the multi-dimensional array case documented in ADR-0023 case (3) — it prevents memory-safety violations while allowing logically invalid indices that happen to land in-bounds. This is acceptable per the established precedent.

### 2.4 Struct-with-Array Access: `s.arr[i]`

Given:
```
TYPE WithArray : STRUCT
    tag    : INT;                       (* slot 0, 1 slot *)
    values : ARRAY[1..4] OF REAL;       (* slots 1-4, 4 slots *)
    count  : INT;                       (* slot 5, 1 slot *)
END_STRUCT; END_TYPE

VAR
    w : WithArray;   (* data_offset = D *)
END_VAR

w.values[3] := 7.5;
```

Resolution:
1. `w` → `data_offset = D`
2. `values` starts at slot index 1 within `WithArray`
3. Array subscript `[3]`: `flat_index = 3 - 1 = 2` (0-based)
4. Byte offset: `D + (1 + 2) * 8 = D + 24`

**With variable subscript**: The compiler emits:
1. Compute `flat_index` from the subscript expression
2. Add the field's slot offset: `flat_index + 1`
3. Multiply by 8: `(flat_index + 1) * 8`
4. Add base: `+ D`

**Bounds checking**: The array has 4 elements starting at slot 1. The bounds check ensures `0 ≤ flat_index < 4`. The array descriptor for this embedded array records `total_elements = 4`.

### 2.5 Complex Composition: `arr[i].inner.values[j]`

Given:
```
TYPE Complex : STRUCT
    id     : INT;                  (* slot 0 *)
    inner  : WithArray;            (* slots 1-6, 6 slots inline *)
    flag   : BOOL;                 (* slot 7 *)
END_STRUCT; END_TYPE

VAR
    items : ARRAY[0..2] OF Complex;   (* data_offset = D, 3 elements, 8 slots each *)
END_VAR

items[1].inner.values[2] := 42.0;
```

Resolution:
1. `items` → `data_offset = D`, struct stride = 8 slots
2. `[1]`: `flat_index = 1 - 0 = 1`, struct base = `1 * 8 = 8` slots from D
3. `inner`: slot offset 1 within Complex
4. `values`: slot offset 1 within WithArray (relative to inner's start)
5. `[2]`: `flat_index = 2 - 1 = 1`
6. Total slot offset: `8 + 1 + 1 + 1 = 11`
7. Byte offset: `D + 11 * 8 = D + 88`

When `i` is a variable but `j` is constant:
- Emit: `D + (flat_index_i * 8 + 1 + 1 + 1) * 8 = D + (flat_index_i * 8 + 3) * 8`
- Bounds check `flat_index_i` against `total_elements = 3`

When both `i` and `j` are variables:
- Two bounds checks needed: `flat_index_i < 3` and `flat_index_j < 4`
- Byte offset: `D + (flat_index_i * 8 + 1 + 1 + flat_index_j) * 8`

---

## 3. Initialization

### 3.1 Structure Variable Initialization

During the init function (function 0), the compiler emits code to initialize each field of the structure variable.

**Default initialization** (no explicit initial value):
- Numeric fields: 0 (matching IEC 61131-3 defaults)
- Boolean fields: FALSE
- Enumeration fields: first enumeration value (or 0)

The init function emits a constant load + data region store for each field:

```
; Initialize pt.x to 0.0 (default REAL)
LOAD_CONST_F32 pool_idx_0    ; push 0.0
; store to data_region[D + 0]  (field x)
...

; Initialize pt.y to 0.0
LOAD_CONST_F32 pool_idx_0    ; push 0.0
; store to data_region[D + 8]  (field y)
...
```

**Explicit initialization**:

```
VAR
    pt : Point := (x := 1.0, y := 2.0);
END_VAR
```

The compiler matches each element in `StructureInitializationDeclaration.elements_init` to the corresponding field, then emits a constant load + store for the specified value. Fields without explicit initializers use their default values.

### 3.2 Nested Structure Initialization

For nested structures, initialization is recursive. Each leaf field gets its own constant + store instruction pair. The compiler flattens the nested structure initialization into a linear sequence of field-by-field stores.

### 3.3 Array-of-Struct Initialization

Arrays of structures are initialized element by element, field by field. For an `ARRAY[1..3] OF Point`, the init function emits 6 stores (3 elements × 2 fields).

---

## 4. Compiler Data Structures

### 4.1 StructVarInfo

New metadata structure for the `CompileContext`:

```
StructVarInfo {
    var_index: u16,                     // slot table index
    data_offset: u32,                   // byte offset in data region
    total_slots: u32,                   // total 8-byte slots for this variable
    field_map: HashMap<String, StructFieldInfo>,  // field name → info
}

StructFieldInfo {
    slot_offset: u32,                   // slot offset relative to struct base
    field_type: IntermediateType,       // for nested resolution
    op_type: OpType,                    // (OpWidth, Signedness) for leaf fields
}
```

For nested structures, the `field_map` is flat at each level — resolving `outer.inner.x` means:
1. Look up `outer` in `ctx.struct_vars` → `StructVarInfo`
2. Look up `inner` in the `field_map` → `StructFieldInfo { slot_offset: 1, field_type: Structure { ... } }`
3. Look up `x` in the nested structure's fields → `StructFieldInfo { slot_offset: 0, ... }`
4. Sum: `1 + 0 = 1`, byte offset: `data_offset + 1 * 8`

### 4.2 Integration with Existing CompileContext

The `CompileContext` gains a new field:

```
struct_vars: HashMap<Id, StructVarInfo>,
```

This parallels the existing `string_vars`, `fb_instances`, and `array_vars` maps.

### 4.3 Type Resolution

The codegen needs to resolve structure type names to their `IntermediateType::Structure` representation. The `TypeEnvironment` is passed to `assign_variables` and can be queried for named types. A new helper similar to `resolve_array_type` is needed:

```
fn resolve_struct_type(&self, type_name: &TypeName) -> Option<&IntermediateType>
```

---

## 5. Data Region Access Mechanism

### 5.1 Current Array Access Pattern

Arrays use `LOAD_ARRAY(var_index, desc_index)` and `STORE_ARRAY(var_index, desc_index)`:
1. Pop `flat_index` from the stack
2. Read `data_offset` from `variable_table[var_index]`
3. Bounds check: `0 ≤ flat_index < descriptor.total_elements`
4. Compute byte address: `data_offset + flat_index * 8`
5. Read/write 8 bytes at that address

### 5.2 Structure Field Access via Array Infrastructure

For structure fields with compile-time-known offsets, the compiler can:

**Approach A — Direct data region read/write opcodes**: If the VM has (or gains) a generic "read/write 8 bytes at a constant data region offset" instruction, the compiler emits the pre-computed offset as an operand.

**Approach B — Reuse LOAD_ARRAY/STORE_ARRAY**: Treat the structure as an array of slots. Register an array descriptor with `total_elements = total_slots`. Field access becomes `flat_index = field_slot_offset` (a constant). This reuses existing infrastructure but requires registering a descriptor per struct variable.

**Approach C — New load/store-by-offset opcodes**: Add `LOAD_DATA_I32(data_offset)` / `STORE_DATA_I32(data_offset)` etc. These read/write a slot at a compile-time-constant byte offset in the data region. This is the most direct approach and avoids the descriptor overhead of Approach B.

**Recommendation**: Approach C for constant-offset field access (most common case), falling back to Approach B (array infrastructure) for variable-subscript array-of-struct access. Approach C requires new opcodes (2-8 depending on type width variants), but these are generic data-region load/store opcodes useful beyond structures.

However, if opcode budget preservation is preferred, Approach B works entirely with existing opcodes at the cost of one array descriptor per structure variable. This is the simpler path for initial implementation.

**Decision**: Start with Approach B (reuse LOAD_ARRAY/STORE_ARRAY) for the initial implementation. Evaluate adding direct data-region load/store opcodes as a follow-up optimization if the descriptor-per-variable overhead is problematic.

---

## 6. Worked Examples

### 6.1 Simple Structure

```
TYPE Point : STRUCT
    x : REAL;
    y : REAL;
END_STRUCT; END_TYPE

PROGRAM Main
VAR
    pt : Point := (x := 1.0, y := 2.0);
    dist : REAL;
END_VAR
    dist := pt.x + pt.y;
END_PROGRAM
```

**Allocation** (in `assign_variables`):
- `pt`: var_index = 0, data_offset = 0, total_slots = 2
- `dist`: var_index = 1 (scalar, no data region allocation)
- Array descriptor 0: `total_elements = 2` (for pt's slot array)
- `data_region_offset` advances to 16

**Init function**:
```
; pt.x := 1.0
LOAD_CONST_F32 [pool: 1.0]
LOAD_CONST_I32 [pool: 0]      ; flat_index = 0 (field x)
STORE_ARRAY var=0, desc=0

; pt.y := 2.0
LOAD_CONST_F32 [pool: 2.0]
LOAD_CONST_I32 [pool: 1]      ; flat_index = 1 (field y)
STORE_ARRAY var=0, desc=0
```

**Scan function** (`dist := pt.x + pt.y`):
```
LOAD_CONST_I32 [pool: 0]      ; flat_index = 0 (field x)
LOAD_ARRAY var=0, desc=0      ; push pt.x
LOAD_CONST_I32 [pool: 1]      ; flat_index = 1 (field y)
LOAD_ARRAY var=0, desc=0      ; push pt.y
ADD_F32                        ; push pt.x + pt.y
STORE_VAR_F32 var=1            ; dist := result
```

### 6.2 Nested Structure

```
TYPE Rect : STRUCT
    topLeft  : Point;
    botRight : Point;
END_STRUCT; END_TYPE

PROGRAM Main
VAR
    r : Rect;
END_VAR
    r.botRight.x := 10.0;
END_PROGRAM
```

**Allocation**:
- `r`: var_index = 0, data_offset = 0, total_slots = 4
- Array descriptor 0: `total_elements = 4`

**Field resolution for `r.botRight.x`**:
- `r` → data_offset = 0
- `botRight` → slot offset 2 within Rect (topLeft occupies slots 0-1)
- `x` → slot offset 0 within Point
- flat_index = 2 + 0 = 2

**Bytecode**:
```
LOAD_CONST_F32 [pool: 10.0]
LOAD_CONST_I32 [pool: 2]      ; flat_index = 2
STORE_ARRAY var=0, desc=0
```

### 6.3 Array of Structures

```
PROGRAM Main
VAR
    points : ARRAY[1..3] OF Point;
    i : INT := 2;
END_VAR
    points[i].y := 5.0;
END_PROGRAM
```

**Allocation**:
- `points`: var_index = 0, data_offset = 0, total_slots = 6 (3 * 2)
- `i`: var_index = 1 (scalar)
- Array descriptor 0: `total_elements = 6`

**Field resolution for `points[i].y`**:
- `points` → array of Point, stride = 2 slots
- `[i]`: variable subscript, `flat_array_index = i - 1` (0-based)
- `y` → slot offset 1 within Point
- `flat_slot_index = flat_array_index * 2 + 1`

**Bytecode**:
```
LOAD_CONST_F32 [pool: 5.0]    ; value to store

; Compute flat_slot_index
LOAD_VAR_I32 var=1             ; push i (as i32)
; sign-extend to i64 for index arithmetic (per ADR-0023 pattern)
...
LOAD_CONST_I64 [pool: 1]      ; lower bound
SUB_I64                        ; flat_array_index = i - 1
LOAD_CONST_I64 [pool: 2]      ; stride
MUL_I64                        ; flat_array_index * stride
LOAD_CONST_I64 [pool: 1]      ; field slot offset for y
ADD_I64                        ; flat_slot_index

STORE_ARRAY var=0, desc=0     ; bounds-checked store
```

---

## 7. Type Safety Considerations

### 7.1 Field Type Tracking

The compiler tracks the `OpType` (OpWidth + Signedness) for each structure field. When emitting LOAD_ARRAY/STORE_ARRAY for a field access, the compiler must ensure the value on the stack matches the field's type. Narrow types (SINT, INT, USINT, UINT) use the promote-operate-truncate strategy (compile.rs line 30-35):
- After loading a narrow field: the value is already promoted (stored as i32 in the 8-byte slot)
- Before storing to a narrow field: emit truncation (TRUNC_I8, TRUNC_U8, TRUNC_I16, TRUNC_U16)

### 7.2 Verifier Implications

The bytecode verifier sees LOAD_ARRAY/STORE_ARRAY operations on what appear to be array variables. It validates:
- Variable has an array descriptor
- Stack depth is correct (value + index for store, index for load)
- Type byte matches descriptor's element type

For structures reusing the array infrastructure, the descriptor's `element_type` should be a generic type tag (e.g., `I64` or a new `SLOT` tag) since structure fields are heterogeneous. The verifier checks bounds but cannot enforce per-field type correctness — this is acceptable because field type correctness is guaranteed by the compiler's type checker.

---

## 8. Future Work

The following items are explicitly deferred and should be tracked as TODOs:

1. **STRING/WSTRING fields in structures** — requires sub-allocating string layouts within the structure's data region block and copy semantics for string assignment
2. **Function block fields in structures** — requires FB instance lifecycle management for embedded instances
3. **Whole-structure assignment** (`s1 := s2`) — requires emitting a field-by-field copy loop or a memcpy-like bulk operation
4. **Structure literals in expressions** — requires temporary structure allocation on the stack or in the data region
5. **Packed byte-level layout** — ADR-0026 migration path
6. **Direct data-region load/store opcodes** — Approach C from section 5.2, to eliminate array descriptor overhead for constant-offset access
7. **Structure type descriptors in the container** — for debugger/verifier support
