# Array Code Generation — Implementation Plan

## Overview

Add code generation and VM support for arrays of primitive types. This document has enough detail for an implementer to work from without additional design decisions.

**Scope**: Arrays of primitive types (INT, REAL, BOOL, SINT, DINT, LINT, USINT, UINT, UDINT, ULINT, BYTE, WORD, DWORD, LWORD, TIME, LTIME). Arrays of STRING, WSTRING, structs, and function blocks are deferred.

**Prerequisite reading**: ADR-0023 (array bounds safety), ADR-0017 (unified data region), ADR-0005 (safety-first principle).

### Key Design Decisions

1. **Element size**: 8 bytes (slot-sized) per element in the data region. Matches FB field storage. Future optimization to pack elements deferred.

2. **Always 0-based descriptors**: The compiler normalizes all subscripts to 0-based flat indices before emitting `LOAD_ARRAY`/`STORE_ARRAY`. Descriptors always store `lower_bound=0, upper_bound=total_elements-1`. Original IEC bounds live in the debug section.

3. **Bounds safety**: Three layers — compile-time per-dimension for constants, runtime flat check in VM, load-time verifier check. See ADR-0023.

4. **Safety-critical note**: The flat runtime bounds check guarantees memory safety but does not catch all logically invalid multi-dimensional indices (e.g., `matrix[0, 5]` on `ARRAY[1..3, 1..4]` may access a valid but semantically wrong element — see ADR-0023 case 3). For safety-critical applications, per-dimension runtime checks should be added as a follow-up. This does not block the initial implementation since memory safety is guaranteed.

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
    pub total_elements: u16, // number of elements (upper_bound + 1, max 32768)
}
```

On disk, each descriptor is 8 bytes per the container format spec (`bytecode-container-format.md:157-167`):
```
[element_type: u8] [reserved: u8 = 0] [lower_bound: i16 LE = 0] [upper_bound: i16 LE = total_elements - 1] [element_extra: u16 LE = 0]
```

The on-disk format retains `lower_bound` (always 0) and `upper_bound` (always `total_elements - 1`) per the spec. The in-memory `ArrayDescriptor` struct stores only `total_elements` for convenience; serialization writes `lower_bound=0, upper_bound=total_elements-1`, and deserialization computes `total_elements = upper_bound + 1` (after verifying `lower_bound == 0`).

`element_extra` is reserved for future use (e.g., for arrays of strings, it would hold the max string length; for arrays of structs, it would hold a type descriptor index).

**Limit**: `upper_bound` is `i16`, so max value is 32767, giving a maximum of 32768 elements per array. This is sufficient for typical PLC programs but should be documented as a known limit.

### 3b: Extend `TypeSection`

Add `pub array_descriptors: Vec<ArrayDescriptor>` to `TypeSection`. Update serialization:
- After writing FB types, write array descriptor count (`u16 LE`) followed by each descriptor (8 bytes each)
- Update deserialization symmetrically

This follows the type section serialization order defined in the container format spec (line 527): `num_arrays (u16, LE)` appears after FB type descriptors.

**Do NOT add a header field for array descriptor count.** The count lives in the type section body, not in the header. The header's `num_fb_types` field is the only type-section count in the header. The array count follows the same pattern as function signatures — stored inline in the type section.

### 3c: Link variables to descriptors via VarEntry

Per the container format spec (`bytecode-container-format.md:151-153`), each `VarEntry` has:
- `flags: u8` — bit 0 is `is_array`
- `extra: u16` — for arrays, this holds the array descriptor index

When the variable table (VarEntry section) is implemented in the type section, array variables must have `flags` bit 0 set and `extra` pointing to their descriptor index. This is how the VM and verifier link a `LOAD_ARRAY`/`STORE_ARRAY` variable index to its bounds.

**Current state**: The VarEntry section is not yet implemented in `type_section.rs`. For this initial implementation, the VM builds the var_index → descriptor mapping from the array descriptors at load time using a simpler approach: descriptors are stored in order, and the codegen passes the descriptor index through the `ContainerBuilder`. The builder stores a `Vec<(u16, ArrayDescriptor)>` mapping var_index to descriptor. At load time, the VM iterates the descriptors and builds a `HashMap<u16, ArrayDescriptor>`.

**Future**: When the VarEntry section is implemented, migrate to using `VarEntry.flags` and `VarEntry.extra` per the spec. This is a container-internal change that doesn't affect the codegen or VM opcode semantics.

### 3d: Update `ContainerBuilder`

**File**: `compiler/container/src/builder.rs`

Add method:
```
pub fn add_array_descriptor(mut self, var_index: u16, element_type: u8, total_elements: u16) -> Self
```

The builder stores descriptors in a `Vec<(u16, ArrayDescriptor)>`. During `build()`, it serializes them into the type section after FB type descriptors. The descriptor index is implicitly the position in the Vec.

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
    data_offset: u32,         // byte offset in data region where elements start
    element_type_byte: u8,    // type byte for LOAD_ARRAY/STORE_ARRAY
    element_var_type_info: VarTypeInfo,  // element's VarTypeInfo for truncation
    total_elements: u32,      // total flattened element count
    dimensions: Vec<DimensionInfo>,  // per-dimension bounds and strides
}
```

### 4b: Add to `CompileContext`

```rust
struct CompileContext {
    // ... existing fields ...
    array_vars: HashMap<Id, ArrayVarInfo>,
}
```

Initialize `array_vars: HashMap::new()` in `CompileContext::new()`.

### 4c: Widen `data_region_offset` to `u32`

The existing `data_region_offset: u16` in `CompileContext` must be widened to `u32`. Arrays can easily exceed 64KB: `ARRAY[1..1000] OF DINT` = 8000 bytes, and a few such arrays would overflow `u16`. The header's `data_region_bytes` field is already `u32`, so the container supports this. The codegen is the only place that needs widening.

Change `data_region_offset: u16` to `data_region_offset: u32` in `CompileContext`. Update all existing code that uses it (STRING allocation, FB allocation) to use `u32` arithmetic. The `checked_add` calls remain but now operate on `u32`.

**Impact on existing code**: `StringVarInfo.data_offset` and `FbInstanceInfo.data_offset` should also be widened to `u32`. The emitter's `LOAD_CONST_I32` for storing data offsets into variable slots already takes `i32`, which holds values up to 2^31 — sufficient.

### 4d: Thread `TypeEnvironment` through

The `compile()` function receives `_types: &TypeEnvironment` but ignores it. Change to `types: &TypeEnvironment` and pass it to `assign_variables()` and other functions that need array type resolution. This is needed to resolve user-defined array type names (e.g., `TYPE MY_ARRAY : ARRAY[1..10] OF INT; END_TYPE`).

---

## Step 5: Handle `InitialValueAssignmentKind::Array` in `assign_variables()`

**File**: `compiler/codegen/src/compile.rs`, function `assign_variables()`

Currently, the `match` on `decl.initializer` handles `Simple`, `String`, `FunctionBlock`, etc. The `Array` case falls through to the catch-all `_ => (...)` with no special handling. Add an explicit case.

### 5a: Extract array subranges into a helper

Since both `SpecificationKind::Inline` and `SpecificationKind::Named` need the same processing once the subranges are resolved, extract the core logic into a helper:

```rust
/// Processes resolved array subranges and registers the array in the compile context.
fn register_array_variable(
    ctx: &mut CompileContext,
    id: &Id,
    var_index: u16,
    subranges: &ArraySubranges,
    initial_values: &[ArrayInitialElementKind],
    span: &SourceSpan,
) -> Result<(u8, String), Diagnostic> {
    // 1. Resolve element type
    let element_type_name = &subranges.type_name;
    let element_vti = resolve_type_name(&Id::from(element_type_name.to_string()))
        .ok_or_else(|| Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(span.clone(), "Unsupported array element type"),
        ))?;

    // 2. Extract dimension info from subranges
    let mut dimensions: Vec<DimensionInfo> = Vec::new();
    let mut total_elements: u32 = 1;
    for range in &subranges.ranges {
        let lower = signed_integer_to_i32(&range.start)?;
        let upper = signed_integer_to_i32(&range.end)?;
        let size = (upper as i64 - lower as i64 + 1) as u32;
        dimensions.push(DimensionInfo { lower_bound: lower, size, stride: 0 });
        total_elements = total_elements.checked_mul(size).ok_or_else(|| {
            Diagnostic::problem(
                Problem::NotImplemented,
                Label::span(span.clone(), "Array too large"),
            )
        })?;
    }

    // 3. Validate total_elements fits in the container descriptor (i16 upper_bound)
    if total_elements > 32768 {
        return Err(Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(span.clone(), "Array exceeds maximum 32768 elements"),
        ));
    }

    // 4. Compute strides (reverse pass)
    // stride[last] = 1, stride[k] = stride[k+1] * size[k+1]
    let n = dimensions.len();
    if n > 0 {
        dimensions[n - 1].stride = 1;
        for k in (0..n-1).rev() {
            dimensions[k].stride = dimensions[k + 1].stride * dimensions[k + 1].size;
        }
    }

    // 5. Allocate data region space (u32 arithmetic — no overflow for valid arrays)
    let data_offset = ctx.data_region_offset;
    let total_bytes = total_elements * 8;  // 8 bytes per element, u32 arithmetic
    ctx.data_region_offset = ctx.data_region_offset
        .checked_add(total_bytes)
        .ok_or_else(|| Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(span.clone(), "Data region overflow"),
        ))?;

    // 6. Determine element type byte
    let element_type_byte = var_type_info_to_type_byte(&element_vti);

    // 7. Store in context
    ctx.array_vars.insert(id.clone(), ArrayVarInfo {
        var_index,
        data_offset,
        element_type_byte,
        element_var_type_info: element_vti,
        total_elements,
        dimensions,
    });

    let type_tag = iec_type_tag::OTHER;
    let type_name_str = format!("ARRAY OF {}", element_type_name.to_string().to_uppercase());
    Ok((type_tag, type_name_str))
}
```

### 5b: Handle inline and named array specifications

```
InitialValueAssignmentKind::Array(array_init) => {
    match &array_init.spec {
        SpecificationKind::Inline(array_subranges) => {
            let (tag, name) = register_array_variable(
                ctx, &id, index, array_subranges,
                &array_init.initial_values,
                &decl.identifier.span(),
            )?;
            (tag, name)
        }
        SpecificationKind::Named(type_name) => {
            // Named array type: look up in TypeEnvironment to get the
            // ArrayDeclaration, then extract its subranges.
            let resolved = types.resolve_array_type(type_name)
                .ok_or_else(|| Diagnostic::problem(
                    Problem::NotImplemented,
                    Label::span(type_name.span(), "Unknown array type"),
                ))?;
            let (tag, name) = register_array_variable(
                ctx, &id, index, &resolved.spec_as_subranges(),
                &array_init.initial_values,
                &type_name.span(),
            )?;
            (tag, name)
        }
    }
}
```

**Note**: The `TypeEnvironment` may need a new method `resolve_array_type()` that looks up a type name and returns the `ArraySubranges` if the type is an array type. Check what methods `TypeEnvironment` already provides and add one if needed. The key is to extract the subranges (dimensions and element type) from the type definition.

### 5c: Helper function `var_type_info_to_type_byte`

Maps from VarTypeInfo to the element type byte:

| VarTypeInfo | Type byte |
|------------|-----------|
| W32 + Signed (SINT, INT, DINT, BOOL, TIME) | 0 (I32) |
| W32 + Unsigned (USINT, UINT, UDINT, BYTE, WORD, DWORD) | 1 (U32) |
| W64 + Signed (LINT, LTIME) | 2 (I64) |
| W64 + Unsigned (ULINT, LWORD) | 3 (U64) |
| F32 (REAL) | 4 (F32) |
| F64 (LREAL) | 5 (F64) |

### 5d: Use existing `signed_integer_to_i32` helper

The design uses the existing `signed_integer_to_i32()` function (line 1407 in `compile.rs`) which already handles the `u128` value field with `is_neg` flag and emits proper `ConstantOverflow` diagnostics on range errors. No new helper is needed.

---

## Step 6: Array Initialization in `emit_initial_values()`

**File**: `compiler/codegen/src/compile.rs`

In `emit_initial_values()`, the `Array` case currently falls through to the catch-all `_ => {}` which silently does nothing. Replace it with explicit handling.

For each array variable:

1. **Store data_offset in slot**: Like FB instances, emit `LOAD_CONST_I32 data_offset`, `STORE_VAR_I32 var_index`. This makes the variable's slot hold the data region pointer.

2. **Initialize elements**: For arrays with no explicit initial values (`initial_values` is empty), the data region is zero-initialized by the VM — nothing to emit. For arrays with initial values, emit stores for each element.

### 6a: Flatten initial values

Add a helper function `flatten_array_initial_values` that recursively walks the `ArrayInitialElementKind` tree and produces a flat `Vec<ConstantKind>`:

```rust
fn flatten_array_initial_values(
    elements: &[ArrayInitialElementKind],
) -> Result<Vec<ConstantKind>, Diagnostic> {
    let mut result = Vec::new();
    for elem in elements {
        match elem {
            ArrayInitialElementKind::Constant(value) => {
                result.push(value.clone());
            }
            ArrayInitialElementKind::EnumValue(_) => {
                // Deferred: enum arrays not in scope
                return Err(Diagnostic::todo(file!(), line!()));
            }
            ArrayInitialElementKind::Repeated(repeated) => {
                let count = repeated.size.value as usize;
                match repeated.init.as_ref() {
                    Some(inner) => {
                        // Recursively flatten the inner element
                        let inner_values = flatten_array_initial_values(&[inner.clone()])?;
                        for _ in 0..count {
                            result.extend_from_slice(&inner_values);
                        }
                    }
                    None => {
                        // Repeated with no value means zero-fill; use integer 0
                        for _ in 0..count {
                            result.push(ConstantKind::integer_zero());
                        }
                    }
                }
            }
        }
    }
    Ok(result)
}
```

### 6b: Emit initialization sequence

For each flattened initial value at flat index `i`:

```
LOAD_CONST <initial_value>   // push value (width matches element type)
LOAD_CONST_I32 <i>           // push 0-based flat index
STORE_ARRAY var_index, type_byte
```

**Note**: Initialization uses `STORE_ARRAY` with constant indices, which means the VM will bounds-check during init. This is correct and safe.

---

## Step 7: Compile Array Access (the `todo!()` replacement)

**File**: `compiler/codegen/src/compile.rs`

The `todo!()` is in `resolve_variable()` at line 3079 inside the `SymbolicVariableKind::Array` match arm. Array access cannot go through `resolve_variable()` because it returns a `u16` index — arrays need index computation code emitted, not just a variable index.

Instead, array access must be intercepted **before** `resolve_variable()` is called, in the two places that handle variable references:

1. **Expression context** (`compile_expr`, line 1745): Currently calls `resolve_variable()` then `emit_load_var()`. Must check for array first.
2. **Assignment context** (`compile_statement` / `StmtKind::Assignment`, line 1085): Currently calls `resolve_variable()` then `emit_store_var()`. Must check for array first.

### 7a: Add helper to check if a variable is an array access

```rust
/// If the variable is an array access, returns the ArrayVariable.
/// Returns None for non-array variables.
fn as_array_access(variable: &Variable) -> Option<&ArrayVariable> {
    match variable {
        Variable::Symbolic(SymbolicVariableKind::Array(array)) => Some(array),
        _ => None,
    }
}
```

### 7b: Resolve array variable info

Add a helper function that walks the `ArrayVariable` chain to collect subscripts and resolve the base variable:

```rust
fn resolve_array_access(
    array_var: &ArrayVariable,
    ctx: &CompileContext,
) -> Result<(Vec<&Expr>, &ArrayVarInfo), Diagnostic> {
    // Walk the chain collecting subscripts in dimension order.
    // For nested arrays arr[i][j], the AST is:
    //   ArrayVariable {
    //       subscripted_variable: Array(ArrayVariable {
    //           subscripted_variable: Named(Id("arr")),
    //           subscripts: [i],
    //       }),
    //       subscripts: [j],
    //   }
    // We collect subscripts outermost-first: [i, j].
    let mut all_subscripts: Vec<&Expr> = Vec::new();
    let mut current = array_var;
    loop {
        // Prepend this level's subscripts (they are the outer dimensions)
        // We'll reverse at the end.
        let insertion_point = all_subscripts.len();
        for sub in &current.subscripts {
            all_subscripts.push(sub);
        }
        // Rotate the newly added subscripts to the front
        all_subscripts[..insertion_point + current.subscripts.len()]
            .rotate_right(current.subscripts.len());

        match current.subscripted_variable.as_ref() {
            SymbolicVariableKind::Array(inner) => {
                current = inner;
            }
            SymbolicVariableKind::Named(named) => {
                let info = ctx.array_vars.get(&named.name)
                    .ok_or_else(|| Diagnostic::todo_with_span(
                        named.name.span(), file!(), line!()
                    ))?;
                return Ok((all_subscripts, info));
            }
            other => {
                return Err(Diagnostic::todo_with_span(
                    other.span(), file!(), line!()
                ));
            }
        }
    }
}
```

### 7c: Emit index computation

Add a helper function `emit_flat_index` that computes the 0-based flat index:

```rust
fn emit_flat_index(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    subscripts: &[&Expr],
    dimensions: &[DimensionInfo],
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    if subscripts.len() != dimensions.len() {
        return Err(Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(span.clone(), "Wrong number of array subscripts"),
        ));
    }

    // Check if all subscripts are constant expressions.
    // If so, compute flat index at compile time with per-dimension bounds checking.
    if let Some(flat_index) = try_constant_flat_index(subscripts, dimensions, span)? {
        let const_index = ctx.add_i32_constant(flat_index);
        emitter.emit_load_const_i32(const_index);
        return Ok(());
    }

    // Variable case: emit runtime computation.
    // Emit: (s_0 - l_0) * stride_0 + (s_1 - l_1) * stride_1 + ...
    let op_type = (OpWidth::W32, Signedness::Signed);
    for (k, (subscript, dim)) in subscripts.iter().zip(dimensions.iter()).enumerate() {
        compile_expr(emitter, ctx, subscript, op_type)?;
        if dim.lower_bound != 0 {
            let lb_const = ctx.add_i32_constant(dim.lower_bound);
            emitter.emit_load_const_i32(lb_const);
            emitter.emit_sub_i32();
        }
        if dim.stride != 1 {
            let stride_const = ctx.add_i32_constant(dim.stride as i32);
            emitter.emit_load_const_i32(stride_const);
            emitter.emit_mul_i32();
        }
        if k > 0 {
            emitter.emit_add_i32();  // accumulate into running sum
        }
    }
    Ok(())
}
```

After this sequence, the stack has exactly one I32: the 0-based flat index.

**Constant index helper**:

```rust
fn try_constant_flat_index(
    subscripts: &[&Expr],
    dimensions: &[DimensionInfo],
    span: &SourceSpan,
) -> Result<Option<i32>, Diagnostic> {
    let mut flat_index: i32 = 0;
    for (subscript, dim) in subscripts.iter().zip(dimensions.iter()) {
        let value = match evaluate_constant_i32(subscript) {
            Some(v) => v,
            None => return Ok(None),  // not all constant — use runtime path
        };
        // Per-dimension bounds check at compile time
        let upper = dim.lower_bound + dim.size as i32 - 1;
        if value < dim.lower_bound || value > upper {
            return Err(Diagnostic::problem(
                Problem::ArrayIndexOutOfBounds,  // or appropriate problem code
                Label::span(span.clone(), "Array index out of bounds"),
            ));
        }
        flat_index += (value - dim.lower_bound) * dim.stride as i32;
    }
    Ok(Some(flat_index))
}
```

### 7d: Array read (in expression context)

In `compile_expr()`, change the `ExprKind::Variable` arm (line 1745-1749):

```rust
ExprKind::Variable(variable) => {
    // Check for array access first — arrays need index computation,
    // not a simple variable load.
    if let Some(array_var) = as_array_access(variable) {
        let (subscripts, info) = resolve_array_access(array_var, ctx)?;
        emit_flat_index(emitter, ctx, &subscripts, &info.dimensions, &expr.span())?;
        emitter.emit_load_array(info.var_index, info.element_type_byte);

        // Truncate for sub-32-bit types. LOAD_ARRAY always pushes a full
        // slot (I32 or I64). For SINT (8-bit signed), BOOL (1-bit), BYTE
        // (8-bit unsigned), etc., the value in the data region was stored
        // truncated, so the upper bits are already zero. However, to match
        // the scalar load path and ensure sign-extension for signed sub-32
        // types, apply the same truncation as emit_truncation().
        emit_truncation(emitter, info.element_var_type_info);
        return Ok(());
    }
    let var_index = resolve_variable(ctx, variable)?;
    emit_load_var(emitter, var_index, op_type);
    Ok(())
}
```

### 7e: Array write (in assignment context)

In `compile_statement()`, in the `StmtKind::Assignment` arm (line 1085), add an array check before the existing string and scalar paths:

```rust
StmtKind::Assignment(assignment) => {
    // Check for array target first.
    if let Some(array_var) = as_array_access(&assignment.target) {
        let (subscripts, info) = resolve_array_access(array_var, ctx)?;
        let element_op_type = (info.element_var_type_info.op_width,
                               info.element_var_type_info.signedness);

        // 1. Compile the RHS value (pushes value onto stack).
        compile_expr(emitter, ctx, &assignment.value, element_op_type)?;

        // 2. Truncate for sub-32-bit types before storing.
        emit_truncation(emitter, info.element_var_type_info);

        // 3. Compute the index (pushes index onto stack).
        emit_flat_index(emitter, ctx, &subscripts, &info.dimensions,
                        &assignment.target.span())?;

        // Stack now has: [..., value, index]
        // STORE_ARRAY pops both: index (TOS) then value.
        emitter.emit_store_array(info.var_index, info.element_type_byte);
        return Ok(());
    }

    // ... existing string and scalar assignment code ...
}
```

**Stack order**: `STORE_ARRAY` spec says `[value, I32] → []` — I32 (index) is on top, value is below. The code above pushes value first (via `compile_expr`), then index (via `emit_flat_index`). The VM pops index first (TOS), then value. This matches.

---

## Step 8: VM Implementation

### 8a: New trap type

**File**: `compiler/vm/src/error.rs`

Add to the `Trap` enum:
```
ArrayIndexOutOfBounds { var_index: u16, index: i32, upper_bound: i16 }
```

Implement `Display` to show: `"array index out of bounds: index {index} for array variable {var_index} with bounds [0..{upper_bound}]"`.

**File**: `compiler/vm/resources/problem-codes.csv`

Add: `V4005,ArrayIndexOutOfBounds,Array index out of bounds,true`

### 8b: Array descriptor table in VM

**File**: `compiler/vm/src/vm.rs` (or a new `compiler/vm/src/array.rs` if the file is already large)

At container load time, parse the array descriptor section from the type section and build a lookup:

```rust
struct ArrayDescriptor {
    upper_bound: i16,  // total_elements - 1
    element_type: u8,
}

// In the VM's loaded state:
array_descriptors: HashMap<u16, ArrayDescriptor>  // var_index → descriptor
```

The VM reads descriptors from the type section. Since the VarEntry section is not yet implemented, the VM needs the var_index→descriptor mapping. Two options:

1. **ContainerBuilder embeds var_index**: The builder writes a mapping table (separate from the spec's descriptor format) that the VM reads. This is a temporary approach until VarEntry is implemented.
2. **Codegen passes the mapping out-of-band**: The codegen stores the mapping in the container's constant pool or a custom section.

**Recommended**: Option 1 — extend the type section serialization to write `var_index: u16` before each array descriptor. This is a temporary 2-byte prefix per descriptor that will be removed when VarEntry is implemented. Document this as a temporary deviation.

### 8c: LOAD_ARRAY handler

In the VM's opcode dispatch loop, add after existing handlers. The VM uses `Slot` for all stack operations (not typed push/pop), so follow the existing patterns:

```
LOAD_ARRAY => {
    let var_index = read_u16_le(bytecode, &mut pc);
    let type_byte = bytecode[pc];
    pc += 1;
    let index_slot = stack.pop()?;
    let index = index_slot.as_i32();

    // Look up array descriptor
    let desc = array_descriptors.get(&var_index)
        .ok_or(Trap::InvalidVariable(var_index))?;

    // Bounds check: 0 <= index <= upper_bound
    if index < 0 || index > desc.upper_bound as i32 {
        return Err(Trap::ArrayIndexOutOfBounds {
            var_index,
            index,
            upper_bound: desc.upper_bound,
        });
    }

    // Read data_offset from the variable's slot
    let data_offset = variables.load(scope.resolve(var_index))?.as_i32() as u32 as usize;

    // Compute byte offset into data region
    let byte_offset = data_offset + (index as usize) * 8;

    // Bounds-check data region access (defense-in-depth)
    if byte_offset + 8 > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(byte_offset as u16));
    }

    // Read 8 bytes and push as Slot
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&data_region[byte_offset..byte_offset + 8]);
    let raw = i64::from_le_bytes(buf);
    stack.push(Slot::from_i64(raw))?;
}
```

**Note on type_byte**: The VM reads the `type_byte` operand (for verifier validation) but pushes the raw 8-byte value as a `Slot` regardless of type. The `Slot` type is a uniform 8-byte value — type interpretation happens at the consumer (the next opcode). This matches how FB_LOAD_PARAM works. The `type_byte` is validated by the verifier, not used by the VM at runtime.

### 8d: STORE_ARRAY handler

Symmetric to LOAD_ARRAY. Stack has `[..., value, index]`:

```
STORE_ARRAY => {
    let var_index = read_u16_le(bytecode, &mut pc);
    let type_byte = bytecode[pc];
    pc += 1;
    let index_slot = stack.pop()?;  // TOS = index
    let value_slot = stack.pop()?;  // second = value

    // Same bounds check as LOAD_ARRAY
    let index = index_slot.as_i32();
    let desc = array_descriptors.get(&var_index)
        .ok_or(Trap::InvalidVariable(var_index))?;
    if index < 0 || index > desc.upper_bound as i32 {
        return Err(Trap::ArrayIndexOutOfBounds {
            var_index,
            index,
            upper_bound: desc.upper_bound,
        });
    }

    let data_offset = variables.load(scope.resolve(var_index))?.as_i32() as u32 as usize;
    let byte_offset = data_offset + (index as usize) * 8;
    if byte_offset + 8 > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(byte_offset as u16));
    }

    // Write value to data region as 8 bytes
    data_region[byte_offset..byte_offset + 8]
        .copy_from_slice(&value_slot.as_i64().to_le_bytes());
}
```

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
| `array_initialization_repeated` | `ARRAY[1..6] OF INT := [3(10), 3(20)]` | Emits 6 STORE_ARRAY calls |
| `array_sint_truncation` | `VAR arr: ARRAY[1..3] OF SINT; END_VAR x := arr[1];` | Emits LOAD_ARRAY + TRUNC_I8 |

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

1. `LOAD_ARRAY`/`STORE_ARRAY` operand `var_index` must have a corresponding array descriptor (via VarEntry `is_array` flag when implemented, or via descriptor lookup for now)
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
| `compiler/container/src/builder.rs` | Add `add_array_descriptor()` method |
| `compiler/codegen/src/emit.rs` | Add `emit_load_array()`, `emit_store_array()` |
| `compiler/codegen/src/compile.rs` | Widen `data_region_offset` to `u32`, add `ArrayVarInfo`, `DimensionInfo`, changes to `assign_variables()`, `emit_initial_values()`, `compile_expr()`, `compile_statement()`, thread `TypeEnvironment` |
| `compiler/vm/src/error.rs` | Add `ArrayIndexOutOfBounds` trap variant |
| `compiler/vm/resources/problem-codes.csv` | Add `V4005` |
| `compiler/vm/src/vm.rs` | Add LOAD_ARRAY and STORE_ARRAY handlers, array descriptor loading |
| `specs/design/bytecode-verifier-rules.md` | Add array verification rules |

## Risks and Open Questions

1. **`upper_bound` is `i16`**: Maximum 32768 elements per array. `ARRAY[1..100, 1..100]` is 10000 elements (OK). `ARRAY[1..200, 1..200]` is 40000 elements (exceeds i16 limit, will produce a compile-time error). If large arrays are needed, the container format spec's descriptor field must be widened.

2. **Named array types**: The `SpecificationKind::Named` path requires `TypeEnvironment` to resolve the type name to its `ArraySubranges`. Check what methods `TypeEnvironment` provides — it may need a new accessor. If it can't be resolved at compile time, emit a diagnostic rather than silently failing.

3. **Mixed constant/variable subscripts**: `matrix[2, j]` has one constant and one variable subscript. The current design treats all subscripts as variable when any one is variable (simpler). An optimization to evaluate constant subscripts at compile time and emit `LOAD_CONST (2-1)` instead of `LOAD_VAR, SUB` can be added later but is not required for correctness.

4. **Temporary var_index-in-descriptor**: The var_index prefix on array descriptors in the type section is a temporary measure until the VarEntry section is implemented. Track this as tech debt and remove it when VarEntry is added.
