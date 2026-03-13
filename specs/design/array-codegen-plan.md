# Array Code Generation — Implementation Plan

## Overview

Add code generation and VM support for arrays of primitive types. This document has enough detail for an implementer to work from without additional design decisions.

**Scope**: Arrays of primitive types (INT, REAL, BOOL, SINT, DINT, LINT, USINT, UINT, UDINT, ULINT, BYTE, WORD, DWORD, LWORD, TIME, LTIME). Arrays of STRING, WSTRING, structs, and function blocks are deferred.

**Prerequisite reading**: ADR-0023 (array bounds safety), ADR-0017 (unified data region), ADR-0005 (safety-first principle).

### Key Design Decisions

1. **Element size**: 8 bytes (slot-sized) per element in the data region. Matches FB field storage. Future optimization to pack elements deferred.

2. **Always 0-based descriptors**: The compiler normalizes all subscripts to 0-based flat indices before emitting `LOAD_ARRAY`/`STORE_ARRAY`. Descriptors always store `lower_bound=0, upper_bound=total_elements-1`. Original IEC bounds live in the debug section.

3. **Bounds safety**: Three layers — compile-time per-dimension for constants, runtime flat check in VM, load-time verifier check. See ADR-0023.

---

## Step 1: Opcode Constants

**File**: `compiler/container/src/opcode.rs`

Add two constants matching the instruction set spec:

```
pub const LOAD_ARRAY: u8 = 0x24;
pub const STORE_ARRAY: u8 = 0x25;
```

These opcodes have:
- Operand 1: `u16` (little-endian) — variable table index of the array variable
- Operand 2: `u8` — element type byte

Element type bytes (from instruction set spec):
| Byte | Type |
|------|------|
| 0 | I32 |
| 1 | U32 |
| 2 | I64 |
| 3 | U64 |
| 4 | F32 |
| 5 | F64 |

Stack effects:
- `LOAD_ARRAY`: pops 1 (index), pushes 1 (value) → net 0
- `STORE_ARRAY`: pops 2 (value, index) → net -2

---

## Step 2: Emitter Methods

**File**: `compiler/codegen/src/emit.rs`

Add two methods. These do NOT fit the existing macros because they have both a `u16` and a `u8` operand.

**`emit_load_array(var_index: u16, type_byte: u8)`**:
- Emit `LOAD_ARRAY` opcode byte
- Emit `var_index` as 2 LE bytes
- Emit `type_byte` as 1 byte
- Stack effect: pop 1 (the index is already on the stack), push 1 (the loaded value). Net: 0. The simplest implementation: call `self.pop_stack(1)` then `self.push_stack(1)`, or just no stack change.

**`emit_store_array(var_index: u16, type_byte: u8)`**:
- Emit `STORE_ARRAY` opcode byte
- Emit `var_index` as 2 LE bytes
- Emit `type_byte` as 1 byte
- Stack effect: pop 2 (value and index). Call `self.pop_stack(2)`.

---

## Step 3: Array Descriptors in the Container

Array descriptors tell the VM the bounds and element type for runtime checking.

### 3a: Add `ArrayDescriptor` struct

**File**: `compiler/container/src/type_section.rs` (extend the existing type section)

```
pub struct ArrayDescriptor {
    pub element_type: u8,    // same encoding as element type byte in opcodes
    pub total_elements: u16, // number of elements (upper_bound + 1)
}
```

On disk, each descriptor is 8 bytes (per container format spec):
```
[element_type: u8] [reserved: u8 = 0] [lower_bound: i16 LE = 0] [upper_bound: i16 LE = total_elements - 1] [element_extra: u16 LE = 0]
```

`element_extra` is reserved for future use (e.g., for arrays of strings, it would hold the max string length; for arrays of structs, it would hold a type descriptor index).

### 3b: Extend `TypeSection`

Add `pub array_descriptors: Vec<ArrayDescriptor>` to `TypeSection`. Update serialization:
- After writing FB types, write array descriptor count (`u16 LE`) followed by each descriptor (8 bytes each)
- Update deserialization symmetrically

### 3c: Add header field for array descriptor count

**File**: `compiler/container/src/header.rs`

Add `pub num_array_descriptors: u16` to `FileHeader`. This replaces 2 bytes from the `reserved` field (bytes 216-217, adjust as needed based on current layout). Update serialization/deserialization.

### 3d: Update `ContainerBuilder`

**File**: `compiler/container/src/builder.rs`

Add method:
```
pub fn add_array_descriptor(mut self, element_type: u8, total_elements: u16) -> (Self, u16)
```
Returns `(self, descriptor_index)`. The descriptor index is used to link the variable to its descriptor.

Add a way to mark a variable as an array variable. Two approaches:
1. **Simple**: Add a separate `array_var_flags: HashMap<u16, u16>` mapping var_index → descriptor_index. The builder applies these during `build()`.
2. **VarEntry flags**: If the container already has a per-variable metadata section, add `is_array` flag and `extra` field per the container format spec.

Check the current container to see if per-variable metadata already exists. If not, the simplest approach is to encode the mapping in the type section alongside the descriptors. The VM can build a lookup from variable index to descriptor at load time.

**Recommendation**: Store the mapping as part of the array descriptor itself — add `var_index: u16` to each descriptor. This avoids changes to the variable table format. The VM builds a `HashMap<u16, usize>` (var_index → descriptor_index) at container load time.

Updated on-disk descriptor (still 8 bytes):
```
[var_index: u16 LE] [element_type: u8] [reserved: u8 = 0] [upper_bound: i16 LE = total_elements - 1] [element_extra: u16 LE = 0]
```

This removes `lower_bound` since it's always 0 (the signed check is just `index >= 0 && index <= upper_bound`, equivalent to `(index as u32) <= upper_bound as u32` since both are non-negative).

---

## Step 4: Array Tracking in Codegen

**File**: `compiler/codegen/src/compile.rs`

### 4a: New types

```rust
/// Metadata for a single dimension of an array, used for index computation.
struct DimensionInfo {
    lower_bound: i32,  // IEC declared lower bound (e.g., 1, -5, 0)
    size: u32,         // number of elements in this dimension (upper - lower + 1)
    stride: u32,       // product of all subsequent dimension sizes
}

/// Metadata for an array variable, stored in CompileContext.
struct ArrayVarInfo {
    var_index: u16,           // slot table index (slot holds data_offset)
    data_offset: u16,         // byte offset in data region where elements start
    descriptor_index: u16,    // index into container's array descriptor table
    element_type_byte: u8,    // type byte for LOAD_ARRAY/STORE_ARRAY
    element_var_type_info: VarTypeInfo,  // element's VarTypeInfo for truncation/widening
    total_elements: u32,      // total flattened element count
    dimensions: Vec<DimensionInfo>,  // per-dimension bounds and strides
}
```

### 4b: Add to `CompileContext`

```rust
struct CompileContext {
    // ... existing fields ...
    array_vars: HashMap<Id, ArrayVarInfo>,
    array_descriptors: Vec<(u16, u8, u16)>,  // (var_index, element_type, total_elements)
}
```

### 4c: Thread `TypeEnvironment` through

The `compile()` function receives `_types: &TypeEnvironment` but ignores it. Change to `types: &TypeEnvironment` and pass it to `assign_variables()` and other functions that need array type resolution. This is needed to resolve user-defined array type names (e.g., `TYPE MY_ARRAY : ARRAY[1..10] OF INT; END_TYPE`).

---

## Step 5: Handle `InitialValueAssignmentKind::Array` in `assign_variables()`

**File**: `compiler/codegen/src/compile.rs`, function `assign_variables()`

Currently, the `match` on `decl.initializer` handles `Simple`, `String`, `FunctionBlock`, etc. Add a case for `Array`:

```
InitialValueAssignmentKind::Array(array_init) => {
    match &array_init.spec {
        SpecificationKind::Inline(array_subranges) => {
            // 1. Resolve element type
            let element_type_name = &array_subranges.type_name;
            let element_vti = resolve_type_name(&Id::from(element_type_name.to_string()))
                .ok_or_else(|| /* unsupported element type error */)?;

            // 2. Extract dimension info from subranges
            let mut dimensions: Vec<DimensionInfo> = Vec::new();
            let mut total_elements: u32 = 1;
            for range in &array_subranges.ranges {
                let lower = signed_integer_value(&range.start);
                let upper = signed_integer_value(&range.end);
                let size = (upper - lower + 1) as u32;
                dimensions.push(DimensionInfo { lower_bound: lower as i32, size, stride: 0 });
                total_elements *= size;
            }

            // 3. Compute strides (reverse pass)
            // stride[last] = 1, stride[k] = stride[k+1] * size[k+1]
            let n = dimensions.len();
            if n > 0 {
                dimensions[n - 1].stride = 1;
                for k in (0..n-1).rev() {
                    dimensions[k].stride = dimensions[k + 1].stride * dimensions[k + 1].size;
                }
            }

            // 4. Allocate data region space
            let data_offset = ctx.data_region_offset;
            let total_bytes = (total_elements as u16) * 8;  // 8 bytes per element
            ctx.data_region_offset = ctx.data_region_offset
                .checked_add(total_bytes)
                .ok_or_else(|| /* data region overflow error */)?;

            // 5. Determine element type byte
            let element_type_byte = var_type_info_to_type_byte(&element_vti);

            // 6. Record array descriptor
            let descriptor_index = ctx.array_descriptors.len() as u16;
            ctx.array_descriptors.push((index, element_type_byte, total_elements as u16));

            // 7. Store in context
            ctx.array_vars.insert(id.clone(), ArrayVarInfo {
                var_index: index,
                data_offset,
                descriptor_index,
                element_type_byte,
                element_var_type_info: element_vti,
                total_elements,
                dimensions,
            });
        }
        SpecificationKind::Named(type_name) => {
            // Named array type — look up in TypeEnvironment to get the
            // inline specification, then process as above.
            // This is where the threaded TypeEnvironment is needed.
        }
    }
}
```

**Helper function needed**: `var_type_info_to_type_byte(vti: &VarTypeInfo) -> u8` that maps from VarTypeInfo to the element type byte:

| VarTypeInfo | Type byte |
|------------|-----------|
| W32 + Signed (SINT, INT, DINT, BOOL, TIME) | 0 (I32) |
| W32 + Unsigned (USINT, UINT, UDINT, BYTE, WORD, DWORD) | 1 (U32) |
| W64 + Signed (LINT, LTIME) | 2 (I64) |
| W64 + Unsigned (ULINT, LWORD) | 3 (U64) |
| F32 (REAL) | 4 (F32) |
| F64 (LREAL) | 5 (F64) |

**Helper function needed**: `signed_integer_value(si: &SignedInteger) -> i64` that extracts the signed value from the AST's `SignedInteger` (checking `is_neg` flag).

---

## Step 6: Array Initialization in `emit_initial_values()`

**File**: `compiler/codegen/src/compile.rs`

In the initialization function, for each array variable:

1. **Store data_offset in slot**: Like FB instances, emit `LOAD_CONST_I32 data_offset`, `STORE_VAR var_index`. This makes the variable's slot hold the data region pointer.

2. **Initialize elements**: For arrays with no explicit initial values, the data region is zero-initialized by the VM — nothing to emit. For arrays with initial values (`array_init.initial_values`), emit stores for each element:

```
// For each initial element at flat index i:
LOAD_CONST_I32 <initial_value>
LOAD_CONST_I32 <i>              // 0-based flat index
STORE_ARRAY var_index, type_byte
```

Handle `ArrayInitialElementKind::Constant(value)` — emit the constant.
Handle `ArrayInitialElementKind::Repeated(count, value)` — unroll into `count` stores of the value.

**Note**: Initialization uses `STORE_ARRAY` with constant indices, which means the VM will bounds-check during init. This is correct and safe.

---

## Step 7: Compile Array Access (the `todo!()` replacement)

**File**: `compiler/codegen/src/compile.rs`

The `todo!()` is in the `SymbolicVariableKind::Array` match arm. There are two contexts: reading an array element (in an expression) and writing to an array element (as an assignment target).

### 7a: Resolve array variable info

Add a helper function `resolve_array_access(variable: &ArrayVariable, ctx: &CompileContext) -> (Id, Vec<Expr>, ArrayVarInfo)` that:

1. Recursively walks the `ArrayVariable` chain (handling nested arrays)
2. Collects all subscript expressions into a flat list
3. Returns the base variable name, flattened subscripts, and ArrayVarInfo

For nested arrays `arr[i][j]`, the AST is:
```
ArrayVariable {
    subscripted_variable: Array(ArrayVariable {
        subscripted_variable: Named(Id("arr")),
        subscripts: [i],
    }),
    subscripts: [j],
}
```

Walk recursively: collect subscripts in order `[i, j]`, resolve base name "arr", look up `array_vars["arr"]`.

### 7b: Emit index computation

Add a helper function `emit_flat_index(emitter, ctx, subscripts: &[Expr], dimensions: &[DimensionInfo]) -> Result<(), Diagnostic>` that:

1. Validates that `subscripts.len() == dimensions.len()` (else diagnostic error)
2. Checks if all subscripts are constant expressions
3. If all constant: validate per-dimension bounds, compute flat index at compile time, emit `LOAD_CONST flat_index`
4. If any variable: emit runtime computation

**All-constant case**:
```
flat_index = 0
for k in 0..N:
    value = evaluate_constant_expr(subscripts[k])
    if value < dimensions[k].lower_bound || value > dimensions[k].lower_bound + dimensions[k].size - 1:
        return Err(compile-time bounds error)
    flat_index += (value - dimensions[k].lower_bound) * dimensions[k].stride

emit LOAD_CONST_I32 flat_index
```

**Variable case** (any subscript is non-constant):
```
// Emit code for: (s_0 - l_0) * stride_0 + (s_1 - l_1) * stride_1 + ...
for k in 0..N:
    compile_expr(subscripts[k])       // pushes subscript value
    if dimensions[k].lower_bound != 0:
        emit LOAD_CONST_I32 dimensions[k].lower_bound
        emit SUB_I32
    if dimensions[k].stride != 1:
        emit LOAD_CONST_I32 dimensions[k].stride
        emit MUL_I32
    if k > 0:
        emit ADD_I32                  // accumulate into running sum
```

After this sequence, the stack has exactly one I32: the 0-based flat index.

### 7c: Array read (in expression context)

When compiling an expression and encountering `SymbolicVariableKind::Array`:

```
let (base_name, subscripts, info) = resolve_array_access(array_var, ctx);
emit_flat_index(emitter, ctx, &subscripts, &info.dimensions)?;
emitter.emit_load_array(info.var_index, info.element_type_byte);
// May need truncation for sub-32-bit types (SINT, BOOL, BYTE, etc.)
// Use the same truncation pattern as scalar LOAD_VAR
```

### 7d: Array write (in assignment context)

In `compile_assignment()`, when the assignment target is `SymbolicVariableKind::Array`:

```
// First compile the RHS value (pushes value onto stack)
compile_expr(emitter, ctx, &assignment.value, op_type)?;
// Then compute the index (pushes index onto stack)
let (base_name, subscripts, info) = resolve_array_access(array_var, ctx);
emit_flat_index(emitter, ctx, &subscripts, &info.dimensions)?;
// Stack now has: [..., value, index]
// STORE_ARRAY pops both value and index
emitter.emit_store_array(info.var_index, info.element_type_byte);
```

**IMPORTANT**: Check the stack order. `STORE_ARRAY` pops index first (top), then value (second). The RHS value must be compiled first, then the index. Verify this matches the instruction set spec's stack convention. If the spec says `[value, I32] → []`, then value is below index on the stack, which matches the order above (value pushed first, index pushed second, index is TOS).

---

## Step 8: VM Implementation

### 8a: New trap type

**File**: `compiler/vm/src/error.rs`

Add to the `Trap` enum:
```
ArrayIndexOutOfBounds { var_index: u16, index: i32, total_elements: u16 }
```

Implement `Display` to show: `"array index out of bounds: index {index} for array variable {var_index} with {total_elements} elements"`.

**File**: `compiler/vm/resources/problem-codes.csv`

Add: `V4005,ArrayIndexOutOfBounds,Array index out of bounds,true`

### 8b: Array descriptor table in VM

**File**: `compiler/vm/src/vm.rs` (or a new `compiler/vm/src/array.rs` if the file is already large)

At container load time, parse the array descriptor section from the type section and build a lookup:

```rust
struct ArrayDescriptor {
    total_elements: u16,
    element_type: u8,
}

// In the VM's loaded state:
array_descriptors: HashMap<u16, ArrayDescriptor>  // var_index → descriptor
```

### 8c: LOAD_ARRAY handler

In the VM's opcode dispatch:

```
LOAD_ARRAY => {
    let var_index = read_u16_operand();
    let type_byte = read_u8_operand();
    let index = stack.pop_i32();

    let desc = array_descriptors.get(&var_index)
        .ok_or(Trap::InvalidVariable(var_index))?;

    // Bounds check: 0 <= index < total_elements
    if index < 0 || index as u32 >= desc.total_elements as u32 {
        return Err(Trap::ArrayIndexOutOfBounds {
            var_index,
            index,
            total_elements: desc.total_elements,
        });
    }

    // Read data_offset from the variable's slot
    let data_offset = slots[var_index].as_u32() as usize;

    // Compute byte offset into data region
    let byte_offset = data_offset + (index as usize) * 8;

    // Bounds-check data region access (defense-in-depth)
    if byte_offset + 8 > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(byte_offset as u16));
    }

    // Read 8 bytes as i64
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&data_region[byte_offset..byte_offset + 8]);
    let raw = i64::from_le_bytes(buf);

    // Interpret per type_byte and push
    match type_byte {
        0 => stack.push_i32(raw as i32),   // I32
        1 => stack.push_u32(raw as u32),   // U32  (or push as i32 — check VM's value representation)
        2 => stack.push_i64(raw),          // I64
        3 => stack.push_u64(raw as u64),   // U64
        4 => stack.push_f32(f32::from_bits(raw as u32)),  // F32
        5 => stack.push_f64(f64::from_bits(raw as u64)),  // F64
        _ => return Err(Trap::InvalidOpcode),
    }
}
```

**Note**: Check how the VM currently pushes different types. The existing FB_LOAD_FIELD handler is the closest pattern — study `compiler/vm/src/vm.rs:1458-1462` for the read pattern.

### 8d: STORE_ARRAY handler

Symmetric to LOAD_ARRAY. Stack has `[..., value, index]`:

```
STORE_ARRAY => {
    let var_index = read_u16_operand();
    let type_byte = read_u8_operand();
    let index = stack.pop_i32();  // TOS = index
    let value = stack.pop_i64();  // second = value (as raw i64)

    // Same bounds check as LOAD_ARRAY
    // Same data_offset and byte_offset computation

    // Write value to data region
    data_region[byte_offset..byte_offset + 8].copy_from_slice(&value.to_le_bytes());
}
```

**Stack order**: Verify from instruction set spec. If spec says `[value, I32] → []`, then I32 (index) is on top, value is below. Pop order: index first, then value. This matches above.

---

## Step 9: Tests

### Codegen tests

**File**: `compiler/codegen/tests/` (follow existing test patterns)

Use existing test infrastructure to compile ST programs and inspect generated bytecode.

| Test name | ST program | Verifies |
|-----------|-----------|----------|
| `array_1d_constant_index_load` | `VAR arr: ARRAY[1..5] OF INT; END_VAR x := arr[3];` | Emits `LOAD_CONST 2, LOAD_ARRAY` (0-based: 3-1=2) |
| `array_1d_constant_index_store` | `arr[3] := 42;` | Emits `LOAD_CONST 42, LOAD_CONST 2, STORE_ARRAY` |
| `array_1d_variable_index_load` | `x := arr[i];` | Emits `LOAD_VAR i, LOAD_CONST 1, SUB_I32, LOAD_ARRAY` |
| `array_1d_variable_index_store` | `arr[i] := 42;` | Emits `LOAD_CONST 42, LOAD_VAR i, LOAD_CONST 1, SUB_I32, STORE_ARRAY` |
| `array_multidim_constant_index` | `matrix[2,3]` where `ARRAY[1..3,1..4]` | Flat index = (2-1)*4+(3-1) = 6 |
| `array_multidim_variable_index` | `matrix[i,j]` | Emits SUB, MUL, SUB, ADD sequence |
| `array_nonzero_lower_bound` | `ARRAY[-5..5] OF INT`, access `arr[0]` | 0-based index = 0-(-5) = 5 |
| `array_constant_oob_error` | `arr[11]` on `ARRAY[1..10]` | Compile-time diagnostic error |
| `array_initialization` | `ARRAY[1..3] OF INT := [10, 20, 30]` | Emits STORE_ARRAY for each element |

### VM tests

**File**: `compiler/vm/tests/` (follow existing test patterns)

Build containers programmatically using `ContainerBuilder`, execute, check results.

| Test name | What it tests |
|-----------|--------------|
| `load_array_when_in_bounds_then_returns_value` | Store value at index 3, LOAD_ARRAY at index 3, verify value |
| `store_array_when_in_bounds_then_persists` | STORE_ARRAY at index 0, read back, verify |
| `load_array_when_index_too_large_then_traps` | LOAD_ARRAY with index = total_elements, expect trap |
| `load_array_when_index_negative_then_traps` | LOAD_ARRAY with index = -1, expect trap |
| `array_when_multiple_types_then_correct` | Test with I32, I64, F32, F64 element types |
| `array_when_multiple_elements_then_independent` | Store different values at indices 0,1,2, verify each independently |

### Integration tests

End-to-end ST programs compiled and run:

```iec
PROGRAM ArrayTest
VAR
    arr : ARRAY[1..5] OF INT;
    sum : INT := 0;
    i : INT;
END_VAR
    arr[1] := 10;
    arr[2] := 20;
    arr[3] := 30;
    arr[4] := 40;
    arr[5] := 50;

    FOR i := 1 TO 5 DO
        sum := sum + arr[i];
    END_FOR;
    (* sum should be 150 *)
END_PROGRAM
```

---

## Step 10: Bytecode Verifier Spec Update (no implementation)

**File**: `specs/design/bytecode-verifier-rules.md`

Add these verification rules for documentation purposes (verifier implementation is a separate task):

1. `LOAD_ARRAY`/`STORE_ARRAY` operand `var_index` must have a corresponding array descriptor
2. The `type_byte` operand must match the descriptor's `element_type`
3. Stack must have I32 on top when `LOAD_ARRAY`/`STORE_ARRAY` execute
4. For `STORE_ARRAY`, the value below the index must be compatible with the element type

---

## Implementation Order

```
Step 1: Opcode constants           ← no dependencies
Step 2: Emitter methods            ← depends on Step 1
Step 3: Container descriptors      ← depends on Step 1
Step 4: CompileContext types        ← no dependencies (just struct definitions)
Step 5: assign_variables()         ← depends on Steps 3, 4
Step 6: emit_initial_values()      ← depends on Steps 2, 5
Step 7: Compile array access       ← depends on Steps 2, 5
Step 8: VM implementation          ← depends on Steps 1, 3
Step 9: Tests                      ← depends on everything above
Step 10: Verifier spec             ← can be done anytime
```

Steps 1-4 can be done as a single commit (foundational types). Steps 5-7 form the codegen commit. Step 8 is the VM commit. Step 9 spans all commits.

Recommended PR structure:
1. **PR 1**: Steps 1-4 + Step 8 (container + VM — the "bottom half")
2. **PR 2**: Steps 5-7 + Step 9 (codegen + tests — the "top half")
3. **PR 3**: Step 10 (verifier spec — documentation only)

Or as a single PR if preferred.

---

## Key Files Reference

| File | What changes |
|------|-------------|
| `compiler/container/src/opcode.rs` | Add `LOAD_ARRAY`, `STORE_ARRAY` constants |
| `compiler/container/src/type_section.rs` | Add `ArrayDescriptor` struct, extend `TypeSection` serialization |
| `compiler/container/src/header.rs` | Add `num_array_descriptors` field |
| `compiler/container/src/builder.rs` | Add `add_array_descriptor()` method |
| `compiler/codegen/src/emit.rs` | Add `emit_load_array()`, `emit_store_array()` |
| `compiler/codegen/src/compile.rs` | `ArrayVarInfo`, `DimensionInfo`, changes to `assign_variables()`, `emit_initial_values()`, array access compilation, thread `TypeEnvironment` |
| `compiler/vm/src/error.rs` | Add `ArrayIndexOutOfBounds` trap variant |
| `compiler/vm/resources/problem-codes.csv` | Add `V4005` |
| `compiler/vm/src/vm.rs` | Add LOAD_ARRAY and STORE_ARRAY handlers, array descriptor loading |
| `specs/design/bytecode-verifier-rules.md` | Add array verification rules |

## Risks and Open Questions

1. **`data_region_offset` is `u16`**: Arrays can be large. `ARRAY[1..1000] OF LINT` needs 8000 bytes. The current `u16` `data_region_offset` caps at 65535 bytes, which limits total data region to ~64KB. This is likely sufficient for initial PLC programs but may need to be widened to `u32` in the future.

2. **`total_elements` in descriptor is `u16`**: Caps at 65535 elements. `ARRAY[1..100, 1..100]` is 10000 elements (OK). `ARRAY[1..1000, 1..1000]` is 1M elements (not OK). The analyzer already validates array sizes fit in `u32`, but the descriptor format limits to `u16`. This may need widening if large arrays are common.

3. **Named array types**: `TYPE MY_ARRAY : ARRAY[1..10] OF INT; END_TYPE; VAR arr : MY_ARRAY; END_VAR`. This flows through `InitialValueAssignmentKind::LateResolvedType` or `::Array` with `SpecificationKind::Named`. The codegen needs to look up the TypeEnvironment to resolve bounds. Make sure this path works.

4. **Mixed constant/variable subscripts**: `matrix[2, j]` has one constant and one variable subscript. The constant subscript can be bounds-checked at compile time, but the flat index must still be computed at runtime. Handle this by treating all subscripts as variable when any one is variable (simpler), or optimize the constant ones to compile-time values (emit `LOAD_CONST (2-1)` instead of `LOAD_VAR, SUB`).
