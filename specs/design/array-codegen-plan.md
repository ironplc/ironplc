# Array Code Generation — Implementation Plan

## Overview

Add code generation and VM support for arrays of primitive types. This document has enough detail for an implementer to work from without additional design decisions.

**Scope**: Arrays of primitive types (INT, REAL, BOOL, SINT, DINT, LINT, USINT, UINT, UDINT, ULINT, BYTE, WORD, DWORD, LWORD, TIME, LTIME). Arrays of STRING, WSTRING, structs, and function blocks are deferred.

**Prerequisite reading**: ADR-0023 (array bounds safety), ADR-0017 (unified data region), ADR-0005 (safety-first principle).

### Key Design Decisions

1. **Element size**: 8 bytes (slot-sized) per element in the data region. Matches FB field storage. Future optimization to pack elements deferred. Note: this is particularly wasteful for BOOL arrays (`ARRAY[1..1000] OF BOOL` = 8000 bytes vs. 1000 bytes packed). Packed element storage is especially important for embedded targets and should be prioritized as a follow-up.

2. **Always 0-based descriptors**: The compiler normalizes all subscripts to 0-based flat indices before emitting `LOAD_ARRAY`/`STORE_ARRAY`. Descriptors always store `lower_bound=0, upper_bound=total_elements-1`. Original IEC bounds live in the debug section.

3. **Bounds safety**: Three layers — compile-time per-dimension for constants, runtime flat check in VM, load-time verifier check. See ADR-0023.

4. **Safety-critical note**: The flat runtime bounds check guarantees memory safety but does not catch all logically invalid multi-dimensional indices (e.g., `matrix[0, 5]` on `ARRAY[1..3, 1..4]` may access a valid but semantically wrong element — see ADR-0023 case 3). For safety-critical applications, per-dimension runtime checks should be added as a follow-up. This does not block the initial implementation since memory safety is guaranteed.

5. **Runtime index arithmetic uses i64 to prevent overflow**: For variable subscripts, the flat index computation `(subscript - lower_bound) * stride` is emitted using i64 arithmetic (`SUB_I64`, `MUL_I64`, `ADD_I64`) instead of i32. Since subscripts are i32 (sign-extended to i64 in the Slot representation), lower bounds are bounded i32 constants, and strides are at most 32768, no intermediate i64 value can overflow. The VM's `LOAD_ARRAY`/`STORE_ARRAY` handlers read the flat index as i64 via `as_i64()` and bounds-check against `total_elements` before narrowing — no truncation risk. Constants (lower bounds and strides) are emitted via `LOAD_CONST_I64`. This costs 6 extra bytes per constant (i64 vs i32) but eliminates all overflow concerns with zero architectural complexity.

6. **`data_offset` stored as `i32` in variable slots**: The slot stores `data_offset` via `LOAD_CONST_I32`, limiting effective addressing to `i32::MAX` (2 GiB). The codegen must assert `data_region_offset <= i32::MAX as u32` during allocation to fail fast instead of silently wrapping. With the 32768-element limit this is practically unreachable, but the assertion makes the invariant explicit.

---

## Pre-work PRs

Four independent, zero-risk refactoring PRs that land before any array feature code. Each is independently reviewable and introduces no new behavior.

### Pre-work PR 0a: Preserve Per-Dimension Bounds in `IntermediateType::Array`

**Goal**: Replace the flat `size: Option<u32>` in `IntermediateType::Array` with per-dimension bounds so the codegen can recover lower/upper bounds for each dimension when resolving named array types (e.g., `TYPE MY_ARRAY : ARRAY[1..3, 1..4] OF INT; END_TYPE`).

This is a standalone, prerequisite change that lands before the main array codegen work.

### P0a: Add `ArrayDimension` struct

**File**: `compiler/analyzer/src/intermediate_type.rs`

```rust
/// Bounds for a single dimension of an array type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrayDimension {
    /// Lower bound (inclusive), e.g., 1 in ARRAY[1..10]
    pub lower: i32,
    /// Upper bound (inclusive), e.g., 10 in ARRAY[1..10]
    pub upper: i32,
}
```

### P0b: Update `IntermediateType::Array`

**File**: `compiler/analyzer/src/intermediate_type.rs`

Change:
```rust
Array {
    element_type: Box<IntermediateType>,
    size: Option<u32>,
}
```

To:
```rust
Array {
    element_type: Box<IntermediateType>,
    dimensions: Vec<ArrayDimension>,
}
```

Add a derived method:
```rust
impl IntermediateType {
    /// Returns the total number of elements across all dimensions,
    /// or None if dimensions is empty.
    pub fn array_total_elements(&self) -> Option<u32> {
        match self {
            IntermediateType::Array { dimensions, .. } if !dimensions.is_empty() => {
                let mut total: u32 = 1;
                for dim in dimensions {
                    // Guard: if lower > upper, this dimension is invalid.
                    // The analyzer validates this, but defense-in-depth.
                    let span = dim.upper as i64 - dim.lower as i64 + 1;
                    if span <= 0 {
                        return None;
                    }
                    let dim_size = span as u32;
                    total = total.checked_mul(dim_size)?;
                }
                Some(total)
            }
            _ => None,
        }
    }
}
```

### P0c: Update construction site

**File**: `compiler/analyzer/src/intermediates/array.rs`

The `try_from` function currently calls `calculate_array_size()` to get a flat `u32` and stores it as `size: Some(total_size)`. Change it to build a `Vec<ArrayDimension>` from the subranges:

```rust
// Replace: let total_size = calculate_array_size(&array_subranges.ranges)?;
// With:
let dimensions: Vec<ArrayDimension> = array_subranges.ranges.iter()
    .map(|range| {
        let lower = signed_integer_to_i32(&range.start)?;
        let upper = signed_integer_to_i32(&range.end)?;
        Ok(ArrayDimension { lower, upper })
    })
    .collect::<Result<Vec<_>, Diagnostic>>()?;

// Then construct:
IntermediateType::Array {
    element_type: Box::new(element_type.representation.clone()),
    dimensions,
}
```

Keep `validate_array_bounds()` as-is — it already validates min <= max per dimension.

The `calculate_array_size()` function can be removed (total size is derived via `array_total_elements()`). Alternatively, keep it as a validation step to check for overflow early.

### P0d: Update all match sites

Each site that destructures `IntermediateType::Array { element_type, size }` must be updated. The changes are mechanical:

| File | Line(s) | Change |
|------|---------|--------|
| `intermediate_type.rs` | 190, 228-231 (`size_in_bytes`) | **Widen return type from `Option<u8>` to `Option<u32>`**. The current `u8` return type truncates any type larger than 255 bytes — arrays routinely exceed this (`ARRAY[1..100] OF INT` = 400 bytes). Change the method signature to `pub fn size_in_bytes(&self) -> Option<u32>`. In the Array arm, replace `elem_size.saturating_mul(array_size as u8)` with `(elem_size as u32).checked_mul(self.array_total_elements()?)`. In the Structure/FunctionBlock arms, the computation already uses `u32` internally — just remove the `> u8::MAX` guard and return `Some(total_size)` directly. For primitive type arms (Bool, Int, UInt, Real, Bytes, Time, Date), change `Some(N)` to `Some(N as u32)` or `Some(size.as_bytes() as u32)`. **Callers to update**: (1) `type_attributes.rs:44` — `size_bytes()` currently does `.map(|s| s as u32)`, simplify to just forward the `Option<u32>` directly. (2) `type_attributes.rs:107` — same change. (3) `rule_bit_access_range.rs:88` — currently does `u128::from(bytes) * 8`, change to `bytes as u128 * 8`. (4) `intermediates/structure.rs:38` — currently does `.unwrap_or(0) as u32`, change to `.unwrap_or(0)`. (5) `intermediates/stdlib_function_block.rs:68` — same as (4). All callers already widen to `u32`; this change eliminates the lossy narrowing. |
| `intermediate_type.rs` | 296 (`alignment_bytes`) | No change needed — already uses `{ element_type, .. }` |
| `intermediate_type.rs` | 335-338 (`has_explicit_size`) | Change `size.is_some()` to `!dimensions.is_empty()` |
| `intermediate_type.rs` | 147 (`is_array`) | No change needed — already uses `{ .. }` |
| `rule_bit_access_range.rs` | 132-135 | Change `size: None` to `dimensions: vec![]` |
| `rule_bit_access_range.rs` | 163 | Change `{ element_type, .. }` — already uses wildcard, no change |
| `type_category.rs` | 34 | No change needed — already uses `{ .. }` |
| `xform_resolve_type_decl_environment.rs` | 73-76 | Update pattern match to use `dimensions` |
| `intermediates/structure.rs` | 733, 768 | Update test assertions to check `dimensions` instead of `size` |
| `plc2x/src/lsp_project.rs` | 221 | No change needed — already uses `{ .. }` |

### P0e: Update tests

All test sites that construct `IntermediateType::Array { element_type, size: Some(N) }` must change to `IntermediateType::Array { element_type, dimensions: vec![ArrayDimension { lower: 0, upper: N-1 }] }` (or use the original bounds if the test represents a specific declaration).

Sites that use `size: None` (dynamic/unknown-size arrays) change to `dimensions: vec![]`.

Key test files:
- `intermediate_type.rs` — tests at lines 608, 617, 686, 724, 730, 811, 958
- `intermediates/array.rs` — tests at lines 384, 406, 523
- `intermediates/structure.rs` — tests at lines 705, 733, 768, 802, 1276, 1578
- `type_environment.rs` — test at line 501
- `type_category.rs` — test at line 93

### P0f: Add `TypeEnvironment` accessor for codegen

**File**: `compiler/analyzer/src/type_environment.rs`

Add a method that the codegen can use to resolve a named array type to its dimensions and element type:

```rust
impl TypeEnvironment {
    /// Returns the array dimensions and element type for a named array type.
    /// Returns None if the type is not found or is not an array.
    pub fn resolve_array_type(&self, type_name: &TypeName) -> Option<&IntermediateType> {
        let attrs = self.get(type_name)?;
        match &attrs.representation {
            it @ IntermediateType::Array { .. } => Some(it),
            _ => None,
        }
    }
}
```

The codegen (Step 5b) can then extract `dimensions` and `element_type` directly from the returned `IntermediateType::Array`, instead of needing to recover subranges from the AST.

### Impact on codegen plan

With this pre-work in place, Step 5b's named-type path simplifies. Instead of `types.resolve_array_type(type_name)` returning an opaque object with `.spec_as_subranges()`, it returns `&IntermediateType::Array { element_type, dimensions }`, and `register_array_variable` can extract `DimensionInfo` directly from `dimensions`:

```rust
SpecificationKind::Named(type_name) => {
    let array_type = types.resolve_array_type(type_name)
        .ok_or_else(|| Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(type_name.span(), "Unknown array type"),
        ))?;
    let IntermediateType::Array { element_type, dimensions } = array_type else {
        unreachable!("resolve_array_type guarantees Array variant");
    };
    // Convert ArrayDimension to DimensionInfo and register...
}
```

### Pre-work PR 0b: Widen `data_region_offset` to `u32` and `DataRegionOutOfBounds` to `u32`

**Goal**: Change `data_region_offset: u16` to `u32` in `CompileContext`. Arrays can easily exceed 64KB (`ARRAY[1..1000] OF DINT` = 8000 bytes), and the header's `data_region_bytes` field is already `u32`.

**File**: `compiler/codegen/src/compile.rs`

Changes:
1. `CompileContext.data_region_offset: u16` → `u32`
2. `StringVarInfo.data_offset: u16` → `u32`
3. `FbInstanceInfo.data_offset: u16` → `u32` (currently defined as `u16` at line ~420)
4. Update all `checked_add` call sites (STRING allocation at line 618, FB allocation at line 646, function-local string allocation at line 2404) to use `u32` arithmetic
5. Update `emit_str_store_var()` call sites that pass `data_offset` — the emitter takes `u16`, so this may require widening the emitter parameter too, or asserting the offset fits in `u16` for strings specifically

Add a `data_region_offset <= i32::MAX as u32` assertion at the point where `data_region_offset` is stored into a variable slot via `LOAD_CONST_I32` (see Key Design Decision 6).

**Also in this PR**: Widen `Trap::DataRegionOutOfBounds(u16)` to `DataRegionOutOfBounds(u32)` in `compiler/vm/src/error.rs`. With arrays, data region byte offsets can exceed 65535 (e.g., `ARRAY[1..32768] OF INT` = 262,144 bytes). The current `u16` payload truncates offsets in error messages, making defense-in-depth errors misleading. Update the `Display` impl and all `Trap::DataRegionOutOfBounds(offset as u16)` call sites in `vm.rs` to pass the full `u32` offset. This is a one-line type change + mechanical call-site updates.

Pure refactoring — no behavioral change for any program that compiles today.

### Pre-work PR 0c: Thread `TypeEnvironment` Through Compile Chain

**Goal**: Make `TypeEnvironment` available to `assign_variables()` and `emit_initial_values()` where it will be needed for named array type resolution.

**File**: `compiler/codegen/src/compile.rs`

The call chain is:
```
compile(_types: &TypeEnvironment)          // line 126 — already receives it, unused
  → compile_program_with_functions(...)    // line 143 — does NOT receive it
      → assign_variables(ctx, decls)       // line 188 — does NOT receive it
      → emit_initial_values(emitter, ctx, decls)  // line 209 — does NOT receive it
```

Changes (4 function signatures):
1. `compile()`: rename `_types` to `types`, pass to `compile_program_with_functions()`
2. `compile_program_with_functions()`: add `types: &TypeEnvironment` parameter, pass to `assign_variables()` and `emit_initial_values()`
3. `assign_variables()`: add `_types: &TypeEnvironment` parameter (unused until Step 5)
4. `emit_initial_values()`: add `_types: &TypeEnvironment` parameter (unused until Step 6)

Pure plumbing — no behavioral change.

### Pre-work PR 0d: Create `compile_array.rs` Module

**Goal**: Establish a separate module for array codegen to avoid further bloating `compile.rs` (already 3725 lines, well over the 1000-line module limit).

**File**: New file `compiler/codegen/src/compile_array.rs`

This PR creates the module with only the type definitions and the `ResolvedAccess` enum (see Step 4a revised). No logic yet — just the structural scaffolding that later steps populate.

```rust
//! Array code generation support.
//!
//! Handles array variable registration, index computation, and
//! array read/write compilation. Separated from compile.rs to
//! keep module sizes within the 1000-line guideline.

use std::collections::HashMap;
use ironplc_dsl::core::Id;
use ironplc_dsl::textual::{ArrayVariable, Expr, Variable, SymbolicVariableKind};
use crate::emit::Emitter;

// Re-export from compile.rs what's needed (or use pub(crate) on types there).

/// Normalized array specification, independent of AST representation.
/// Both inline (`ARRAY[1..3, 1..4] OF INT`) and named type paths
/// convert to this form before registration.
pub(crate) struct ArraySpec {
    /// Per-dimension bounds as (lower, upper) inclusive pairs.
    pub dimensions: Vec<(i32, i32)>,
    /// Element type name (e.g., "INT", "DINT").
    pub element_type_name: Id,
}

/// Metadata for a single dimension of an array, used for index computation.
pub(crate) struct DimensionInfo {
    pub lower_bound: i32,
    pub size: u32,
    pub stride: u32,
}

/// Metadata for an array variable, stored in CompileContext.
pub(crate) struct ArrayVarInfo {
    pub var_index: u16,
    pub desc_index: u16,
    pub data_offset: u32,
    pub element_var_type_info: super::VarTypeInfo,
    pub total_elements: u32,
    pub dimensions: Vec<DimensionInfo>,
}

/// The resolved target of a variable access.
///
/// This enum decouples variable resolution from code emission.
/// Each dispatch site (compile_expr, compile_statement) matches
/// on the variant and calls the appropriate emission logic.
///
/// Designed for extensibility: when struct access is implemented,
/// add a `StructField` variant and extend `resolve_access()`.
/// The dispatch sites gain a new match arm without changing shape.
pub(crate) enum ResolvedAccess<'a> {
    /// Simple named variable — use LOAD_VAR/STORE_VAR.
    Scalar { var_index: u16 },
    /// Array element — compute flat index, use LOAD_ARRAY/STORE_ARRAY.
    ArrayElement {
        info: &'a ArrayVarInfo,
        subscripts: Vec<&'a Expr>,
    },
    // Future variants:
    // StructField { base: ..., field_offset: ..., field_type: ... },
}
```

Add `mod compile_array;` to `lib.rs` (or to `compile.rs` as a submodule). The broader split of `compile.rs` into multiple modules (expressions, statements, builtins) is a separate effort tracked outside this plan.

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
- Operand 2: `u16` (little-endian) — array descriptor index into the type section's descriptor table

The descriptor index allows the VM to look up bounds and element type via a direct `Vec` index (O(1)) without requiring VarEntry to be implemented. The element type byte (I32=0, U32=1, I64=2, U64=3, F32=4, F64=5) lives in the `ArrayDescriptor`, not in the opcode. The verifier reads it from the descriptor.

Each instruction is 5 bytes: 1 opcode + 2 var_index + 2 desc_index.

Stack effects:
- `LOAD_ARRAY`: pops 1 (index), pushes 1 (value) → net 0
- `STORE_ARRAY`: pops 2 (value, index) → net -2

---

## Step 2: Emitter Methods

**File**: `compiler/codegen/src/emit.rs`

Add two methods. Both have two `u16` operands (var_index and desc_index).

**`emit_load_array(var_index: u16, desc_index: u16)`**:
- Emit `LOAD_ARRAY` opcode byte
- Emit `var_index` as 2 LE bytes
- Emit `desc_index` as 2 LE bytes
- Stack effect: pop 1 (the index is already on the stack), push 1 (the loaded value). Net: 0. The simplest implementation: call `self.pop_stack(1)` then `self.push_stack(1)`, or just no stack change.

**`emit_store_array(var_index: u16, desc_index: u16)`**:
- Emit `STORE_ARRAY` opcode byte
- Emit `var_index` as 2 LE bytes
- Emit `desc_index` as 2 LE bytes
- Stack effect: pop 2 (value and index). Call `self.pop_stack(2)`.

---

## Step 3: Array Descriptors in the Container

Array descriptors tell the VM the bounds and element type for runtime checking.

### 3a: Add `ArrayDescriptor` struct

**File**: `compiler/container/src/type_section.rs` (extend the existing type section)

```
pub struct ArrayDescriptor {
    pub element_type: u8,     // same encoding as element type byte (I32=0, U32=1, etc.)
    pub total_elements: u32,  // number of elements
}
```

On disk, each descriptor is 8 bytes:
```
[element_type: u8] [reserved: u8 = 0] [total_elements: u32 LE] [element_extra: u16 LE = 0]
```

Since the plan uses always-0-based descriptors (lower_bound is always 0), the on-disk format stores `total_elements` as a single `u32` rather than separate `lower_bound: i16` + `upper_bound: i16`. This uses the same 8 bytes but supports up to 2^32 elements in the container format. The container format spec (`bytecode-container-format.md:157-167`) should be updated to reflect this layout.

`element_extra` is reserved for future use (e.g., for arrays of strings, it would hold the max string length; for arrays of structs, it would hold a type descriptor index).

**Codegen limit**: The codegen enforces a maximum of 32768 elements per array to keep flat-index arithmetic within i32 range (see the i32 safety test in Step 9). This is a codegen-enforced limit, not a container format limit — the on-disk `u32` field supports raising this ceiling in the future without a format change.

### 3b: Extend `TypeSection`

Add `pub array_descriptors: Vec<ArrayDescriptor>` to `TypeSection`. Update serialization:
- After writing FB types, write array descriptor count (`u16 LE`) followed by each descriptor (8 bytes each)
- Update deserialization symmetrically

This follows the type section serialization order defined in the container format spec (line 527): `num_arrays (u16, LE)` appears after FB type descriptors.

**Do NOT add a header field for array descriptor count.** The count lives in the type section body, not in the header. The header's `num_fb_types` field is the only type-section count in the header. The array count follows the same pattern as function signatures — stored inline in the type section.

### 3c: Descriptor index in opcodes (no VarEntry dependency)

Per the container format spec (`bytecode-container-format.md:151-153`), each `VarEntry` has:
- `flags: u8` — bit 0 is `is_array`
- `extra: u16` — for arrays, this holds the array descriptor index

The VarEntry section is not yet implemented in `type_section.rs`. Rather than introducing a temporary workaround (e.g., a var_index→descriptor HashMap in the VM), the descriptor index is encoded directly in the `LOAD_ARRAY`/`STORE_ARRAY` opcodes as the second `u16` operand. The codegen assigns descriptor indices sequentially and emits them into the bytecode. The VM loads the descriptor table as a `Vec<ArrayDescriptor>` and indexes directly — O(1) per array access, no HashMap, no temporary serialization format.

This costs 1 extra byte per array opcode (u16 vs u8 for the second operand) but eliminates runtime lookup overhead in the VM dispatch loop and keeps the type section serialization spec-compliant.

**Future**: When VarEntry is implemented, the desc_index operand becomes redundant (VarEntry.extra already carries it), but it remains valid and costs nothing to keep. No migration needed.

### 3d: Update `ContainerBuilder`

**File**: `compiler/container/src/builder.rs`

Add method:
```
pub fn add_array_descriptor(&mut self, element_type: u8, total_elements: u32) -> u16
```

The builder deduplicates descriptors: if an identical `(element_type, total_elements)` pair already exists, return the existing index instead of appending a duplicate. Use a `HashMap<(u8, u32), u16>` as a dedup cache inside the builder. This is a trivial optimization that avoids creating 100 identical descriptors when 100 variables all declare `ARRAY[1..10] OF INT`, reducing container size and load-time parsing with zero runtime cost.

The codegen stores this index in `ArrayVarInfo.desc_index` and emits it into `LOAD_ARRAY`/`STORE_ARRAY` opcodes. No var_index is needed — the mapping from opcode to descriptor is carried by the opcode itself.

---

## Step 4: Array Tracking in Codegen

**File**: `compiler/codegen/src/compile_array.rs` (new module from Pre-work PR 0d)

### 4a: Types (already scaffolded in Pre-work PR 0d)

`ArraySpec`, `DimensionInfo`, `ArrayVarInfo`, and `ResolvedAccess` are defined in `compile_array.rs`. See Pre-work PR 0d for definitions.

### 4b: Add to `CompileContext`

**File**: `compiler/codegen/src/compile.rs`

```rust
struct CompileContext {
    // ... existing fields ...
    array_vars: HashMap<Id, ArrayVarInfo>,
}
```

Initialize `array_vars: HashMap::new()` in `CompileContext::new()`.

### 4c: Widen `data_region_offset` to `u32` (done in Pre-work PR 0b)

Already completed in Pre-work PR 0b.

### 4d: Thread `TypeEnvironment` through (done in Pre-work PR 0c)

Already completed in Pre-work PR 0c.

### 4e: Implement `resolve_access()`

**File**: `compiler/codegen/src/compile_array.rs`

Add the function that resolves a `Variable` AST node into a `ResolvedAccess`:

```rust
/// Resolves a variable reference into its access kind.
///
/// For named variables, returns Scalar with the variable table index.
/// For array variables, walks the ArrayVariable chain to collect
/// all subscripts and resolve the base variable's ArrayVarInfo.
///
/// This function is the single dispatch point for variable access
/// resolution. When struct access is added, extend the match in
/// this function and add a new ResolvedAccess variant — the call
/// sites in compile_expr and compile_statement stay unchanged.
pub(crate) fn resolve_access<'a>(
    ctx: &'a CompileContext,
    variable: &'a Variable,
) -> Result<ResolvedAccess<'a>, Diagnostic> {
    match variable {
        Variable::Symbolic(SymbolicVariableKind::Array(array_var)) => {
            // Walk the chain collecting subscript groups innermost-first,
            // then reverse. For nested arrays arr[i][j], the AST is:
            //   ArrayVariable {
            //       subscripted_variable: Array(ArrayVariable {
            //           subscripted_variable: Named(Id("arr")),
            //           subscripts: [i],
            //       }),
            //       subscripts: [j],
            //   }
            // We collect: [[j], [i]], reverse to [[i], [j]], flatten to [i, j].
            let mut levels: Vec<&[Expr]> = Vec::new();
            let mut current = array_var;
            loop {
                levels.push(&current.subscripts);
                match current.subscripted_variable.as_ref() {
                    SymbolicVariableKind::Array(inner) => {
                        current = inner;
                    }
                    SymbolicVariableKind::Named(named) => {
                        levels.reverse();
                        let all_subscripts: Vec<&Expr> =
                            levels.into_iter().flatten().collect();
                        let info = ctx.array_vars.get(&named.name)
                            .ok_or_else(|| Diagnostic::todo_with_span(
                                named.name.span(), file!(), line!()
                            ))?;
                        // Note: subscript count vs. dimension count is validated
                        // later in emit_flat_index(), not here. This keeps resolution
                        // simple and avoids duplicating the check. The error message
                        // will reference the emit site, not the resolution site.
                        return Ok(ResolvedAccess::ArrayElement {
                            info,
                            subscripts: all_subscripts,
                        });
                    }
                    other => {
                        return Err(Diagnostic::todo_with_span(
                            other.span(), file!(), line!()
                        ));
                    }
                }
            }
        }
        _ => {
            // Fall through to existing resolve_variable() for scalars.
            let var_index = resolve_variable(ctx, variable)?;
            Ok(ResolvedAccess::Scalar { var_index })
        }
    }
}
```

This replaces the plan's original `as_array_access()` and `resolve_array_access()` pair with a single function that returns a discriminated union. The dispatch sites in `compile_expr` and `compile_statement` match on the enum instead of doing ad-hoc `if let` checks. When struct access is added later, a `StructField` variant is added to `ResolvedAccess` and `resolve_access()` is extended — the dispatch sites gain a match arm without changing shape.

---

## Step 5: Handle `InitialValueAssignmentKind::Array` in `assign_variables()`

**File**: `compiler/codegen/src/compile_array.rs` (registration logic) and `compiler/codegen/src/compile.rs` (call site in `assign_variables()`)

Currently, the `match` on `decl.initializer` handles `Simple`, `String`, `FunctionBlock`, etc. The `Array` case falls through to the catch-all `_ => (...)` with no special handling. Add an explicit case.

### 5a: Normalize to `ArraySpec`, then register

Both `SpecificationKind::Inline` and `SpecificationKind::Named` are converted to `ArraySpec` first, then a single `register_array_variable` function handles all registration logic. This avoids duplicating the core logic across two code paths.

**File**: `compiler/codegen/src/compile_array.rs`

```rust
/// Converts an inline array specification (from the AST) to a normalized ArraySpec.
pub(crate) fn array_spec_from_inline(
    subranges: &ArraySubranges,
    span: &SourceSpan,
) -> Result<ArraySpec, Diagnostic> {
    let dimensions: Vec<(i32, i32)> = subranges.ranges.iter()
        .map(|range| {
            let lower = signed_integer_to_i32(&range.start)?;
            let upper = signed_integer_to_i32(&range.end)?;
            Ok((lower, upper))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    Ok(ArraySpec {
        dimensions,
        element_type_name: Id::from(subranges.type_name.to_string()),
    })
}

/// Converts a named array type (from the TypeEnvironment) to a normalized ArraySpec.
pub(crate) fn array_spec_from_named(
    element_type: &IntermediateType,
    dimensions: &[ArrayDimension],
) -> Result<ArraySpec, Diagnostic> {
    let dims: Vec<(i32, i32)> = dimensions.iter()
        .map(|d| (d.lower, d.upper))
        .collect();
    let element_type_name = intermediate_type_to_name(element_type)?;
    Ok(ArraySpec {
        dimensions: dims,
        element_type_name,
    })
}

/// Maps an IntermediateType to the IEC 61131-3 type name (as an Id) that
/// `resolve_type_name()` in compile.rs can look up. Only primitive types
/// are supported (arrays of complex types are out of scope).
fn intermediate_type_to_name(ty: &IntermediateType) -> Result<Id, Diagnostic> {
    let name = match ty {
        IntermediateType::Bool => "BOOL",
        IntermediateType::Int { size: ByteSized::B8 } => "SINT",
        IntermediateType::Int { size: ByteSized::B16 } => "INT",
        IntermediateType::Int { size: ByteSized::B32 } => "DINT",
        IntermediateType::Int { size: ByteSized::B64 } => "LINT",
        IntermediateType::UInt { size: ByteSized::B8 } => "USINT",
        IntermediateType::UInt { size: ByteSized::B16 } => "UINT",
        IntermediateType::UInt { size: ByteSized::B32 } => "UDINT",
        IntermediateType::UInt { size: ByteSized::B64 } => "ULINT",
        IntermediateType::Bytes { size: ByteSized::B8 } => "BYTE",
        IntermediateType::Bytes { size: ByteSized::B16 } => "WORD",
        IntermediateType::Bytes { size: ByteSized::B32 } => "DWORD",
        IntermediateType::Bytes { size: ByteSized::B64 } => "LWORD",
        IntermediateType::Real { size: ByteSized::B32 } => "REAL",
        IntermediateType::Real { size: ByteSized::B64 } => "LREAL",
        IntermediateType::Time { size: ByteSized::B32 } => "TIME",
        IntermediateType::Time { size: ByteSized::B64 } => "LTIME",
        _ => return Err(Diagnostic::todo(file!(), line!())),
    };
    Ok(Id::from(name))
}

/// Registers an array variable from a normalized ArraySpec.
/// Single code path for both inline and named array types.
pub(crate) fn register_array_variable(
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    id: &Id,
    var_index: u16,
    spec: &ArraySpec,
    span: &SourceSpan,
) -> Result<(u8, String), Diagnostic> {
    // 1. Resolve element type
    let element_vti = resolve_type_name(&spec.element_type_name)
        .ok_or_else(|| Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(span.clone(), "Unsupported array element type"),
        ))?;

    // 2. Build DimensionInfo from normalized bounds
    let mut dimensions: Vec<DimensionInfo> = Vec::new();
    let mut total_elements: u32 = 1;
    for &(lower, upper) in &spec.dimensions {
        let size = (upper as i64 - lower as i64 + 1) as u32;
        dimensions.push(DimensionInfo { lower_bound: lower, size, stride: 0 });
        total_elements = total_elements.checked_mul(size).ok_or_else(|| {
            Diagnostic::problem(
                Problem::NotImplemented,
                Label::span(span.clone(), "Array too large"),
            )
        })?;
    }

    // 3. Validate element limit (i32 safety for flat-index arithmetic)
    if total_elements > 32768 {
        return Err(Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(span.clone(), "Array exceeds maximum 32768 elements"),
        ));
    }

    // 4. Compute strides (reverse pass)
    let n = dimensions.len();
    if n > 0 {
        dimensions[n - 1].stride = 1;
        for k in (0..n-1).rev() {
            dimensions[k].stride = dimensions[k + 1].stride * dimensions[k + 1].size;
        }
    }

    // 5. Allocate data region space
    let data_offset = ctx.data_region_offset;
    let total_bytes = total_elements * 8;
    ctx.data_region_offset = ctx.data_region_offset
        .checked_add(total_bytes)
        .ok_or_else(|| Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(span.clone(), "Data region overflow"),
        ))?;

    // 6. Assert data_offset fits in i32 (stored in slot via LOAD_CONST_I32)
    if data_offset > i32::MAX as u32 {
        return Err(Diagnostic::problem(
            Problem::NotImplemented,
            Label::span(span.clone(), "Data region exceeds 2 GiB limit"),
        ));
    }

    // 7. Register descriptor in the container and get its index
    let element_type_byte = var_type_info_to_type_byte(&element_vti);
    let desc_index = builder.add_array_descriptor(element_type_byte, total_elements);

    // 8. Store in context
    ctx.array_vars.insert(id.clone(), ArrayVarInfo {
        var_index,
        desc_index,
        data_offset,
        element_var_type_info: element_vti,
        total_elements,
        dimensions,
    });

    let type_tag = iec_type_tag::OTHER;
    let type_name_str = format!("ARRAY OF {}", spec.element_type_name.to_string().to_uppercase());
    Ok((type_tag, type_name_str))
}
```

### 5b: Handle inline and named array specifications

**File**: `compiler/codegen/src/compile.rs`, in `assign_variables()`

```rust
InitialValueAssignmentKind::Array(array_init) => {
    let spec = match &array_init.spec {
        SpecificationKind::Inline(array_subranges) => {
            array_spec_from_inline(array_subranges, &decl.identifier.span())?
        }
        SpecificationKind::Named(type_name) => {
            let array_type = types.resolve_array_type(type_name)
                .ok_or_else(|| Diagnostic::problem(
                    Problem::NotImplemented,
                    Label::span(type_name.span(), "Unknown array type"),
                ))?;
            let IntermediateType::Array { element_type, dimensions } = array_type else {
                unreachable!("resolve_array_type guarantees Array variant");
            };
            array_spec_from_named(element_type, dimensions)?
        }
    };
    let (tag, name) = register_array_variable(
        ctx, builder, &id, index, &spec, &decl.identifier.span(),
    )?;
    (tag, name)
}
```

Both paths normalize to `ArraySpec` first, then call the single `register_array_variable` function. No code duplication.

### 5c: Helper function `var_type_info_to_type_byte`

**File**: `compiler/codegen/src/compile_array.rs`

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

**Visibility change**: Since `signed_integer_to_i32()` is currently private to `compile.rs` and `array_spec_from_inline()` lives in `compile_array.rs`, this function must be changed to `pub(crate)`. Alternatively, `array_spec_from_inline()` can be called from `compile.rs` where `signed_integer_to_i32()` is already in scope — choose whichever keeps the module boundary cleaner.

---

## Step 6: Array Initialization in `emit_initial_values()`

**Files**: `compiler/codegen/src/compile_array.rs` (helper) and `compiler/codegen/src/compile.rs` (call site in `emit_initial_values()`)

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
                // Note: `repeated.init` is `Box<Option<ArrayInitialElementKind>>`,
                // so deref the Box first, then match on the Option.
                match repeated.init.as_ref().as_ref() {
                    Some(inner) => {
                        // Recursively flatten the inner element
                        let inner_values = flatten_array_initial_values(
                            &[inner.clone()]
                        )?;
                        for _ in 0..count {
                            result.extend_from_slice(&inner_values);
                        }
                    }
                    None => {
                        // Repeated with no value means zero-fill; use integer 0.
                        // Construct via ConstantKind::integer_literal("0").unwrap()
                        // (defined in common.rs:37 — parses "0" into a SignedInteger).
                        let zero = ConstantKind::integer_literal("0")
                            .expect("literal '0' is always valid");
                        for _ in 0..count {
                            result.push(zero.clone());
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
STORE_ARRAY var_index, desc_index
```

**Note**: Initialization uses `STORE_ARRAY` with constant indices, which means the VM will bounds-check during init. This is correct and safe.

---

## Step 7: Compile Array Access (the `todo!()` replacement)

**Files**: `compiler/codegen/src/compile_array.rs` (helpers) and `compiler/codegen/src/compile.rs` (dispatch sites)

The `todo!()` is in `resolve_variable()` at line 3079 inside the `SymbolicVariableKind::Array` match arm. Array access cannot go through `resolve_variable()` because it returns a `u16` index — arrays need index computation code emitted, not just a variable index.

Instead, the dispatch sites in `compile_expr` and `compile_statement` call `resolve_access()` (from Step 4e) and match on the `ResolvedAccess` enum. The `resolve_variable()` function's `Array` arm remains as a fallback error — it should never be reached for well-formed programs since `resolve_access()` handles arrays before `resolve_variable()` is called.

### 7a: `resolve_access()` (already implemented in Step 4e)

The `resolve_access()` function in `compile_array.rs` replaces the plan's original `as_array_access()` + `resolve_array_access()` pair. See Step 4e for the implementation.

### 7b: Emit index computation

**File**: `compiler/codegen/src/compile_array.rs`

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

    // Check if all subscripts are integer literals.
    // If so, compute flat index at compile time with per-dimension bounds checking.
    if let Some(flat_index) = try_constant_flat_index(subscripts, dimensions, span)? {
        let const_index = ctx.add_i32_constant(flat_index);
        emitter.emit_load_const_i32(const_index);
        return Ok(());
    }

    // Variable case: emit runtime computation using i64 arithmetic to prevent overflow.
    // The subscript expression is compiled as i32 but lives in a 64-bit Slot (sign-extended).
    // SUB_I64/MUL_I64/ADD_I64 read the full i64 value, so no explicit widening is needed.
    // Emit: (s_0 - l_0) * stride_0 + (s_1 - l_1) * stride_1 + ... (all in i64)
    let subscript_op_type = (OpWidth::W32, Signedness::Signed);
    for (k, (subscript, dim)) in subscripts.iter().zip(dimensions.iter()).enumerate() {
        compile_expr(emitter, ctx, subscript, subscript_op_type)?;
        if dim.lower_bound != 0 {
            let lb_const = ctx.add_i64_constant(dim.lower_bound as i64);
            emitter.emit_load_const_i64(lb_const);
            emitter.emit_sub_i64();
        }
        if dim.stride != 1 {
            let stride_const = ctx.add_i64_constant(dim.stride as i64);
            emitter.emit_load_const_i64(stride_const);
            emitter.emit_mul_i64();
        }
        if k > 0 {
            emitter.emit_add_i64();  // accumulate into running sum
        }
    }
    Ok(())
}
```

After this sequence, the stack has exactly one value: the 0-based flat index as i64 in the Slot. The VM's `LOAD_ARRAY`/`STORE_ARRAY` handlers read it via `as_i64()` and bounds-check the full i64 value before narrowing (see Step 8c/8d).

**Constant index helper**:

```rust
fn try_constant_flat_index(
    subscripts: &[&Expr],
    dimensions: &[DimensionInfo],
    span: &SourceSpan,
) -> Result<Option<i32>, Diagnostic> {
    let mut flat_index: i32 = 0;
    for (subscript, dim) in subscripts.iter().zip(dimensions.iter()) {
        let value = match try_extract_integer_literal(subscript) {
            Some(v) => v,
            None => return Ok(None),  // not a literal — use runtime path
        };
        // Per-dimension bounds check at compile time
        let upper = dim.lower_bound + dim.size as i32 - 1;
        if value < dim.lower_bound || value > upper {
            return Err(Diagnostic::problem(
                Problem::ArrayIndexOutOfBounds,  // P2027
                Label::span(span.clone(), "Array index out of bounds"),
            ));
        }
        flat_index += (value - dim.lower_bound) * dim.stride as i32;
    }
    Ok(Some(flat_index))
}

/// Extracts an i32 value from an expression if it is a literal integer.
/// Returns None for any non-literal expression (variables, arithmetic, etc.)
/// — those fall through to the runtime index computation path.
/// This is NOT a constant-folding evaluator; it only matches direct literals
/// like `3` or `-5`. Expressions like `2+1` produce None and take the
/// runtime path, which is correct but not optimally efficient.
///
/// Handles two representations of negative constants:
/// 1. `IntegerLiteral` with `is_neg=true` (parser-level negation, e.g., `ARRAY[-5..5]`)
/// 2. `UnaryOp::Neg` wrapping a positive literal (expression-level negation, e.g., `-i`)
fn try_extract_integer_literal(expr: &Expr) -> Option<i32> {
    match &expr.kind {
        ExprKind::Const(ConstantKind::IntegerLiteral(lit)) => {
            // IntegerLiteral.value is a SignedInteger with is_neg flag.
            // Use signed_integer_to_i32 logic: negate if is_neg is set.
            let unsigned = i32::try_from(lit.value.value).ok()?;
            if lit.value.is_neg {
                unsigned.checked_neg()
            } else {
                Some(unsigned)
            }
        }
        ExprKind::UnaryOp(unary) if unary.op == UnaryOp::Neg => {
            // Handle expression-level negation: -(3) → -3
            match &unary.operand.kind {
                ExprKind::Const(ConstantKind::IntegerLiteral(lit)) => {
                    let val = i32::try_from(lit.value.value).ok()?;
                    if lit.value.is_neg {
                        // Double negation: -(-3) → 3
                        Some(val)
                    } else {
                        val.checked_neg()
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}
```

A general constant-folding pass (evaluating `2+1`, `N-1` where N is a constant, etc.) would benefit many areas of the compiler beyond arrays. It is explicitly deferred and is not needed for correctness — non-literal subscripts simply take the runtime index computation path.

### 7c: Array read (in expression context)

**File**: `compiler/codegen/src/compile.rs`

In `compile_variable_read()`, replace the default arm that calls `resolve_variable()` with a call to `resolve_access()` and match on the result:

```rust
_ => {
    match resolve_access(ctx, variable)? {
        ResolvedAccess::Scalar { var_index } => {
            emit_load_var(emitter, var_index, op_type);
        }
        ResolvedAccess::ArrayElement { info, subscripts } => {
            emit_flat_index(emitter, ctx, &subscripts, &info.dimensions, &variable.span())?;
            emitter.emit_load_array(info.var_index, info.desc_index);
            // No truncation on load — matches the scalar load path.
            // The value was already truncated at store time; upper bits are zero.
        }
    }
    Ok(())
}
```

The `BitAccess` arm above this remains unchanged. The key change is that `resolve_variable()` is no longer called directly — `resolve_access()` handles both scalars and arrays through the `ResolvedAccess` enum. When struct access is added, a new match arm appears here.

### 7d: Array write (in assignment context)

**File**: `compiler/codegen/src/compile.rs`

In `compile_statement()`, in the `StmtKind::Assignment` arm (line 1085), after the existing bit-access and string checks, replace the `resolve_variable()` call with `resolve_access()`:

```rust
// ... existing bit access check (unchanged) ...
// ... existing string check (unchanged) ...

// Resolve the target variable using resolve_access().
match resolve_access(ctx, &assignment.target)? {
    ResolvedAccess::Scalar { var_index } => {
        let type_info = target_name.and_then(|name| ctx.var_type_info(name));
        let op_type = type_info
            .map(|ti| (ti.op_width, ti.signedness))
            .unwrap_or(DEFAULT_OP_TYPE);
        compile_expr(emitter, ctx, &assignment.value, op_type)?;
        if let Some(ti) = type_info {
            emit_truncation(emitter, ti);
        }
        emit_store_var(emitter, var_index, op_type);
    }
    ResolvedAccess::ArrayElement { info, subscripts } => {
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
        emitter.emit_store_array(info.var_index, info.desc_index);
    }
}
```

**Stack order**: `STORE_ARRAY` spec says `[value, I32] → []` — I32 (index) is on top, value is below. The code above pushes value first (via `compile_expr`), then index (via `emit_flat_index`). The VM pops index first (TOS), then value. This matches.

When struct access is added, a new `ResolvedAccess::StructField` arm appears here alongside the existing scalar and array arms.

---

## Step 8: VM Implementation

### 8a: New trap type

**File**: `compiler/vm/src/error.rs`

Add to the `Trap` enum:
```
ArrayIndexOutOfBounds { var_index: u16, index: i32, total_elements: u32 }
```

Implement `Display` to show: `"array index out of bounds: index {index} for array variable {var_index} with {total_elements} elements"`.

**File**: `compiler/vm/resources/problem-codes.csv`

Add: `V4005,ArrayIndexOutOfBounds,Array index out of bounds,true`

### 8b: Array descriptor table in VM

**File**: `compiler/vm/src/vm.rs` (or a new `compiler/vm/src/array.rs` if the file is already large)

At container load time, parse the array descriptor section from the type section into a `Vec<ArrayDescriptor>`:

```rust
struct ArrayDescriptor {
    total_elements: u32,  // from on-disk u32 field
    element_type: u8,     // for verifier validation (not used by VM at runtime)
}

// In the VM's loaded state:
array_descriptors: Vec<ArrayDescriptor>  // indexed by desc_index from opcodes
```

The VM reads descriptors from the type section in index order. The codegen emits descriptor indices into `LOAD_ARRAY`/`STORE_ARRAY` opcodes, so the VM uses the index as a direct `Vec` offset — O(1) lookup with no hashing. This is the same pattern as the constant pool (`Vec` indexed by pool index from opcodes).

### 8c: LOAD_ARRAY handler

In the VM's opcode dispatch loop, add after existing handlers. The VM uses `Slot` for all stack operations (not typed push/pop), so follow the existing patterns:

```
LOAD_ARRAY => {
    let var_index = read_u16_le(bytecode, &mut pc);
    let desc_index = read_u16_le(bytecode, &mut pc);
    let index_slot = stack.pop()?;

    // Read index as i64 to catch overflow from i64 flat-index arithmetic.
    // The codegen emits i64 arithmetic (SUB_I64, MUL_I64, ADD_I64) to prevent
    // intermediate overflow. Reading as i64 here ensures no truncation before
    // the bounds check — a huge out-of-range i64 value is caught, not silently
    // truncated to an in-bounds i32.
    let index_i64 = index_slot.as_i64();

    // Look up array descriptor by index (O(1) Vec access)
    let desc = array_descriptors.get(desc_index as usize)
        .ok_or(Trap::InvalidVariable(var_index))?;

    // Bounds check: 0 <= index < total_elements (checked in i64 before narrowing)
    if index_i64 < 0 || index_i64 >= desc.total_elements as i64 {
        return Err(Trap::ArrayIndexOutOfBounds {
            var_index,
            index: index_i64 as i32,  // truncate for error message only
            total_elements: desc.total_elements,
        });
    }
    let index = index_i64 as u32;  // safe: checked above, value is in [0, 32768)

    // Check variable scope access (defense-in-depth)
    scope.check_access(var_index)?;

    // Read data_offset from the variable's slot
    let data_offset = variables.load(var_index)?.as_i32() as u32 as usize;

    // Compute byte offset into data region
    let byte_offset = data_offset + index as usize * 8;

    // Bounds-check data region access (defense-in-depth)
    if byte_offset + 8 > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(byte_offset as u32));
    }

    // Read 8 bytes and push as Slot
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&data_region[byte_offset..byte_offset + 8]);
    let raw = i64::from_le_bytes(buf);
    stack.push(Slot::from_i64(raw))?;
}
```

**Note on element_type**: The `element_type` byte lives in the `ArrayDescriptor`, not in the opcode. The VM does not use it at runtime — it pushes the raw 8-byte value as a `Slot` regardless of type. Type interpretation happens at the consumer (the next opcode). This matches how `FB_LOAD_PARAM` works. The verifier validates `element_type` at load time.

### 8d: STORE_ARRAY handler

Symmetric to LOAD_ARRAY. Stack has `[..., value, index]`:

```
STORE_ARRAY => {
    let var_index = read_u16_le(bytecode, &mut pc);
    let desc_index = read_u16_le(bytecode, &mut pc);
    let index_slot = stack.pop()?;  // TOS = index
    let value_slot = stack.pop()?;  // second = value

    // Same i64 bounds check as LOAD_ARRAY
    let index_i64 = index_slot.as_i64();
    let desc = array_descriptors.get(desc_index as usize)
        .ok_or(Trap::InvalidVariable(var_index))?;
    if index_i64 < 0 || index_i64 >= desc.total_elements as i64 {
        return Err(Trap::ArrayIndexOutOfBounds {
            var_index,
            index: index_i64 as i32,
            total_elements: desc.total_elements,
        });
    }
    let index = index_i64 as u32;  // safe: checked above

    // Check variable scope access (defense-in-depth)
    scope.check_access(var_index)?;

    let data_offset = variables.load(var_index)?.as_i32() as u32 as usize;
    let byte_offset = data_offset + index as usize * 8;
    if byte_offset + 8 > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(byte_offset as u32));
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
| `array_1d_variable_index_load` | `x := arr[i];` | Emits `LOAD_VAR i, LOAD_CONST_I64 1, SUB_I64, LOAD_ARRAY` |
| `array_1d_variable_index_store` | `arr[i] := 42;` | Emits `LOAD_CONST 42, LOAD_VAR i, LOAD_CONST_I64 1, SUB_I64, STORE_ARRAY` |
| `array_multidim_constant_index` | `matrix[2,3]` where `ARRAY[1..3,1..4]` | Flat index = (2-1)*4+(3-1) = 6 |
| `array_multidim_variable_index` | `matrix[i,j]` | Emits SUB_I64, MUL_I64, SUB_I64, ADD_I64 sequence |
| `array_nonzero_lower_bound` | `ARRAY[-5..5] OF INT`, access `arr[0]` | 0-based index = 0-(-5) = 5 |
| `array_constant_oob_error` | `arr[11]` on `ARRAY[1..10]` | Compile-time diagnostic error |
| `array_initialization` | `ARRAY[1..3] OF INT := [10, 20, 30]` | Emits STORE_ARRAY for each element |
| `array_initialization_repeated` | `ARRAY[1..6] OF INT := [3(10), 3(20)]` | Emits 6 STORE_ARRAY calls |
| `array_sint_no_truncation_on_load` | `VAR arr: ARRAY[1..3] OF SINT; END_VAR x := arr[1];` | Emits LOAD_ARRAY only (no TRUNC — matches scalar load path; truncation happens at store time) |
| `array_sint_truncation_on_store` | `VAR arr: ARRAY[1..3] OF SINT; END_VAR arr[1] := 42;` | Emits TRUNC_I8 before STORE_ARRAY |
| `array_named_type` | `TYPE MY_ARR : ARRAY[1..5] OF INT; END_TYPE VAR arr : MY_ARR; END_VAR x := arr[3];` | Named type path resolves dimensions from TypeEnvironment |

### Codegen negative tests (compile-time rejection)

These tests verify that invalid array programs produce clear compile-time diagnostics, not panics or silent misbehavior.

| Test name | ST program | Expected error |
|-----------|-----------|----------------|
| `array_when_exceeds_element_limit_then_error` | `VAR arr: ARRAY[1..32769] OF INT; END_VAR` | "Array exceeds maximum 32768 elements" |
| `array_when_dimension_mismatch_then_error` | `VAR matrix: ARRAY[1..3, 1..4] OF INT; END_VAR x := matrix[1];` | "Wrong number of array subscripts" |
| `array_when_constant_oob_below_then_error` | `VAR arr: ARRAY[1..10] OF INT; END_VAR x := arr[0];` | Compile-time "Array index out of bounds" (P2027) |
| `array_when_single_element_then_works` | `VAR arr: ARRAY[1..1] OF INT; END_VAR arr[1] := 42; x := arr[1];` | Compiles successfully (degenerate but valid) |
| `array_when_multidim_exceeds_limit_then_error` | `VAR matrix: ARRAY[1..200, 1..200] OF INT; END_VAR` | "Array exceeds maximum 32768 elements" (40000 > 32768) |

### Flat index i64 safety test

Add a unit test in `compiler/codegen/tests/` that validates the i64 overflow invariant:

```rust
#[test]
fn flat_index_arithmetic_when_worst_case_subscript_then_fits_i64() {
    // Worst case: subscript = i32::MAX, lower_bound = i32::MIN, stride = 32768.
    // Intermediate value: (i32::MAX - i32::MIN) as i64 * 32768
    //                   = 4_294_967_295 * 32768
    //                   = 140_737_488_289_792
    // i64::MAX = 9_223_372_036_854_775_807 — fits with massive margin.
    let max_range: i64 = i32::MAX as i64 - i32::MIN as i64;
    let max_stride: i64 = 32768;
    let result = max_range.checked_mul(max_stride);
    assert!(result.is_some(), "flat index must fit in i64");
    assert!(result.unwrap() <= i64::MAX);
}
```

This test protects the invariant: even with the most extreme possible i32 subscript values and maximum stride, the i64 flat index computation cannot overflow. The VM's i64 bounds check then catches any out-of-range result before narrowing to u32.

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

End-to-end ST programs compiled and run. Verify results by reading the variable table after execution — the test harness calls `vm.variable_table().load(var_index)` on the `sum` variable and asserts the expected value. Follow the pattern used by existing integration tests (e.g., FB integration tests in `compiler/vm/tests/`).

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
END_PROGRAM
```

**Assertion**: After execution, read `sum` from the variable table and assert `sum.as_i32() == 150`.

---

## Step 10: Bytecode Verifier Spec Update (no implementation)

**File**: `specs/design/bytecode-verifier-rules.md`

Add these verification rules for documentation purposes (verifier implementation is a separate task):

1. `LOAD_ARRAY`/`STORE_ARRAY` operand `desc_index` must be a valid index into the array descriptor table
2. The descriptor's `element_type` must be a valid type byte (0-5)
3. When VarEntry is implemented: `var_index` must have `is_array` flag set and `VarEntry.extra` must equal `desc_index`
4. Stack must have an integer value on top when `LOAD_ARRAY`/`STORE_ARRAY` execute (i64 from flat-index arithmetic or i32 from constant index)
5. For `STORE_ARRAY`, the value below the index must be compatible with the descriptor's `element_type`
6. **Data region bounds**: For each array variable, verify at load time that `data_offset + total_elements * 8 <= data_region_bytes`. This ensures the array's data region allocation is fully contained within the allocated data region — the runtime `scope.check_access(var_index)` only validates access to the variable slot holding `data_offset`, not the data region bytes themselves. Without this check, a malformed container could reference data region bytes belonging to another program instance.
7. **No data region overlap** (multi-program): When multiple programs share a data region, verify that each program's array data allocations `[data_offset, data_offset + total_elements * 8)` do not overlap with other programs' allocations (arrays, strings, FB instances). This prevents one program's array write from corrupting another program's data.

---

## Implementation Order

```
Pre-work 0a: IntermediateType change   ← no dependencies, lands FIRST (high blast radius across analyzer)
Pre-work 0b: Widen data_region_offset + DataRegionOutOfBounds(u32) ← merge after 0a to avoid rebase churn
Pre-work 0c: Thread TypeEnvironment    ← merge after 0a to avoid rebase churn
Pre-work 0d: Create compile_array.rs   ← merge after 0a to avoid rebase churn
Step 1: Opcode constants               ← no dependencies
Step 2: Emitter methods                ← depends on Step 1
Step 3: Container descriptors          ← depends on Step 1
Step 4: CompileContext + resolve_access ← depends on Pre-work 0d
Step 5: assign_variables()             ← depends on Steps 3, 4, Pre-work 0a/0b/0c
Step 6: emit_initial_values()          ← depends on Steps 2, 5
Step 7: Compile array access           ← depends on Steps 2, 4, 5
Step 8: VM implementation              ← depends on Steps 1, 3
Step 9: Tests                          ← depends on everything above
Step 10: Verifier spec                 ← can be done anytime
```

### Recommended PR structure

Each PR is independently reviewable. Pre-work PRs are pure refactoring with zero feature risk.

| PR | Content | Scope | Dependencies |
|----|---------|-------|-------------|
| **PR 0a** | Pre-work: `IntermediateType` per-dimension bounds + fix `size_in_bytes()` u8 truncation (P0a-P0f) | Analyzer only | None |
| **PR 0b** | Pre-work: Widen `data_region_offset` to `u32` + `StringVarInfo.data_offset` + `FbInstanceInfo.data_offset` + `Trap::DataRegionOutOfBounds` to `u32` | Codegen + VM refactoring | PR 0a (merge order) |
| **PR 0c** | Pre-work: Thread `TypeEnvironment` through compile chain (4 function signatures) | Codegen plumbing | PR 0a (merge order) |
| **PR 0d** | Pre-work: Create `compile_array.rs` with type stubs (`ArraySpec`, `DimensionInfo`, `ArrayVarInfo`, `ResolvedAccess`) | Structural scaffolding | PR 0a (merge order) |
| **PR 1** | Container layer: opcodes + emitter + descriptors + builder (Steps 1-3) + update `bytecode-container-format.md` array descriptor layout (lines 157-167) to reflect `total_elements: u32` instead of `lower_bound/upper_bound` pair | Container + codegen crate + spec | None |
| **PR 2** | VM: trap type + descriptor loading + LOAD/STORE_ARRAY handlers + VM tests (Step 8) | VM crate | PR 1 |
| **PR 3** | Codegen: `CompileContext.array_vars` + `resolve_access()` + variable registration in `assign_variables()` (Steps 4-5) | Codegen | PRs 0a-0d, 1 |
| **PR 4** | Codegen: array initialization in `emit_initial_values()` + init tests (Step 6) | Codegen | PR 3 |
| **PR 5** | Codegen: array access compilation + end-to-end tests (Step 7 + Step 9) | Codegen + integration | PRs 2, 4 |
| **PR 6** | Verifier spec update (Step 10) | Docs only | None |

**PR 0a should merge first** — it touches the `IntermediateType::Array` variant across 10+ files in the analyzer, so any concurrent analyzer changes would cause merge conflicts. PRs 0b-0d are independent of 0a and can be reviewed in parallel, but should merge after 0a to avoid rebase churn. PRs 1 and 2 build the "bottom half" (container format + VM). PRs 3-5 build the "top half" (codegen) incrementally. PR 6 is documentation-only.

**Problem code verification**: Before starting PR 1, verify that `P2027` (compiler) and `V4005` (VM) are unallocated in `compiler/problems/resources/problem-codes.csv` and `compiler/vm/resources/problem-codes.csv` respectively.

---

## Key Files Reference

| File | What changes |
|------|-------------|
| `compiler/container/src/opcode.rs` | Add `LOAD_ARRAY`, `STORE_ARRAY` constants |
| `compiler/container/src/type_section.rs` | Add `ArrayDescriptor` struct, extend `TypeSection` serialization |
| `compiler/container/src/builder.rs` | Add `add_array_descriptor()` method |
| `compiler/codegen/src/emit.rs` | Add `emit_load_array(var_index, desc_index)`, `emit_store_array(var_index, desc_index)` |
| `compiler/codegen/src/compile_array.rs` | **New file**: `ArraySpec`, `DimensionInfo`, `ArrayVarInfo`, `ResolvedAccess`, `resolve_access()`, `register_array_variable()`, `emit_flat_index()`, constant folding helpers |
| `compiler/codegen/src/compile.rs` | Widen `data_region_offset` to `u32`, add `array_vars` to `CompileContext`, thread `TypeEnvironment`, update `assign_variables()`, `emit_initial_values()`, `compile_variable_read()`, `compile_statement()` dispatch sites |
| `compiler/problems/resources/problem-codes.csv` | Add `P2027,ArrayIndexOutOfBounds` |
| `docs/compiler/problems/P2027.rst` | Document the compile-time array index out of bounds diagnostic |
| `compiler/vm/src/error.rs` | Add `ArrayIndexOutOfBounds` trap variant; widen `DataRegionOutOfBounds` from `u16` to `u32` (Pre-work PR 0b) |
| `compiler/vm/resources/problem-codes.csv` | Add `V4005` |
| `compiler/vm/src/vm.rs` | Add LOAD_ARRAY and STORE_ARRAY handlers, array descriptor loading |
| `specs/design/bytecode-container-format.md` | Update array descriptor layout (lines 157-167) to reflect `total_elements: u32` format (PR 1) |
| `specs/design/bytecode-verifier-rules.md` | Add array verification rules |
| `compiler/analyzer/src/intermediate_type.rs` | Widen `size_in_bytes()` return type from `Option<u8>` to `Option<u32>` (Pre-work PR 0a) |

## Risks and Open Questions

1. **Codegen enforces 32768-element limit**: The container format supports u32 `total_elements`, but the codegen limits arrays to 32768 elements for the initial implementation. `ARRAY[1..200, 1..200]` = 40000 elements will produce a compile-time error. Since the flat-index arithmetic uses i64 (which has massive headroom), this limit can be raised in the future by simply increasing the constant — no arithmetic changes needed.

2. **Mixed constant/variable subscripts**: `matrix[2, j]` has one constant and one variable subscript. The current design treats all subscripts as variable when any one is variable (simpler). An optimization to evaluate constant subscripts at compile time and emit `LOAD_CONST (2-1)` instead of `LOAD_VAR, SUB` can be added later but is not required for correctness.

3. **`DataRegionOutOfBounds` trap payload widened to `u32`**: Pre-work PR 0b widens `Trap::DataRegionOutOfBounds` from `u16` to `u32` so that array data region offsets (which can exceed 65535) are reported accurately in error messages.

4. **Problem codes verified**: `P2027` is unallocated (last allocated is P2026) in `compiler/problems/resources/problem-codes.csv`. `V4005` is unallocated (codes jump from V4003 to V9001) in `compiler/vm/resources/problem-codes.csv`. Verified 2026-03-15.

5. **`compile.rs` size**: At 3725 lines, `compile.rs` is already 3.7x over the 1000-line module guideline. Array codegen goes in the new `compile_array.rs` module (Pre-work PR 0d) to avoid further growth. A broader split of `compile.rs` into expression/statement/builtin modules is a separate effort that should not be mixed with array feature work.

6. **Scalability to struct access**: The `ResolvedAccess` enum (Step 4e) is designed to extend to struct field access. When structs are implemented, add a `StructField` variant and extend `resolve_access()`. The dispatch sites in `compile_variable_read()` and `compile_statement()` gain new match arms without changing shape. The per-type info hashmaps in `CompileContext` (`string_vars`, `fb_instances`, `array_vars`) scale linearly; consolidate into a `VariableMetadata` enum *before* adding struct field access (not after) to avoid accumulating a fourth parallel HashMap.
