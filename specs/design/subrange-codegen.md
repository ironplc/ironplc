# Design: Subrange Type Code Generation

## Overview

This design specifies how the IronPLC compiler generates bytecode for IEC 61131-3 subrange types used as variable types. Subrange types restrict an integer base type to a subset of its values (section 2.3.3.1). For example, `TYPE MY_RANGE : INT (1..100); END_TYPE` defines a type whose values must be in the inclusive range [1, 100].

The design builds on:

- **[Enumeration Code Generation](enumeration-codegen.md)**: Precedent for derived-type codegen with no new opcodes
- **[Bytecode Instruction Set](bytecode-instruction-set.md)**: Integer load/store/compare opcodes reused for subrange values
- **[Bytecode Container Format](bytecode-container-format.md)**: Debug section variable name entries

## Design Goals

1. **No new opcodes** — subrange variables compile to the same integer load, store, and arithmetic opcodes as their base type. The VM is unaware of subrange constraints.
2. **Base-type equivalence** — a subrange variable has the same runtime cost and layout as a variable of its base type. One slot, same width and signedness.
3. **Correct default initialization** — uninitialized subrange variables default to the lower bound of the range, per IEC 61131-3 section 2.4.3.1.
4. **Alias transparency** — type aliases (`ALIAS : BASE_RANGE`) are fully supported and behave identically to the base subrange type.

## Scope

**In scope:** Named subrange types declared via `TYPE ... END_TYPE`, type aliases to subrange types, inline subrange specifications in variable declarations, use in variable declarations with and without explicit initialization, participation in arithmetic and comparison expressions, signed and unsigned integer base types.

**Out of scope (deferred):**
- Runtime bounds checking (clamping or trapping on out-of-range assignment)
- Type-declared default values (`:= 50` in `TYPE MY_RANGE : INT (1..100) := 50; END_TYPE` — the `50` is parsed but not propagated to codegen; default is always `min_value` for now)
- Subrange-typed function/FB parameters (VAR_INPUT, VAR_OUTPUT, VAR_IN_OUT)
- Subrange-typed array elements
- Subrange types in structure field declarations (already works via `compile_struct.rs`)

---

## 1. Storage Encoding

**REQ-SR-001** A subrange variable occupies the same number of VM slots as its base type: one slot (64 bits).

**REQ-SR-002** The `VarTypeInfo` for a subrange variable inherits the `op_width`, `signedness`, and `storage_bits` from the base type. For example, `INT (1..100)` uses `VarTypeInfo { op_width: W32, signedness: Signed, storage_bits: 16 }`.

**REQ-SR-003** For subrange types with an unsigned base type (USINT, UINT, UDINT, ULINT), the `signedness` is `Unsigned`.

**REQ-SR-004** The base type resolution follows the `IntermediateType::Subrange { base_type, .. }` chain recursively, matching the existing behavior in `compile_struct::resolve_field_op_type()`.

## 2. Late-Bound Type Resolution

**REQ-SR-010** When a variable is declared with a named subrange type and no initializer (`VAR x : MY_RANGE; END_VAR`), the parser produces `InitialValueAssignmentKind::LateResolvedType`. The late-bound resolver must convert this to `InitialValueAssignmentKind::Subrange(SpecificationKind::Named(type_name))`.

**REQ-SR-011** The late-bound resolver checks `IntermediateType::is_subrange()` on the type environment entry before falling through to the scoped type table.

**REQ-SR-012** The scoped type table match handles `TypeDefinitionKind::Subrange` by producing `InitialValueAssignmentKind::Subrange(SpecificationKind::Named(type_name))`.

**REQ-SR-013** When a variable is declared with a named subrange type and an explicit initializer (`VAR x : MY_RANGE := 75; END_VAR`), the parser produces `InitialValueAssignmentKind::Simple`. The codegen `assign_variables` function detects subrange types in the `Simple` arm by consulting the type environment.

## 3. Variable Allocation

**REQ-SR-020** A variable declared with `InitialValueAssignmentKind::Subrange(SpecificationKind::Named(type_name))` receives `VarTypeInfo` resolved from the type environment's `IntermediateType::Subrange`.

**REQ-SR-021** A variable declared with `InitialValueAssignmentKind::Subrange(SpecificationKind::Inline(spec))` receives `VarTypeInfo` resolved from the inline specification's `ElementaryTypeName`.

**REQ-SR-022** A variable declared with `InitialValueAssignmentKind::Simple` whose type name resolves to a subrange in the type environment receives `VarTypeInfo` from the subrange's base type.

**REQ-SR-023** The `VarNameEntry` in the debug section uses the base type's `iec_type_tag` and the user-defined type name as `type_name`.

## 4. Initialization

**REQ-SR-030** When a subrange variable has no explicit initial value and arrives as `InitialValueAssignmentKind::Subrange(Named(type_name))`, the codegen emits `LOAD_CONST` with the subrange's `min_value` (lower bound) followed by `STORE_VAR`.

**REQ-SR-031** When a subrange variable has no explicit initial value and arrives as `InitialValueAssignmentKind::Subrange(Inline(spec))`, the codegen extracts the lower bound from `spec.subrange.start` and emits it as the default.

**REQ-SR-032** When a subrange variable has an explicit initial value (`VAR x : MY_RANGE := 75; END_VAR`), it arrives as `InitialValueAssignmentKind::Simple` and the existing Simple initialization path emits the constant.

**REQ-SR-033** For base types narrower than the register width (e.g., SINT is 8-bit in a 32-bit register), truncation instructions are emitted after loading the default value, following the same pattern as `compile_struct::emit_truncation_for_field()`.

**REQ-SR-034** For 64-bit base types (LINT, ULINT), the codegen emits `LOAD_CONST_I64` and `STORE_VAR_I64` instead of the 32-bit variants.

## 5. Type Aliases

**REQ-SR-040** A type alias (`TYPE ALIAS : BASE_RANGE; END_TYPE`) is resolved by `xform_resolve_type_decl_environment` to `DataTypeDeclarationKind::Subrange(Named(BASE_RANGE))`. The type environment stores the alias with the same `IntermediateType::Subrange` as the base type.

**REQ-SR-041** A variable declared with a type alias (`VAR x : ALIAS; END_VAR`) resolves identically to a variable declared with the base subrange type. The `min_value` used for default initialization comes from the resolved `IntermediateType::Subrange`.

**REQ-SR-042** Nested type aliases (`TYPE BASE : INT (10..50); MID : BASE; TOP : MID; END_TYPE`) resolve transitively. A variable of type `TOP` has the same `IntermediateType::Subrange` (with `min_value=10`, `max_value=50`) as a variable of type `BASE`.

## 6. Expressions

**REQ-SR-050** A subrange variable participates in arithmetic expressions (ADD, SUB, MUL, DIV, MOD) using the same opcodes as its base type. No special handling is needed — the variable's `VarTypeInfo` determines the correct opcode width.

**REQ-SR-051** A subrange variable participates in comparison expressions (EQ, NE, LT, LE, GT, GE) using the same opcodes as its base type.

**REQ-SR-052** Assignment to a subrange variable uses the same `STORE_VAR` opcode as the base type, with truncation applied for narrow types per REQ-SR-033.
