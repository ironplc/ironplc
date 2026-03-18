# Structure Code Generation — Implementation Plan

## Overview

Add code generation and VM support for user-defined structures (STRUCT). This document has enough detail for an implementer to work from without additional design decisions.

**Scope**: Structures containing primitive types (BOOL, INT, DINT, LINT, REAL, LREAL, USINT, UINT, UDINT, ULINT, BYTE, WORD, DWORD, LWORD, TIME, LTIME, DATE, LDATE, TOD, LTOD, DT, LDT), enumerations, other structures (nesting), and arrays of these types.

**Out of scope** (deferred):
- Structures containing STRING/WSTRING fields — TODO: requires string sub-allocation within struct data region
- Structures containing function block instances — TODO: requires FB lifecycle management
- Whole-structure assignment (`s1 := s2`) — TODO: requires field-by-field copy emission
- Structure literals in expressions — TODO: requires temp struct allocation
- Packed byte-level layout — TODO: tracked in ADR-0026 migration path

**Prerequisite reading**: ADR-0026 (structure memory layout), ADR-0027 (compile-time field offset resolution), design doc `structure-codegen-memory-layout.md`, array codegen plan `array-codegen-plan.md`.

### Key Design Decisions (Summary)

1. **8-byte slot per field** in the data region (ADR-0026). Matches array/FB element storage. Future migration to packed layout documented.
2. **Compile-time offset resolution** (ADR-0027). No new opcodes. Field access resolves to data region byte offsets at compile time.
3. **Reuse LOAD_ARRAY/STORE_ARRAY** for data region access. Register an array descriptor per structure variable treating it as a flat array of slots.
4. **Flat slot arrays** for arrays of structures. An `ARRAY[1..N] OF S` with S having K slots registers a descriptor with `total_elements = N * K`.

---

## PR Dependency Graph

```
PR 1: TypeEnvironment helper + StructVarInfo
        │
        v
PR 2: Structure variable allocation in assign_variables
        │
        v
PR 3: Structure field initialization
        │
        v
PR 4: Structure field read (load)
        │
        v
PR 5: Structure field write (store)
        │
        v
PR 6: Nested structure support
        │
        v
PR 7: Structures containing arrays
        │
        v
PR 8: Arrays of structures
```

PRs are sequential because each builds on the infrastructure from the previous one. Each PR is independently testable and shippable.

---

## PR 1: TypeEnvironment Helper + StructVarInfo

**Goal**: Add infrastructure for resolving structure types and tracking structure variable metadata in the codegen.

### 1a: Add `resolve_struct_type` to TypeEnvironment

**File**: `compiler/analyzer/src/type_environment.rs`

Add a method to resolve a type name to its `IntermediateType::Structure`:

```rust
/// Returns the intermediate type for a named structure type.
///
/// Returns `Some` with the `IntermediateType::Structure` if the type is found
/// and is a structure type, or `None` if the type is not found or is not a structure.
pub fn resolve_struct_type(&self, type_name: &TypeName) -> Option<&IntermediateType> {
    let attrs = self.get(type_name)?;
    match &attrs.representation {
        it @ IntermediateType::Structure { .. } => Some(it),
        _ => None,
    }
}
```

This mirrors the existing `resolve_array_type` pattern.

### 1b: Add slot-count computation to IntermediateType

**File**: `compiler/analyzer/src/intermediate_type.rs`

Add a method that computes the number of 8-byte slots a type occupies:

```rust
impl IntermediateType {
    /// Returns the number of 8-byte slots this type occupies in the data region.
    ///
    /// Returns `Some(n)` for types with known slot counts, `None` for types
    /// that cannot yet be stored in the data region (STRING, WSTRING, FunctionBlock).
    pub fn slot_count(&self) -> Option<u32> {
        match self {
            // Primitives: 1 slot each
            IntermediateType::Bool
            | IntermediateType::Int { .. }
            | IntermediateType::UInt { .. }
            | IntermediateType::Real { .. }
            | IntermediateType::Bytes { .. }
            | IntermediateType::Time { .. }
            | IntermediateType::Date { .. }
            | IntermediateType::TimeOfDay { .. }
            | IntermediateType::DateAndTime { .. }
            | IntermediateType::Enumeration { .. }
            | IntermediateType::Subrange { .. } => Some(1),

            // Structures: sum of field slot counts
            IntermediateType::Structure { fields } => {
                let mut total = 0u32;
                for field in fields {
                    total = total.checked_add(field.field_type.slot_count()?)?;
                }
                Some(total)
            }

            // Arrays: total_elements * element_slots
            IntermediateType::Array { element_type, dimensions } => {
                let elem_slots = element_type.slot_count()?;
                let total_elements = dimensions.iter().try_fold(1u32, |acc, dim| {
                    let size = (dim.upper - dim.lower + 1) as u32;
                    acc.checked_mul(size)
                })?;
                total_elements.checked_mul(elem_slots)
            }

            // Not yet supported in data region
            IntermediateType::String { .. }
            | IntermediateType::FunctionBlock { .. }
            | IntermediateType::Function { .. } => None,
        }
    }
}
```

### 1c: Add StructVarInfo and StructFieldInfo to codegen

**File**: `compiler/codegen/src/compile.rs`

Add new metadata types:

```rust
/// Metadata for a structure variable.
struct StructVarInfo {
    /// Variable table index holding the data region offset.
    var_index: u16,
    /// Data region byte offset where this structure's fields start.
    data_offset: u32,
    /// Total number of 8-byte slots this structure occupies.
    total_slots: u32,
    /// Array descriptor index for this structure (treats struct as flat slot array).
    desc_index: u16,
    /// Maps field name (lowercase) to field metadata.
    field_map: HashMap<String, StructFieldInfo>,
}

/// Metadata for a single structure field.
struct StructFieldInfo {
    /// Slot offset relative to the containing structure's base.
    slot_offset: u32,
    /// The field's intermediate type (for nested resolution).
    field_type: IntermediateType,
    /// Op type for leaf (primitive/enum) fields.
    op_type: Option<OpType>,
}
```

Add `struct_vars: HashMap<Id, StructVarInfo>` to `CompileContext`.

### 1d: Add helper to build StructFieldInfo from IntermediateType::Structure

```rust
/// Builds a field map from a structure's intermediate type.
///
/// Computes slot offsets for each field, flattening nested structures
/// are NOT flattened here — each level is a separate field_map.
fn build_struct_field_map(fields: &[IntermediateStructField]) -> HashMap<String, StructFieldInfo> {
    let mut map = HashMap::new();
    let mut slot_offset = 0u32;
    for field in fields {
        let field_slots = field.field_type.slot_count().unwrap_or(0);
        let op_type = resolve_field_op_type(&field.field_type);
        map.insert(
            field.name.to_string().to_lowercase(),
            StructFieldInfo {
                slot_offset,
                field_type: field.field_type.clone(),
                op_type,
            },
        );
        slot_offset += field_slots;
    }
    map
}
```

Where `resolve_field_op_type` returns `Some(OpType)` for primitive/enum fields and `None` for structure/array fields (which are accessed via further resolution).

### Tests

- `slot_count_when_primitive_then_returns_1`
- `slot_count_when_structure_with_two_fields_then_returns_2`
- `slot_count_when_nested_structure_then_returns_sum`
- `slot_count_when_array_of_primitives_then_returns_total_elements`
- `slot_count_when_array_of_structures_then_returns_elements_times_struct_slots`
- `slot_count_when_string_field_then_returns_none`
- `resolve_struct_type_when_structure_then_returns_type`
- `resolve_struct_type_when_not_structure_then_returns_none`
- `build_struct_field_map_when_two_fields_then_sequential_offsets`
- `build_struct_field_map_when_nested_struct_then_inner_occupies_multiple_slots`

---

## PR 2: Structure Variable Allocation

**Goal**: Handle `InitialValueAssignmentKind::Structure` in `assign_variables`, allocating data region space and registering the structure metadata.

### 2a: Handle Structure in assign_variables

**File**: `compiler/codegen/src/compile.rs`, in `assign_variables()`

Add a match arm for `InitialValueAssignmentKind::Structure(struct_init)`:

```rust
InitialValueAssignmentKind::Structure(struct_init) => {
    // Resolve the structure type from the type environment
    let struct_type = types.resolve_struct_type(&struct_init.type_name)
        .ok_or_else(|| Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(struct_init.type_name.span(), "Unknown structure type"),
        ))?;

    let IntermediateType::Structure { fields } = struct_type else {
        unreachable!("resolve_struct_type guarantees Structure variant");
    };

    // Compute total slots
    let total_slots = struct_type.slot_count().ok_or_else(|| {
        Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(struct_init.type_name.span(), "Structure contains unsupported field types"),
        )
    })?;

    // Allocate data region space
    let data_offset = ctx.data_region_offset;
    let total_bytes = total_slots * 8;
    ctx.data_region_offset = ctx.data_region_offset
        .checked_add(total_bytes)
        .ok_or_else(|| Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(decl.identifier.span(), "Data region overflow"),
        ))?;

    // Register array descriptor (treating struct as flat slot array)
    let desc_index = builder.add_array_descriptor(total_slots, ...);

    // Build field map
    let field_map = build_struct_field_map(fields);

    // Store metadata
    ctx.struct_vars.insert(
        id.clone(),
        StructVarInfo {
            var_index: index,
            data_offset,
            total_slots,
            desc_index,
            field_map,
        },
    );

    let type_name_str = struct_init.type_name.to_string().to_uppercase();
    (iec_type_tag::OTHER, type_name_str)
}
```

### 2b: Handle `Simple` initializer that resolves to a structure type

When a variable is declared as `myVar : MyStructType;` (no explicit initial values), the parser may produce `InitialValueAssignmentKind::Simple` with the struct's type name. The `LateResolvedType` transform may also produce this. Check if the `Simple` type name resolves to a structure in the type environment, and if so, treat it as a structure allocation.

Alternatively, ensure the late-bound type resolution transform (`xform_resolve_late_bound_expr_kind.rs`) correctly produces `InitialValueAssignmentKind::Structure` for structure-typed variables. Verify this path in testing.

### Tests

- `compile_when_struct_var_then_allocates_data_region`
- `compile_when_struct_var_then_registers_array_descriptor`
- `compile_when_two_struct_vars_then_sequential_data_offsets`
- `compile_when_nested_struct_var_then_allocates_sum_of_slots`

---

## PR 3: Structure Field Initialization

**Goal**: Emit initialization code for structure fields in the init function.

### 3a: Emit default initialization for structure fields

**File**: `compiler/codegen/src/compile.rs`, in the init function emission section

For each structure variable, emit constant-load + STORE_ARRAY for each field:

```rust
InitialValueAssignmentKind::Structure(struct_init) => {
    if let Some(struct_info) = ctx.struct_vars.get(id) {
        let data_offset = struct_info.data_offset;
        let var_index = struct_info.var_index;
        let desc_index = struct_info.desc_index;

        // Store data_offset into the variable slot
        let offset_const = ctx.add_i32_constant(data_offset as i32);
        emitter.emit_load_const_i32(offset_const);
        emitter.emit_store_var_i32(var_index);

        // Initialize each field
        initialize_struct_fields(emitter, ctx, struct_info, &struct_init.elements_init)?;
    }
}
```

### 3b: Field-by-field initialization

```rust
fn initialize_struct_fields(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    struct_info: &StructVarInfo,
    element_inits: &[StructureElementInit],
) -> Result<(), Diagnostic> {
    // Build a map of explicit initializers
    let init_map: HashMap<String, &StructInitialValueAssignmentKind> = element_inits
        .iter()
        .map(|e| (e.name.to_string().to_lowercase(), &e.init))
        .collect();

    // Iterate over fields in declaration order
    for (field_name, field_info) in &struct_info.field_map {
        let slot_idx = field_info.slot_offset;

        if let Some(op_type) = field_info.op_type {
            // Leaf field (primitive/enum)
            if let Some(init_value) = init_map.get(field_name) {
                // Emit explicit initial value
                compile_struct_field_init(emitter, ctx, init_value, op_type)?;
            } else {
                // Emit default value (0)
                emit_default_for_type(emitter, ctx, op_type)?;
            }

            // Truncate if needed
            emit_truncation_for_op_type(emitter, op_type);

            // Store to field
            let idx_const = ctx.add_i32_constant(slot_idx as i32);
            emitter.emit_load_const_i32(idx_const);
            emitter.emit_store_array(struct_info.var_index, struct_info.desc_index);
        } else {
            // Nested structure or array field — recurse
            // (handled in PR 6 and PR 7)
        }
    }
    Ok(())
}
```

### Tests

- `compile_when_struct_init_then_stores_data_offset_in_slot`
- `compile_when_struct_with_explicit_init_then_emits_field_stores`
- `compile_when_struct_with_default_init_then_emits_zero_stores`
- `compile_when_struct_with_partial_init_then_defaults_unspecified_fields`

**End-to-end VM test** (compile + execute):
- `vm_when_struct_field_initialized_then_reads_correct_value`

---

## PR 4: Structure Field Read (Load)

**Goal**: Compile `expr := myStruct.field` (reading a structure field in an expression).

### 4a: Resolve SymbolicVariableKind::Structured for expressions

**File**: `compiler/codegen/src/compile.rs`

The `compile_expr` function handles `Variable::Symbolic(SymbolicVariableKind::Structured(...))`. Currently it returns `todo_with_span`. Replace with:

```rust
SymbolicVariableKind::Structured(structured) => {
    let (var_index, desc_index, slot_offset, op_type) =
        resolve_struct_field_access(ctx, structured)?;

    // Push flat_index (compile-time constant)
    let idx_const = ctx.add_i32_constant(slot_offset as i32);
    emitter.emit_load_const_i32(idx_const);

    // Load from data region via array infrastructure
    emitter.emit_load_array(var_index, desc_index);

    // The value is now on the stack with the field's type
    Ok(())
}
```

### 4b: Implement resolve_struct_field_access

This function walks the `StructuredVariable` AST chain and resolves it to a (var_index, desc_index, slot_offset, op_type) tuple:

```rust
fn resolve_struct_field_access(
    ctx: &CompileContext,
    structured: &StructuredVariable,
) -> Result<(u16, u16, u32, OpType), Diagnostic> {
    // Walk the record chain to find the root variable and accumulate slot offsets
    let (root_name, mut slot_offset, field_type) =
        walk_struct_chain(ctx, &structured.record, &structured.field)?;

    let struct_info = ctx.struct_vars.get(&root_name)
        .ok_or_else(|| ...)?;

    let op_type = resolve_op_type_from_intermediate(&field_type)?;

    Ok((struct_info.var_index, struct_info.desc_index, slot_offset, op_type))
}

fn walk_struct_chain(
    ctx: &CompileContext,
    record: &SymbolicVariableKind,
    field: &Id,
) -> Result<(Id, u32, IntermediateType), Diagnostic> {
    match record {
        SymbolicVariableKind::Named(named) => {
            // Base case: root is a named variable
            let struct_info = ctx.struct_vars.get(&named.name)
                .ok_or_else(|| ...)?;
            let field_info = struct_info.field_map.get(&field.to_string().to_lowercase())
                .ok_or_else(|| ...)?;
            Ok((named.name.clone(), field_info.slot_offset, field_info.field_type.clone()))
        }
        SymbolicVariableKind::Structured(inner) => {
            // Recursive case: nested access
            let (root, parent_offset, parent_type) =
                walk_struct_chain(ctx, &inner.record, &inner.field)?;

            // parent_type must be a Structure
            let IntermediateType::Structure { fields } = &parent_type else {
                return Err(...);
            };

            // Find the field within the parent structure type
            let (field_slot_offset, field_type) =
                find_field_in_type(fields, field)?;

            Ok((root, parent_offset + field_slot_offset, field_type))
        }
        // Array access within struct chain handled in PR 8
        _ => Err(Diagnostic::todo_with_span(...))
    }
}
```

### Tests

- `compile_when_struct_field_read_then_emits_load_array`
- `compile_when_struct_field_read_then_correct_slot_offset`
- `vm_when_struct_field_read_then_returns_initialized_value`
- `vm_when_struct_field_arithmetic_then_correct_result`

---

## PR 5: Structure Field Write (Store)

**Goal**: Compile `myStruct.field := expr` (writing to a structure field).

### 5a: Handle Structured target in assignment compilation

**File**: `compiler/codegen/src/compile.rs`

In the assignment compilation (where `Variable::Symbolic(SymbolicVariableKind::Structured(...))` appears as the target of `:=`), use the same `resolve_struct_field_access` from PR 4:

```rust
SymbolicVariableKind::Structured(structured) => {
    let (var_index, desc_index, slot_offset, op_type) =
        resolve_struct_field_access(ctx, structured)?;

    // Compile the RHS expression
    compile_expr(emitter, ctx, rhs_expr, op_type)?;

    // Truncate if needed
    emit_truncation_for_type(emitter, ...);

    // Push flat_index
    let idx_const = ctx.add_i32_constant(slot_offset as i32);
    emitter.emit_load_const_i32(idx_const);

    // Store to data region
    emitter.emit_store_array(var_index, desc_index);
}
```

### Tests

- `compile_when_struct_field_write_then_emits_store_array`
- `vm_when_struct_field_written_then_value_persists`
- `vm_when_struct_field_written_then_other_fields_unchanged`
- `vm_when_struct_field_write_narrow_type_then_truncated` (e.g., SINT field)

---

## PR 6: Nested Structure Support

**Goal**: Support multi-level field access (`outer.inner.field`) and nested structure initialization.

### 6a: walk_struct_chain already handles nesting (PR 4b)

The recursive `walk_struct_chain` function from PR 4 handles arbitrary nesting depth. This PR adds:
- Initialization of nested structure fields (recursive field-by-field init)
- Test coverage for 2-level and 3-level nesting

### 6b: Nested initialization

Extend `initialize_struct_fields` to handle nested structure fields:

```rust
if field_info.field_type.is_structure() {
    // Recurse into nested structure
    let inner_fields = field_info.field_type.structure_fields().unwrap();
    let nested_inits = find_nested_inits(element_inits, field_name);
    initialize_nested_struct_fields(
        emitter, ctx,
        struct_info.var_index, struct_info.desc_index,
        field_info.slot_offset,
        inner_fields, &nested_inits,
    )?;
}
```

### Tests

- `compile_when_nested_struct_then_allocates_combined_slots`
- `vm_when_nested_struct_field_read_then_correct_value`
- `vm_when_nested_struct_field_write_then_correct_offset`
- `vm_when_deeply_nested_struct_then_correct_offsets` (3 levels)
- `vm_when_nested_struct_init_then_all_fields_initialized`

---

## PR 7: Structures Containing Arrays

**Goal**: Support structures with array fields (`myStruct.arr[i]`).

### 7a: Handle array fields in slot-count computation

The `slot_count` method from PR 1 already handles array fields (returns `total_elements * element_slots`). This PR ensures the field_map correctly accounts for array fields occupying multiple slots.

### 7b: Handle struct-then-array access pattern

When the access chain is `s.arr[i]`, the `StructuredVariable` AST looks like:
```
ArrayVariable {
    subscripted_variable: StructuredVariable { record: Named("s"), field: "arr" },
    subscripts: [i]
}
```

This requires extending the expression compilation to handle `SymbolicVariableKind::Array` where the subscripted variable is a `StructuredVariable`. The resolution:
1. Walk the struct chain to find the array field's base slot offset
2. Compute the array subscript flat index
3. Add the base slot offset to the flat index
4. Use LOAD_ARRAY/STORE_ARRAY with the root struct's descriptor

### 7c: Initialization of array fields within structures

Extend `initialize_struct_fields` to handle array-typed fields:
- For each element of the embedded array, emit a constant + STORE_ARRAY at the correct slot offset.

### Tests

- `compile_when_struct_with_array_field_then_allocates_correct_slots`
- `vm_when_struct_array_field_const_index_then_correct_value`
- `vm_when_struct_array_field_var_index_then_correct_value`
- `vm_when_struct_array_field_bounds_check_then_traps`
- `vm_when_struct_array_field_init_then_all_elements_initialized`

---

## PR 8: Arrays of Structures

**Goal**: Support `arr[i].field` where `arr` is an array of a structure type.

### 8a: Handle array-of-struct allocation

When `assign_variables` encounters an array whose element type resolves to a structure, compute:
- `struct_slots` = structure's `slot_count()`
- `total_slots` = `array_total_elements * struct_slots`
- Register array descriptor with `total_elements = total_slots`

The variable is stored in `array_vars` (it's an array), but with additional metadata indicating the element is a structure.

### 8b: Handle array-then-struct access pattern

When the access chain is `arr[i].field`, the AST looks like:
```
StructuredVariable {
    record: ArrayVariable { subscripted_variable: Named("arr"), subscripts: [i] },
    field: "field"
}
```

Resolution:
1. Identify `arr` as an array of structures
2. Compute `struct_stride = struct_slots`
3. For constant subscript: `flat_slot = (i - lower) * struct_stride + field_slot_offset`
4. For variable subscript: emit `(subscript - lower) * struct_stride + field_slot_offset` as runtime computation
5. Bounds check the total flat_slot against `total_slots`

### 8c: Combined patterns

The walk functions from PRs 4-7 compose to handle `arr[i].inner.values[j]`:
1. Resolve `arr` → array of struct, stride = struct_slots
2. Compute array index `i` → base slot = `(i - lower) * struct_stride`
3. Walk struct chain: `inner` → slot offset within struct, `values` → slot offset within inner
4. Compute array index `j` within the embedded array
5. Sum all offsets

### 8d: Initialization

Array-of-struct initialization iterates over array elements, and for each element, iterates over structure fields:

```
for element_index in 0..array_size {
    for (field_name, field_info) in struct_fields {
        let slot = element_index * struct_stride + field_info.slot_offset;
        // emit constant + STORE_ARRAY at slot
    }
}
```

### Tests

- `compile_when_array_of_struct_then_allocates_elements_times_struct_slots`
- `vm_when_array_of_struct_const_index_then_correct_field`
- `vm_when_array_of_struct_var_index_then_correct_field`
- `vm_when_array_of_struct_bounds_then_traps`
- `vm_when_array_of_struct_field_write_then_correct_offset`
- `vm_when_array_of_nested_struct_then_deep_access_correct`
- `vm_when_struct_with_array_in_array_of_struct_then_all_indices_work`

---

## Testing Strategy

### Unit Tests (codegen crate)

Test that the compiler emits correct bytecode sequences for each access pattern. Use the existing `compile_and_check` test infrastructure from the array codegen tests.

### Integration Tests (VM crate)

Compile a program, load it into the VM, execute, and verify variable values. Test scenarios:

1. **Simple struct**: Declare, initialize, read, write each field
2. **Nested struct (2 levels)**: Access inner fields, verify isolation between instances
3. **Nested struct (3 levels)**: Deep access, verify correct offset accumulation
4. **Struct with array**: Read/write array elements within struct, bounds checking
5. **Array of struct**: Index into array, access fields, bounds checking
6. **Complex composition**: `arr[i].inner.values[j]` with both constant and variable indices
7. **Multiple struct variables**: Verify they don't interfere (separate data region allocations)
8. **All primitive types in struct**: Verify correct OpType handling for each (BOOL, SINT, INT, DINT, LINT, USINT, UINT, UDINT, ULINT, REAL, LREAL, BYTE, WORD, DWORD, LWORD, TIME, LTIME, DATE, LDATE, TOD, LTOD, DT, LDT)
9. **Enumeration fields**: Verify enum values stored/loaded correctly
10. **Narrow type truncation**: SINT/INT/USINT/UINT fields truncated on store, promoted on load

### Test Naming Convention

BDD-style: `function_when_condition_then_result`

Examples:
- `assign_variables_when_struct_type_then_allocates_data_region`
- `compile_expr_when_struct_field_access_then_emits_load_array`
- `vm_execute_when_nested_struct_write_then_correct_offset`

---

## Risk Assessment

### Low Risk
- PR 1 (infrastructure): Pure additions, no existing behavior changes
- PR 2 (allocation): Adds a new match arm in assign_variables, doesn't touch existing arms
- PR 3 (initialization): Extends the init function with new field init logic

### Medium Risk
- PRs 4-5 (field read/write): Touches `compile_expr` and assignment compilation, which handle many existing patterns. Risk of regression in existing expression compilation. Mitigated by existing test suite.
- PR 6 (nesting): Recursive resolution adds complexity. Risk of infinite recursion with pathological types. Mitigated by the analyzer already validating against recursive structure definitions.

### Higher Risk
- PRs 7-8 (struct+array composition): Combines two access patterns (struct field resolution + array index computation) in the same expression. The interaction between compile-time struct offsets and runtime array indices requires careful arithmetic. Risk of off-by-one errors in offset computation. Mitigated by exhaustive VM integration tests with known expected values.

### Key Invariant to Maintain

**All data region accesses must be within bounds.** The defense-in-depth strategy:
1. Compiler computes correct offsets (validated by unit tests)
2. Array descriptor bounds check catches runtime violations (VM enforcement)
3. Bytecode verifier checks descriptor validity (load-time enforcement)

---

## TODO Items for Future Work

These should be tracked as issues or inline TODOs in the codebase:

- [ ] **TODO(struct-string)**: Support STRING/WSTRING fields in structures — requires sub-allocating string `[max_length][cur_length][data]` within the struct's data region block
- [ ] **TODO(struct-fb)**: Support function block instance fields in structures — requires FB lifecycle management for embedded instances
- [ ] **TODO(struct-assign)**: Support whole-structure assignment (`s1 := s2`) — emit field-by-field copy or bulk memcpy
- [ ] **TODO(struct-literal)**: Support structure literals in expressions — requires temp allocation
- [ ] **TODO(struct-packed)**: Migrate to packed byte-level layout (ADR-0026 migration path)
- [ ] **TODO(struct-debug)**: Emit debug metadata mapping data region offsets to structure field paths
- [ ] **TODO(struct-direct-load)**: Add direct data-region load/store opcodes to eliminate array descriptor overhead for constant-offset access
- [ ] **TODO(struct-verifier)**: Extend bytecode verifier with structure-aware validation rules
