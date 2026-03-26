# Structure Code Generation — Implementation Plan

## Overview

Add code generation and VM support for user-defined structures (STRUCT). This document has enough detail for an implementer to work from without additional design decisions.

**Scope**: Structures containing primitive types (BOOL, INT, DINT, LINT, REAL, LREAL, USINT, UINT, UDINT, ULINT, BYTE, WORD, DWORD, LWORD, TIME, LTIME, DATE, LDATE, TOD, LTOD, DT, LDT), enumerations, other structures (nesting), and arrays of these types.

**Out of scope** (deferred):
- Structures containing STRING/WSTRING fields — TODO: requires string sub-allocation within struct data region
- Structures containing function block instances — TODO: requires FB lifecycle management
- Whole-structure assignment (`s1 := s2`) — TODO: requires field-by-field copy emission
- Structure literals in expressions — TODO: requires temp struct allocation
- Structure-typed function/FB parameters (VAR_INPUT, VAR_OUTPUT, VAR_IN_OUT) — TODO: requires pass-by-value copy or pass-by-reference semantics
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

Add a method that computes the number of 8-byte slots a type occupies.

The method returns `Result<u32, SlotCountError>` rather than `Option<u32>` so that
callers can produce accurate diagnostic messages (distinguishing "unsupported field
type" from "nesting too deep" from "arithmetic overflow"):

```rust
/// Reason why `slot_count` could not compute a slot count.
#[derive(Debug, Clone, PartialEq)]
pub enum SlotCountError {
    /// The type contains a field type not yet supported in the data region
    /// (STRING, WSTRING, or FunctionBlock).
    UnsupportedFieldType,
    /// The type nesting depth exceeds the maximum allowed depth (defense-in-depth
    /// guard against recursive type cycles that the analyzer should have rejected).
    MaxDepthExceeded,
    /// The total slot count overflows u32 (structure or array is too large).
    Overflow,
}

impl IntermediateType {
    /// Returns the number of 8-byte slots this type occupies in the data region.
    ///
    /// Returns `Ok(n)` for types with known slot counts.
    ///
    /// Returns `Err(SlotCountError::UnsupportedFieldType)` for types that cannot
    /// yet be stored in the data region (STRING, WSTRING, FunctionBlock).
    ///
    /// Returns `Err(SlotCountError::MaxDepthExceeded)` if the nesting depth
    /// exceeds 32 levels (defense-in-depth against recursive type cycles that
    /// the analyzer's toposort pass should have rejected).
    ///
    /// Returns `Err(SlotCountError::Overflow)` if the total slot count exceeds u32.
    pub fn slot_count(&self) -> Result<u32, SlotCountError> {
        self.slot_count_inner(0)
    }

    fn slot_count_inner(&self, depth: u32) -> Result<u32, SlotCountError> {
        // Guard against runaway recursion (defense-in-depth).
        // The analyzer rejects recursive types via toposort, but if a bug
        // allows one through, this prevents a stack overflow.
        const MAX_NESTING_DEPTH: u32 = 32;
        if depth > MAX_NESTING_DEPTH {
            return Err(SlotCountError::MaxDepthExceeded);
        }

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
            | IntermediateType::Subrange { .. }
            | IntermediateType::Reference { .. } => Ok(1),

            // Structures: sum of field slot counts
            IntermediateType::Structure { fields } => {
                let mut total = 0u32;
                for field in fields {
                    let field_slots = field.field_type.slot_count_inner(depth + 1)?;
                    total = total.checked_add(field_slots)
                        .ok_or(SlotCountError::Overflow)?;
                }
                Ok(total)
            }

            // Arrays: total_elements * element_slots
            IntermediateType::Array { element_type, dimensions } => {
                let elem_slots = element_type.slot_count_inner(depth + 1)?;
                let total_elements = dimensions.iter().try_fold(1u32, |acc, dim| {
                    let size = u32::try_from(dim.upper - dim.lower + 1)
                        .map_err(|_| SlotCountError::Overflow)?;
                    acc.checked_mul(size).ok_or(SlotCountError::Overflow)
                })?;
                total_elements.checked_mul(elem_slots)
                    .ok_or(SlotCountError::Overflow)
            }

            // Not yet supported in data region
            IntermediateType::String { .. }
            | IntermediateType::FunctionBlock { .. }
            | IntermediateType::Function { .. } => Err(SlotCountError::UnsupportedFieldType),
        }
    }
}
```

### 1c: Add StructVarInfo and StructFieldInfo to codegen

**File**: `compiler/codegen/src/compile_struct.rs` (new module, following the `compile_array.rs` pattern)

Create a new module `compile_struct.rs` and register it in `compiler/codegen/src/lib.rs`:
```rust
mod compile_struct;
```

`compile.rs` is already ~4400 lines (well over the project's 1000-line module limit).
All structure-specific logic — `StructVarInfo`, `StructFieldInfo`, `build_struct_fields`,
`allocate_struct_variable`, `resolve_struct_field_access`, `walk_struct_chain`,
`initialize_struct_fields`, and their helpers — goes in `compile_struct.rs`. Only the
match arm dispatch in `assign_variables` and `compile_expr` stays in `compile.rs`,
calling `pub(crate)` functions from the new module.

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
    /// Fields in declaration order. Preserving order ensures deterministic
    /// bytecode emission (reproducible builds) and predictable initialization
    /// sequences. Use `field_index` for O(1) lookup by name.
    fields: Vec<StructFieldInfo>,
    /// Maps field name (lowercase) to index in `fields` Vec for O(1) lookup.
    field_index: HashMap<String, usize>,
}

/// Metadata for a single structure field.
struct StructFieldInfo {
    /// Field name (lowercase, for matching against access chains).
    name: String,
    /// Slot offset relative to the containing structure's base.
    slot_offset: u32,
    /// The field's intermediate type (for nested resolution).
    field_type: IntermediateType,
    /// Op type for leaf (primitive/enum) fields. `None` for structure/array
    /// fields (which are accessed via further resolution).
    op_type: Option<OpType>,
}
```

Add `struct_vars: HashMap<Id, StructVarInfo>` to `CompileContext`.

**Field lookup pattern**: All field access by name goes through `field_index` to get the Vec index, then indexes into `fields`. All iteration (initialization, debug output) uses `fields` directly, which preserves declaration order.

### 1d: Add helper to build struct field metadata from IntermediateType::Structure

```rust
/// Builds field metadata (ordered Vec + lookup HashMap) from a structure's
/// intermediate type.
///
/// Returns `Err` if any field has an unsupported type (STRING, WSTRING,
/// FunctionBlock). Nested structures are NOT flattened — each level is a
/// separate field list.
fn build_struct_fields(
    fields: &[IntermediateStructField],
    span: &SourceSpan,
) -> Result<(Vec<StructFieldInfo>, HashMap<String, usize>), Diagnostic> {
    let mut field_list = Vec::with_capacity(fields.len());
    let mut field_index = HashMap::with_capacity(fields.len());
    let mut slot_offset = 0u32;
    for field in fields {
        let field_slots = field.field_type.slot_count().map_err(|e| {
            let msg = match e {
                SlotCountError::UnsupportedFieldType => format!(
                    "Structure field '{}' has unsupported type (STRING, WSTRING, or FunctionBlock)",
                    field.name
                ),
                SlotCountError::MaxDepthExceeded => format!(
                    "Structure field '{}' exceeds maximum nesting depth (possible recursive type)",
                    field.name
                ),
                SlotCountError::Overflow => format!(
                    "Structure field '{}' is too large (slot count overflows)",
                    field.name
                ),
            };
            Diagnostic::problem(Problem::NotImplemented, Label::span(span.clone(), msg))
        })?;
        let name = field.name.to_string().to_lowercase();
        let op_type = resolve_field_op_type(&field.field_type);
        field_index.insert(name.clone(), field_list.len());
        field_list.push(StructFieldInfo {
            name,
            slot_offset,
            field_type: field.field_type.clone(),
            op_type,
        });
        slot_offset += field_slots;
    }
    Ok((field_list, field_index))
}
```

**Why `unwrap_or(0)` is wrong**: If `slot_count()` returns an error (unsupported field type like STRING), using `unwrap_or(0)` would assign 0 slots to that field, causing all subsequent fields to overlap with it in memory. This silently produces corrupt layouts. The function must return an error instead.

### 1e: Helper function signatures

**File**: `compiler/codegen/src/compile_struct.rs`

The following helper functions are referenced throughout the plan. All live in
`compile_struct.rs` and are `pub(crate)` where called from `compile.rs` match arms.

```rust
/// Maps an IntermediateType to its OpType for leaf fields.
///
/// Returns `Some((OpWidth, Signedness))` for primitive, enum, and subrange types.
/// Returns `None` for structure, array, and other composite types (which are
/// accessed via further resolution, not loaded/stored directly as single values).
///
/// This mirrors how `resolve_type_name` works for elementary type names, but
/// operates on IntermediateType rather than Id. The mapping follows the same
/// rules as the existing `resolve_type_name` in compile.rs:
/// - Bool → (W32, Unsigned) with 8-bit storage
/// - Int { B8..B32 } → (W32, Signed), Int { B64 } → (W64, Signed)
/// - UInt / Bytes → (W32/W64, Unsigned)
/// - Real { B32 } → (F32, Signed), Real { B64 } → (F64, Signed)
/// - Time/Date/TimeOfDay/DateAndTime → width depends on ByteSized
/// - Enumeration → delegate to underlying_type
/// - Subrange → delegate to base_type
/// - Reference → (W64, Unsigned) (references are u64 indices)
fn resolve_field_op_type(field_type: &IntermediateType) -> Option<OpType>

/// Emits a constant load for the type-appropriate default value of a struct field.
///
/// For subrange types, emits the subrange's lower bound (min_value) as an i32/i64
/// constant, since IEC 61131-3 §2.4.3.1 specifies the default is the "leftmost
/// value" of the subrange. For all other types, emits zero via `emit_zero_const`.
fn emit_default_for_field(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    field_type: &IntermediateType,
    op_type: OpType,
) -> Result<(), Diagnostic>

/// Compiles an explicit initial value for a structure field.
///
/// Handles constant expressions (integer/real/boolean literals and enum values)
/// from StructInitialValueAssignmentKind. Emits the appropriate LOAD_CONST
/// instruction. Returns an error for unsupported initializer kinds.
fn compile_struct_field_init(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    init: &StructInitialValueAssignmentKind,
    op_type: OpType,
) -> Result<(), Diagnostic>

/// Emits truncation instructions for narrow types when storing to a struct field.
///
/// Delegates to the existing `emit_truncation(emitter, type_info)` in compile.rs,
/// but takes an IntermediateType to derive the VarTypeInfo. This is needed because
/// struct fields are identified by IntermediateType, not by variable-table entries.
fn emit_truncation_for_field(emitter: &mut Emitter, field_type: &IntermediateType)

/// Looks up a field by name within an IntermediateType::Structure's field list.
///
/// Returns `(slot_offset, field_type)` for the named field, or an error if the
/// field is not found. Used by `walk_struct_chain` to resolve nested field access.
fn find_field_in_type(
    fields: &[IntermediateStructField],
    field_name: &Id,
) -> Result<(u32, IntermediateType), Diagnostic>

/// Finds nested initializer values for a specific field in a structure initializer.
///
/// Given a list of `StructureElementInit` entries and a field name, returns the
/// sub-initializer list for that field (for nested structure initialization in PR 6).
fn find_nested_inits<'a>(
    element_inits: &'a [StructureElementInit],
    field_name: &str,
) -> Vec<&'a StructureElementInit>

/// Initializes fields of a nested structure within a parent structure's data region.
///
/// Like `initialize_struct_fields` but takes a base slot offset within the parent
/// structure's flat slot array. Each field is stored at
/// `parent_base_offset + field.slot_offset`.
fn initialize_nested_struct_fields(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    var_index: u16,
    desc_index: u16,
    base_slot_offset: u32,
    inner_fields: &[IntermediateStructField],
    nested_inits: &[&StructureElementInit],
) -> Result<(), Diagnostic>
```

### Tests

- `slot_count_when_primitive_then_returns_1`
- `slot_count_when_structure_with_two_fields_then_returns_2`
- `slot_count_when_nested_structure_then_returns_sum`
- `slot_count_when_array_of_primitives_then_returns_total_elements`
- `slot_count_when_array_of_structures_then_returns_elements_times_struct_slots`
- `slot_count_when_reference_field_then_returns_1`
- `slot_count_when_string_field_then_returns_unsupported_field_type`
- `slot_count_when_nesting_exceeds_max_depth_then_returns_max_depth_exceeded`
- `slot_count_when_total_overflows_u32_then_returns_overflow`
- `resolve_struct_type_when_structure_then_returns_type`
- `resolve_struct_type_when_not_structure_then_returns_none`
- `build_struct_fields_when_two_fields_then_sequential_offsets`
- `build_struct_fields_when_nested_struct_then_inner_occupies_multiple_slots`
- `build_struct_fields_when_string_field_then_returns_error`
- `build_struct_fields_when_fb_field_then_returns_error`
- `build_struct_fields_when_iterated_then_declaration_order_preserved`

---

## PR 2: Structure Variable Allocation

**Goal**: Handle `InitialValueAssignmentKind::Structure` in `assign_variables`, allocating data region space and registering the structure metadata.

### 2a: Handle Structure in assign_variables

**File**: `compiler/codegen/src/compile_struct.rs` (allocation function), `compiler/codegen/src/compile.rs` (match arm dispatch only)

Extract a helper function in `compile_struct.rs` that performs structure allocation, so it can be called from both the `Structure` and `LateResolvedType` match arms in `compile.rs`:

```rust
/// Allocates data region space for a structure variable and registers metadata.
///
/// Called from both the `Structure` and `LateResolvedType` match arms in
/// `assign_variables`.
fn allocate_struct_variable(
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    types: &TypeEnvironment,
    type_name: &TypeName,
    id: &Id,
    index: u16,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    let struct_type = types.resolve_struct_type(type_name)
        .ok_or_else(|| Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(span.clone(), "Unknown structure type"),
        ))?;

    let IntermediateType::Structure { fields } = struct_type else {
        unreachable!("resolve_struct_type guarantees Structure variant");
    };

    // Compute total slots
    let total_slots = struct_type.slot_count().map_err(|e| {
        let msg = match e {
            SlotCountError::UnsupportedFieldType => "Structure contains unsupported field types (STRING, WSTRING, or FunctionBlock)",
            SlotCountError::MaxDepthExceeded => "Structure exceeds maximum nesting depth (possible recursive type)",
            SlotCountError::Overflow => "Structure is too large (slot count overflows u32)",
        };
        Diagnostic::problem(Problem::NotImplemented, Label::span(span.clone(), msg))
    })?;

    // Enforce slot limit (matches existing array limit for i32 flat-index safety)
    if total_slots > 32768 {
        return Err(Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(span.clone(), "Structure exceeds maximum 32768 slots"),
        ));
    }

    // Allocate data region space
    let data_offset = ctx.data_region_offset;
    let total_bytes = total_slots * 8;
    ctx.data_region_offset = ctx.data_region_offset
        .checked_add(total_bytes)
        .ok_or_else(|| Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(span.clone(), "Data region overflow"),
        ))?;

    // Guard against i32 truncation (data_offset is stored as i32 in the
    // variable slot, matching the array pattern)
    if ctx.data_region_offset > i32::MAX as u32 {
        return Err(Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(span.clone(), "Data region exceeds 2 GiB limit"),
        ));
    }

    // Register array descriptor (treating struct as flat slot array).
    // Use FieldType::Slot for heterogeneous structure fields. This constant
    // is defined in the container crate's FieldType enum (value 10), ensuring
    // a single source of truth shared by codegen, the bytecode verifier, and
    // debug tools. See section "Element Type Byte for Structure Descriptors"
    // below for rationale.
    let desc_index = builder.add_array_descriptor(FieldType::Slot as u8, total_slots);

    // Build field metadata (returns error for unsupported field types)
    let (fields_vec, field_index) = build_struct_fields(fields, span)?;

    // Store metadata
    ctx.struct_vars.insert(
        id.clone(),
        StructVarInfo {
            var_index: index,
            data_offset,
            total_slots,
            desc_index,
            fields: fields_vec,
            field_index,
        },
    );
    Ok(())
}
```

Add the match arm in `assign_variables`:

```rust
InitialValueAssignmentKind::Structure(struct_init) => {
    allocate_struct_variable(
        ctx, builder, types, &struct_init.type_name,
        id, index, &decl.identifier.span(),
    )?;
    let type_name_str = struct_init.type_name.to_string().to_uppercase();
    (iec_type_tag::OTHER, type_name_str)
}
```

### 2b: Handle `LateResolvedType` that resolves to a structure type

**Investigation result**: When a variable is declared as `myVar : MyStructType;` (no explicit initial values), the parser produces `InitialValueAssignmentKind::LateResolvedType(TypeName("MyStructType"))`. The `xform_resolve_late_bound_expr_kind` transform resolves late-bound *expressions* (e.g., RHS of assignments) but does **not** change the `InitialValueAssignmentKind` variant itself. So when codegen's `assign_variables` runs, the variant is still `LateResolvedType`.

The existing catch-all arm `_ => (iec_type_tag::OTHER, String::new())` silently skips these variables, meaning they get a variable slot but no data region allocation.

**Fix**: Add a `LateResolvedType` match arm that checks the type environment and dispatches to the appropriate allocation function:

```rust
InitialValueAssignmentKind::LateResolvedType(type_name) => {
    // Check if this late-resolved type is a structure
    if types.resolve_struct_type(type_name).is_some() {
        allocate_struct_variable(
            ctx, builder, types, type_name,
            id, index, &decl.identifier.span(),
        )?;
        let type_name_str = type_name.to_string().to_uppercase();
        (iec_type_tag::OTHER, type_name_str)
    } else {
        // Not a structure — fall through to default handling.
        // Future work may add FB, array-of-struct, etc. dispatch here.
        (iec_type_tag::OTHER, String::new())
    }
}
```

This arm must appear before the catch-all `_` in the match statement.

### Element Type Byte for Structure Descriptors

The existing array descriptor element type encoding uses the `FieldType` enum from
the container crate (`compiler/container/src/type_section.rs`). Values 0-9 are
already assigned (I32, U32, I64, U64, F32, F64, String, WString, FbInstance, Time).
Structures are heterogeneous — different fields have different types — so no single
primitive type byte applies. Add a new variant to `FieldType`:

**File**: `compiler/container/src/type_section.rs`

```rust
pub enum FieldType {
    I32 = 0,
    U32 = 1,
    I64 = 2,
    U64 = 3,
    F32 = 4,
    F64 = 5,
    String = 6,
    WString = 7,
    FbInstance = 8,
    Time = 9,
    /// Heterogeneous structure field slot. Used as the element type in array
    /// descriptors that back structure variables (which are treated as flat
    /// arrays of 8-byte slots). The VM does not check this value at runtime.
    Slot = 10,
}
```

Update `FieldType::from_u8` to handle value 10.

The VM's `LOAD_ARRAY` / `STORE_ARRAY` implementation does not check the element type byte at runtime — it only uses `total_elements` for bounds checking and always reads/writes 8 bytes. The type byte is metadata for the bytecode verifier and debug tools. Defining `Slot` as a shared `FieldType` variant (rather than a local constant) ensures that:
- The bytecode verifier can distinguish structure descriptors from true array descriptors.
- Debug tools can identify which descriptors belong to structure variables.
- Descriptor deduplication (which keys on `(element_type, total_elements)`) does not falsely merge a structure descriptor with an unrelated array of the same size.
- All components reference a single source of truth — no magic numbers scattered across crates.

### Tests

- `compile_when_struct_var_with_init_then_allocates_data_region` (Structure variant)
- `compile_when_struct_var_without_init_then_allocates_data_region` (LateResolvedType variant)
- `compile_when_struct_var_then_registers_descriptor_with_slot_type`
- `compile_when_two_struct_vars_then_sequential_data_offsets`
- `compile_when_nested_struct_var_then_allocates_sum_of_slots`
- `compile_when_struct_exceeds_32768_slots_then_error`
- `compile_when_struct_causes_data_region_overflow_then_error`

---

## PR 3: Structure Field Initialization

**Goal**: Emit initialization code for structure fields in the init function.

### 3a: Emit default initialization for structure fields

**File**: `compiler/codegen/src/compile_struct.rs` (initialization logic), `compiler/codegen/src/compile.rs` (match arm dispatch in init function)

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

**LateResolvedType init arm** (mirrors the allocation arm from PR 2b):

When a variable is declared as `myVar : MyStructType;` (no explicit initial values), the
`InitialValueAssignmentKind` is `LateResolvedType`. PR 2b handles this in `assign_variables`
for allocation. The init function must also handle it — otherwise these variables get allocated
but never initialized, hitting the catch-all `_ => {}`.

```rust
InitialValueAssignmentKind::LateResolvedType(type_name) => {
    // Check if this late-resolved type was allocated as a structure in assign_variables
    if let Some(struct_info) = ctx.struct_vars.get(id) {
        let data_offset = struct_info.data_offset;
        let var_index = struct_info.var_index;

        // Store data_offset into the variable slot
        let offset_const = ctx.add_i32_constant(data_offset as i32);
        emitter.emit_load_const_i32(offset_const);
        emitter.emit_store_var_i32(var_index);

        // Initialize with type-default values (no explicit initializers)
        initialize_struct_fields(emitter, ctx, struct_info, &[])?;
    }
    // else: not a struct — other LateResolvedType kinds (FB, array-of-struct,
    // etc.) are handled by their own init arms or are future work.
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

    // Iterate over fields in declaration order (Vec guarantees deterministic order)
    for field_info in &struct_info.fields {
        let slot_idx = field_info.slot_offset;

        if let Some(op_type) = field_info.op_type {
            // Leaf field (primitive/enum)
            if let Some(init_value) = init_map.get(&field_info.name) {
                // Emit explicit initial value
                compile_struct_field_init(emitter, ctx, init_value, op_type)?;
            } else {
                // Emit type-appropriate default value.
                // For subrange types, the default is the lower bound of the range
                // (IEC 61131-3 §2.4.3.1: "initial value [...] is the leftmost value").
                // For all other types, the default is 0.
                emit_default_for_field(emitter, ctx, &field_info.field_type, op_type)?;
            }

            // Truncate narrow types (e.g., SINT stored in W32 slot)
            emit_truncation_for_field(emitter, &field_info.field_type);

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
- `compile_when_struct_late_resolved_type_then_initializes_with_defaults` (LateResolvedType variant — no explicit init values)

**End-to-end VM test** (compile + execute):
- `vm_when_struct_field_initialized_then_reads_correct_value`
- `vm_when_struct_late_resolved_type_then_fields_have_defaults` (verifies LateResolvedType init path)

---

## PR 4: Structure Field Read (Load)

**Goal**: Compile `expr := myStruct.field` (reading a structure field in an expression).

### 4a: Resolve SymbolicVariableKind::Structured for expressions

**File**: `compiler/codegen/src/compile_struct.rs` (resolution logic), `compiler/codegen/src/compile.rs` (match arm dispatch in `compile_expr`)

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
            let field_name = field.to_string().to_lowercase();
            let &field_idx = struct_info.field_index.get(&field_name)
                .ok_or_else(|| ...)?;
            let field_info = &struct_info.fields[field_idx];
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

**File**: `compiler/codegen/src/compile_struct.rs` (resolution + store logic), `compiler/codegen/src/compile.rs` (match arm dispatch in assignment compilation)

In the assignment compilation (where `Variable::Symbolic(SymbolicVariableKind::Structured(...))` appears as the target of `:=`), use the same `resolve_struct_field_access` from PR 4:

```rust
SymbolicVariableKind::Structured(structured) => {
    let (var_index, desc_index, slot_offset, op_type) =
        resolve_struct_field_access(ctx, structured)?;

    // Compile the RHS expression
    compile_expr(emitter, ctx, rhs_expr, op_type)?;

    // Truncate narrow types (e.g., SINT stored in W32 slot)
    emit_truncation_for_field(emitter, &structured_field_type);

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
// Note: IntermediateType has is_structure() but no structure_fields() accessor.
// Use pattern matching to extract the fields from the Structure variant.
if let IntermediateType::Structure { fields, .. } = &field_info.field_type {
    // Recurse into nested structure
    let nested_inits = find_nested_inits(element_inits, field_name);
    initialize_nested_struct_fields(
        emitter, ctx,
        struct_info.var_index, struct_info.desc_index,
        field_info.slot_offset,
        fields, &nested_inits,
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

**File**: `compiler/codegen/src/compile_struct.rs`

When the access chain is `s.arr[i]`, the `StructuredVariable` AST looks like:
```
ArrayVariable {
    subscripted_variable: StructuredVariable { record: Named("s"), field: "arr" },
    subscripts: [i]
}
```

This requires extending the expression compilation to handle `SymbolicVariableKind::Array` where the subscripted variable is a `StructuredVariable`. The resolution:
1. Walk the struct chain to find the array field's base slot offset and its `IntermediateType::Array`
2. Compute the array subscript flat index (within the embedded array)
3. Add the struct field's base slot offset to the flat index
4. Use LOAD_ARRAY/STORE_ARRAY with the root struct's descriptor

**Bytecode emission for `s.arr[i]` (read, variable subscript)**:

```rust
/// Compiles a read of `struct_var.array_field[subscript]`.
///
/// The entire struct is a flat slot array. The embedded array's elements
/// occupy slots starting at `field_base_offset`. The subscript selects
/// which element within the embedded array.
fn compile_struct_array_field_load(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    struct_info: &StructVarInfo,
    field_base_offset: u32,    // slot offset of array field within struct
    array_total_elements: u32, // number of elements in the embedded array
    array_dimensions: &[ArrayDimension],
    subscripts: &[Expr],
) -> Result<(), Diagnostic> {
    // Compute flat index within the embedded array (0-based).
    // For constant subscript: emit as compile-time constant.
    // For variable subscript: emit runtime arithmetic.
    //
    // IMPORTANT: The VM's LOAD_ARRAY/STORE_ARRAY bounds-checks against the
    // struct's total_slots, NOT the embedded array's element count. An
    // out-of-range array index that still falls within the struct would
    // silently read/write wrong fields. We must emit our own bounds check
    // against the embedded array's actual element count.
    let is_const = is_constant_subscript(&subscripts[0]);

    if is_const {
        // Compile-time: validate bounds statically and fold into one constant
        let array_index = evaluate_constant_subscript(&subscripts[0])?;
        let zero_based = array_index - array_dimensions[0].lower;
        if zero_based < 0 || zero_based as u32 >= array_total_elements {
            return Err(Diagnostic::problem(
                Problem::ArrayIndexOutOfBounds,
                Label::span(subscripts[0].span(), "Index out of bounds for embedded array"),
            ));
        }
        let total_slot = field_base_offset + zero_based as u32;
        let idx_const = ctx.add_i32_constant(total_slot as i32);
        emitter.emit_load_const_i32(idx_const);
    } else {
        // Runtime: emit (subscript - lower), then bounds-check against
        // the embedded array's element count, then add field_base_offset.
        //
        // Bytecode sequence:
        //   LOAD_VAR subscript_var          ; push subscript value
        //   LOAD_CONST lower_bound          ; push array lower bound
        //   SUB_I32                          ; subscript - lower = 0-based index
        //   DUP                             ; duplicate for bounds check
        //   LOAD_CONST array_total_elements ; push embedded array size
        //   BOUNDS_CHECK                    ; trap if 0-based index >= total_elements
        //   LOAD_CONST field_base_offset    ; push struct field offset
        //   ADD_I32                          ; total flat slot index
        //
        // BOUNDS_CHECK (or equivalent: compare + conditional trap) ensures
        // the 0-based index is within [0, array_total_elements). Without this,
        // the VM would only check against the struct's total_slots, allowing
        // an out-of-range array subscript to silently alias another field.
        //
        // If a dedicated BOUNDS_CHECK opcode is not available, the equivalent
        // can be emitted as:
        //   DUP
        //   LOAD_CONST 0
        //   LT_I32              ; index < 0?
        //   TRAP_IF             ; trap on negative index
        //   DUP
        //   LOAD_CONST array_total_elements
        //   GE_I32              ; index >= total_elements?
        //   TRAP_IF             ; trap on overflow
        compile_expr(emitter, ctx, &subscripts[0], (OpWidth::W32, Signedness::Signed))?;
        let lower_const = ctx.add_i32_constant(array_dimensions[0].lower as i32);
        emitter.emit_load_const_i32(lower_const);
        emitter.emit(opcode::SUB_I32);
        // Emit embedded-array bounds check
        emit_array_bounds_check(emitter, ctx, array_total_elements, &subscripts[0])?;
        let offset_const = ctx.add_i32_constant(field_base_offset as i32);
        emitter.emit_load_const_i32(offset_const);
        emitter.emit(opcode::ADD_I32);
    }

    // Load from the struct's flat slot array
    emitter.emit_load_array(struct_info.var_index, struct_info.desc_index);
    Ok(())
}

/// Emits a runtime bounds check for a 0-based index already on the stack.
/// Traps if the index is negative or >= `total_elements`.
/// The index value remains on the stack after the check (consumed copy via DUP).
///
/// This is needed for embedded arrays within structures because the VM's
/// built-in LOAD_ARRAY/STORE_ARRAY bounds check uses the struct's total_slots,
/// not the embedded array's element count.
fn emit_array_bounds_check(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    total_elements: u32,
    span_source: &Expr,  // for error reporting
) -> Result<(), Diagnostic> {
    // Implementation uses the same pattern as standalone array bounds checking.
    // The exact opcode sequence depends on available VM primitives (BOUNDS_CHECK
    // opcode or compare+trap sequence). See compile_array.rs for the existing
    // pattern.
    todo!("Emit bounds check: 0 <= top-of-stack < total_elements")
}
```

The store variant is symmetric (emit RHS value first, then compute the index with bounds check, then `emit_store_array`). Both load and store paths must include the embedded-array bounds check.

Multi-dimensional arrays within structs follow the same flat-index computation
as standalone arrays (see `compile_array.rs`), but add `field_base_offset` to the
resulting flat index.

### 7c: Initialization of array fields within structures

Extend `initialize_struct_fields` to handle array-typed fields:
- For each element of the embedded array, emit a constant + STORE_ARRAY at the correct slot offset.

### Tests

- `compile_when_struct_with_array_field_then_allocates_correct_slots`
- `compile_when_struct_array_field_const_index_out_of_range_then_error` (compile-time bounds check)
- `vm_when_struct_array_field_const_index_then_correct_value`
- `vm_when_struct_array_field_var_index_then_correct_value`
- `vm_when_struct_array_field_var_index_out_of_range_then_traps` (embedded-array bounds check, not struct-level)
- `vm_when_struct_array_field_var_index_in_struct_but_out_of_array_then_traps` (index within struct total_slots but beyond embedded array — must still trap)
- `vm_when_struct_array_field_init_then_all_elements_initialized`

---

## PR 8: Arrays of Structures

**Goal**: Support `arr[i].field` where `arr` is an array of a structure type.

### 8a: Handle array-of-struct allocation

When `assign_variables` encounters an array whose element type resolves to a structure, compute:
- `struct_slots` = structure's `slot_count()`
- `total_slots` = `array_total_elements * struct_slots`
- Register array descriptor with `total_elements = total_slots`

The variable is stored in `array_vars` (it's an array), and additionally in a new
`ctx.array_of_struct_vars: HashMap<Id, ArrayOfStructInfo>` with the structure metadata
needed for field-access compilation (see `ArrayOfStructInfo` definition in PR 8b).

### 8b: Handle array-then-struct access pattern

**File**: `compiler/codegen/src/compile_struct.rs`

**Define `ArrayOfStructInfo`**: This struct captures the metadata needed to compile
`arr[i].field` access patterns. It is stored in a new `HashMap<Id, ArrayOfStructInfo>`
in `CompileContext`, populated during `assign_variables` when an array's element type
resolves to a structure.

```rust
/// Metadata for an array-of-structure variable.
///
/// Stored in `CompileContext::array_of_struct_vars` during allocation (PR 8a).
/// Used during expression compilation to resolve `arr[i].field` patterns.
struct ArrayOfStructInfo {
    /// Variable slot index (same as the array's var_index).
    var_index: u16,
    /// Array descriptor index (descriptor has total_elements = array_size * struct_slots).
    desc_index: u16,
    /// Number of slots per structure element (from slot_count()).
    struct_stride: u32,
    /// Array dimensions (for lower-bound subtraction during subscript computation).
    dimensions: Vec<DimensionInfo>,
    /// Field metadata for the structure element type (reused from struct type resolution).
    /// Indexed by field name for O(1) lookup during field-access compilation.
    field_index: HashMap<String, usize>,
    /// Ordered list of fields (same as StructVarInfo::fields).
    fields: Vec<StructFieldInfo>,
}
```

When the access chain is `arr[i].field`, the AST looks like:
```
StructuredVariable {
    record: ArrayVariable { subscripted_variable: Named("arr"), subscripts: [i] },
    field: "field"
}
```

Resolution:
1. Identify `arr` as an array of structures
2. Compute `struct_stride = struct_slots` (number of slots per struct element)
3. For constant subscript: `flat_slot = (i - lower) * struct_stride + field_slot_offset`
4. For variable subscript: emit runtime arithmetic
5. The VM's bounds check on the descriptor's `total_slots` catches out-of-range access

**Bytecode emission for `arr[i].field` (read, variable subscript)**:

```rust
/// Compiles a read of `array_of_struct[subscript].field`.
///
/// The array-of-struct variable is a flat slot array where each struct
/// element occupies `struct_stride` consecutive slots. The field's position
/// within each element is `field_slot_offset`.
fn compile_array_of_struct_field_load(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    arr_info: &ArrayOfStructInfo,  // descriptor index, var index, stride, dimensions
    field_slot_offset: u32,
    subscripts: &[Expr],
) -> Result<(), Diagnostic> {
    let is_const = is_constant_subscript(&subscripts[0]);

    if is_const {
        // Compile-time: fold everything into one constant
        let array_index = evaluate_constant_subscript(&subscripts[0])?;
        let zero_based = array_index - arr_info.dimensions[0].lower;
        let flat_slot = zero_based as u32 * arr_info.struct_stride + field_slot_offset;
        let idx_const = ctx.add_i32_constant(flat_slot as i32);
        emitter.emit_load_const_i32(idx_const);
    } else {
        // Runtime: emit (subscript - lower) * struct_stride + field_slot_offset
        //
        // Bytecode sequence:
        //   LOAD_VAR subscript_var          ; push subscript
        //   LOAD_CONST lower_bound          ; push lower
        //   SUB_I32                          ; subscript - lower
        //   LOAD_CONST struct_stride        ; push stride
        //   MUL_I32                          ; (subscript - lower) * stride
        //   LOAD_CONST field_slot_offset    ; push field offset
        //   ADD_I32                          ; final flat slot index
        compile_expr(emitter, ctx, &subscripts[0], (OpWidth::W32, Signedness::Signed))?;
        let lower_const = ctx.add_i32_constant(arr_info.dimensions[0].lower as i32);
        emitter.emit_load_const_i32(lower_const);
        emitter.emit(opcode::SUB_I32);
        let stride_const = ctx.add_i32_constant(arr_info.struct_stride as i32);
        emitter.emit_load_const_i32(stride_const);
        emitter.emit(opcode::MUL_I32);
        let offset_const = ctx.add_i32_constant(field_slot_offset as i32);
        emitter.emit_load_const_i32(offset_const);
        emitter.emit(opcode::ADD_I32);
    }

    // Load from the array-of-struct's flat slot array.
    // The VM's bounds check on total_slots catches out-of-range flat indices.
    emitter.emit_load_array(arr_info.var_index, arr_info.desc_index);
    Ok(())
}
```

The store variant is symmetric. Note that `struct_stride` and `field_slot_offset` are
both compile-time constants, so even the "variable subscript" path only has one truly
runtime value (the subscript itself).

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
    // Iterate fields in declaration order (Vec, not HashMap)
    for field_info in &struct_fields {
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
- PR 6 (nesting): Recursive resolution adds complexity. Risk of infinite recursion with pathological types. Mitigated by (a) the analyzer rejecting recursive type cycles via toposort (`xform_toposort_declarations.rs`, `Problem::RecursiveCycle`) and (b) the defense-in-depth depth guard in `slot_count()` (max 32 levels).

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
- [ ] **TODO(struct-params)**: Support structure-typed function/FB parameters (VAR_INPUT, VAR_OUTPUT, VAR_IN_OUT) — requires pass-by-value copy or pass-by-reference semantics
- [ ] **TODO(struct-packed)**: Migrate to packed byte-level layout (ADR-0026 migration path)
- [ ] **TODO(struct-debug)**: Emit debug metadata mapping data region offsets to structure field paths
- [ ] **TODO(struct-direct-load)**: Add direct data-region load/store opcodes to eliminate array descriptor overhead for constant-offset access
- [ ] **TODO(struct-verifier)**: Extend bytecode verifier with structure-aware validation rules
