# Design: Enumeration Code Generation and Debug Display

## Overview

This design specifies how the IronPLC compiler generates bytecode for IEC 61131-3 user-defined enumerations and how the debug toolchain displays enumeration values to the user. Enumerations are a core IEC 61131-3 feature (section 2.3.3.1) that maps a set of named identifiers to an integer encoding.

The design builds on:

- **[ADR-0019](../adrs/0019-type-encoding-in-debug-variable-names.md)**: Type encoding in debug variable names — enums use the underlying integer's `iec_type_tag` with the user-defined `type_name`
- **[Bytecode Container Format](bytecode-container-format.md)**: Debug section Tag Registry and sub-table format
- **[Bytecode Instruction Set](bytecode-instruction-set.md)**: Integer load/store/compare opcodes reused for enum values

## Design Goals

1. **No new opcodes** — enumerations compile to the same DINT load, store, and compare opcodes. The VM is unaware of enumerations.
2. **Zero-cost abstraction** — an enum variable has the same runtime cost as a DINT variable (one 64-bit slot, 32-bit operations, no truncation).
3. **Graceful debug degradation** — the `iec_type_tag` always shows a valid integer interpretation. The ENUM_DEF table adds human-readable names as an optional enhancement.
4. **Declarative ordinal mapping** — ordinals are determined by declaration order (0-based), matching IEC 61131-3 semantics. No explicit numeric assignment is needed.

## Scope

**In scope:** Named enumeration types declared via `TYPE ... END_TYPE`, used in variable declarations, assignments, expressions (comparisons), CASE selectors, and structure field initializers.

**Out of scope (deferred):**
- Inline (anonymous) enumeration types (`VAR x : (A, B, C); END_VAR`)
- Enumeration-typed function/FB parameters (VAR_INPUT, VAR_OUTPUT, VAR_IN_OUT)
- Enumeration-typed array elements
- Explicit numeric assignment to enum values (not standard IEC 61131-3)

---

## 1. Ordinal Encoding

**REQ-EN-001** Each enumeration value is assigned a 0-based ordinal equal to its position in the declaration. For `TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE`, the ordinals are RED=0, GREEN=1, BLUE=2.

**REQ-EN-002** The ordinal is the runtime integer value stored in the variable slot. No translation table is consulted at runtime.

**REQ-EN-003** At codegen level, all enumeration values are stored as DINT (signed 32-bit integer, W32). The analyzer's `IntermediateType::Enumeration { underlying_type }` uses B8/B16 for semantic sizing, but the codegen always uses `VarTypeInfo { op_width: W32, signedness: Signed, storage_bits: 32 }`. This avoids unnecessary truncation opcodes since every VM slot is 64 bits wide and there is no memory savings from narrow storage.

**REQ-EN-004** Enumerations support only assignment (`:=`), equality comparison (`=`, `<>`), and CASE matching. Arithmetic operators (ADD, SUB, MUL, DIV, MOD, EXPT) are not valid on enumeration types.

## 2. Variable Allocation

**REQ-EN-010** A variable declared with a named enumeration type (`VAR x : COLOR; END_VAR`) receives `VarTypeInfo { op_width: W32, signedness: Signed, storage_bits: 32 }`.

**REQ-EN-011** The variable occupies one slot in the variable table, identical to any other scalar integer variable.

**REQ-EN-012** The `VarNameEntry` in the debug section uses `iec_type_tag::DINT` (tag 3) and the user-defined type name as `type_name` (e.g., `"COLOR"`). This follows [ADR-0019](../adrs/0019-type-encoding-in-debug-variable-names.md) — the tag drives value interpretation, the type_name identifies the enum for display.

## 3. Initialization

**REQ-EN-020** When a variable has an explicit initial value (`VAR x : COLOR := GREEN; END_VAR`), the codegen emits `LOAD_CONST_I32(ordinal)` + `STORE_VAR_I32`, where `ordinal` is the 0-based position of `GREEN` in the type declaration. No truncation is needed (32-bit storage per REQ-EN-003).

**REQ-EN-021** When a variable has no explicit initial value (`VAR x : COLOR; END_VAR`), the initial ordinal is determined by the type declaration's default value. For `TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE`, the default is RED's ordinal (0).

**REQ-EN-022** When the type declaration specifies no default (e.g., `TYPE COLOR : (RED, GREEN, BLUE); END_TYPE`), the initial ordinal is 0 (the first declared value).

**REQ-EN-023** Function-local enum variables are re-initialized on every call (IEC 61131-3 stateless function requirement), following the same initialization rules as REQ-EN-020 through REQ-EN-022.

## 4. Expressions

**REQ-EN-030** An `ExprKind::EnumeratedValue` compiles to `LOAD_CONST_I32(ordinal)`, pushing the ordinal onto the stack.

**REQ-EN-031** A qualified enumeration reference (`COLOR#GREEN`) resolves the ordinal using the explicit type name and value name.

**REQ-EN-032** An unqualified enumeration reference (`GREEN`) resolves the ordinal using the value name alone. The semantic analyzer guarantees unqualified names are unambiguous within scope.

**REQ-EN-033** Enumeration equality comparison (`x = GREEN`) compiles to the same integer comparison sequence as any other integer type: load both operands, emit `EQ_I32`.

**REQ-EN-034** Assignment of an enumeration value to an enum variable (`x := GREEN`) compiles to `LOAD_CONST_I32(ordinal)` + `STORE_VAR_I32`.

## 5. CASE Selectors

**REQ-EN-040** A `CaseSelectionKind::EnumeratedValue` in a CASE statement compiles by loading the selector expression, loading the enum value's ordinal as a constant, and comparing with `EQ_I32`.

**REQ-EN-041** Multiple enum values in the same CASE arm combine with boolean OR, following the same pattern as integer CASE selectors.

## 6. Structure Field Initialization

**REQ-EN-050** A `StructInitialValueAssignmentKind::EnumeratedValue` in a struct initializer compiles by emitting `LOAD_CONST_I32(ordinal)`, which is then stored into the struct field's data region slot.

**REQ-EN-051** Structure fields of enumeration type already receive the correct `op_type` via `resolve_field_op_type`, which delegates `IntermediateType::Enumeration` to its underlying type (`compiler/codegen/src/compile_struct.rs:99`).

## 7. Debug Section: Enum Definition Table (Tag 9)

The existing debug section Tag Registry reserves tags 4-8 for other purposes. This design adds Tag 9 (ENUM_DEF) for enumeration definitions.

**REQ-EN-060** The debug section Tag Registry entry for Tag 9 is:

| Tag | Name | Status | Description |
|-----|------|--------|-------------|
| 9   | ENUM_DEF | v1 | Enumeration type definitions (type name → ordered value names) |

**REQ-EN-061** The ENUM_DEF sub-table payload format is:

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | count | u16 | Number of enum type entries |
| 2 | entries | [EnumDefEntry; count] | Variable size each |

Each EnumDefEntry (variable size):

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | type_name_length | u8 | Length of type name in bytes |
| 1 | type_name | [u8; type_name_length] | UTF-8 type name (e.g., "COLOR") |
| 1+N | value_count | u16 | Number of enumeration values |
| 3+N | values | [EnumValueName; value_count] | Value names in ordinal order |

Each EnumValueName (variable size):

| Offset | Field | Type | Description |
|--------|-------|------|-------------|
| 0 | name_length | u8 | Length of value name in bytes |
| 1 | name | [u8; name_length] | UTF-8 value name (e.g., "RED") |

**REQ-EN-062** Value names appear in ordinal order: the first entry is ordinal 0, the second is ordinal 1, etc. The ordinal is implicit from position.

**REQ-EN-063** A reader that does not recognize Tag 9 skips it using the directory's `size` field (existing extensibility mechanism per the container format spec).

**REQ-EN-064** Only named enumeration types (declared via `TYPE ... END_TYPE`) are emitted in the ENUM_DEF table.

## 8. Playground Display

**REQ-EN-070** When the playground displays a variable whose `type_name` matches an ENUM_DEF entry, it shows the value name followed by the ordinal in parentheses. For example: `GREEN (1)`.

**REQ-EN-071** When the raw ordinal does not match any entry in the ENUM_DEF table (e.g., out of range due to corruption), the playground falls back to showing the integer value formatted according to the `iec_type_tag`, per ADR-0019 graceful degradation.

**REQ-EN-072** When no ENUM_DEF table is present (older container, stripped debug section), the playground displays the integer value using the `iec_type_tag`, which is always valid per REQ-EN-012.

## 9. Ordinal Map Construction

**REQ-EN-080** The codegen builds the ordinal map by walking `LibraryElementKind::DataTypeDeclaration(Enumeration(decl))` entries in the library AST. For each `EnumerationDeclaration` whose `spec_init.spec` is `SpecificationKind::Inline(values)`, the codegen enumerates `values.values` and records `(type_name, value_name) → ordinal`.

**REQ-EN-081** The ordinal map also maintains a reverse lookup from unqualified value names to `(type_name, ordinal)` for resolving unqualified references per REQ-EN-032.

**REQ-EN-082** The ordinal map also stores the type declaration's default value (from `spec_init.default`) as a pre-resolved ordinal, used by REQ-EN-021.

**REQ-EN-083** The ordinal map is built once at codegen entry and stored in `CompileContext` for use by all codegen phases.
