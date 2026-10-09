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

**In scope:** Named enumeration types declared via `TYPE ... END_TYPE`, used in variable declarations, assignments, expressions (comparisons), CASE selectors, and structure field initializers. Inline enumerations declared in place of a type name (`VAR x : (A, B, C); END_VAR`, section 10).

**Out of scope (deferred):**
- Inline enumerations in structure fields, function inputs and global variables (#1946)
- Enumeration-typed function/FB parameters (VAR_INPUT, VAR_OUTPUT, VAR_IN_OUT)
- Enumeration-typed array elements
- Explicit numeric assignment to enum values (not standard IEC 61131-3)

---

## 1. Ordinal Encoding

**REQ-EN-codegen-001** Each enumeration value is assigned a 0-based ordinal equal to its position in the declaration. For `TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE`, the ordinals are RED=0, GREEN=1, BLUE=2.

**REQ-EN-codegen-002** The ordinal is the runtime integer value stored in the variable slot. No translation table is consulted at runtime.

**REQ-EN-codegen-003** At codegen level, all enumeration values are stored as DINT (signed 32-bit integer, W32). The analyzer's `SemanticType::Enumeration { underlying_type }` uses B8/B16 for semantic sizing, but the codegen always uses `VarTypeInfo { op_width: W32, signedness: Signed, storage_bits: 32 }`. This avoids unnecessary truncation opcodes since every VM slot is 64 bits wide and there is no memory savings from narrow storage.

**REQ-EN-codegen-004** Enumerations support only assignment (`:=`), equality comparison (`=`, `<>`), and CASE matching. Arithmetic operators (ADD, SUB, MUL, DIV, MOD, EXPT) are not valid on enumeration types.

## 2. Variable Allocation

**REQ-EN-codegen-010** A variable declared with a named enumeration type (`VAR x : COLOR; END_VAR`) receives `VarTypeInfo { op_width: W32, signedness: Signed, storage_bits: 32 }`.

**REQ-EN-codegen-011** The variable occupies one slot in the variable table, identical to any other scalar integer variable.

**REQ-EN-codegen-012** The `VarNameEntry` in the debug section uses `iec_type_tag::DINT` (tag 3) and the user-defined type name as `type_name` (e.g., `"COLOR"`). This follows [ADR-0019](../adrs/0019-type-encoding-in-debug-variable-names.md) — the tag drives value interpretation, the type_name identifies the enum for display.

## 3. Initialization

The analyzer resolves the member each variable starts at and completes the
declaration's initializer with it ([Initial Values](initial-values.md));
codegen stores its ordinal.

**REQ-EN-codegen-020** When a variable has an explicit initial value (`VAR x : COLOR := GREEN; END_VAR`), the codegen emits `LOAD_CONST_I32(ordinal)` + `STORE_VAR_I32`, where `ordinal` is the 0-based position of `GREEN` in the type declaration. No truncation is needed (32-bit storage per REQ-EN-codegen-003).

**REQ-EN-codegen-021** When a variable has no explicit initial value (`VAR x : COLOR; END_VAR`), the initial ordinal is determined by the type declaration's default value. For `TYPE COLOR : (RED, GREEN, BLUE) := RED; END_TYPE`, the default is RED's ordinal (0).

**REQ-EN-codegen-022** When the type declaration specifies no default (e.g., `TYPE COLOR : (RED, GREEN, BLUE); END_TYPE`), the initial ordinal is the first declared value's: 0, unless that value is given an explicit one (`(A := 1, B := 5)` starts at 1).

**REQ-EN-codegen-023** Function-local enum variables are re-initialized on every call (IEC 61131-3 stateless function requirement), following the same initialization rules as REQ-EN-codegen-020 through REQ-EN-codegen-022.

## 4. Expressions

**REQ-EN-codegen-030** An `ExprKind::EnumeratedValue` compiles to `LOAD_CONST_I32(ordinal)`, pushing the ordinal onto the stack.

**REQ-EN-codegen-031** A qualified enumeration reference (`COLOR#GREEN`) resolves the ordinal as a member of the type it names.

**REQ-EN-codegen-032** An unqualified enumeration reference (`GREEN`) resolves the ordinal as a member of the type the analyzer gave the expression (REQ-EN-codegen-081). Two enumerations may declare the same value name.

**REQ-EN-codegen-033** Enumeration equality comparison (`x = GREEN`) compiles to the same integer comparison sequence as any other integer type: load both operands, emit `EQ_I32`.

**REQ-EN-codegen-034** Assignment of an enumeration value to an enum variable (`x := GREEN`) compiles to `LOAD_CONST_I32(ordinal)` + `STORE_VAR_I32`.

## 5. CASE Selectors

**REQ-EN-codegen-040** A `CaseSelectionKind::EnumeratedValue` in a CASE statement compiles by loading the selector expression, loading the enum value's ordinal as a constant, and comparing with `EQ_I32`.

**REQ-EN-codegen-041** Multiple enum values in the same CASE arm combine with boolean OR, following the same pattern as integer CASE selectors.

## 6. Structure Field Initialization

**REQ-EN-codegen-050** An enumerated value in a struct initializer is stored as its ordinal: the analyzer completes the variable's initializer with the member every enumeration field starts at (see [Initial Values](initial-values.md)), and codegen emits `LOAD_CONST_I32(ordinal)`, which is then stored into the struct field's data region slot.

**REQ-EN-codegen-051** Structure fields of enumeration type already receive the correct `op_type` via `resolve_field_op_type`, which delegates `SemanticType::Enumeration` to its underlying type (`compiler/codegen/src/compile_struct.rs:99`).

## 7. Debug Section: Enum Definition Table (Tag 9)

The existing debug section Tag Registry reserves tags 4-8 for other purposes. This design adds Tag 9 (ENUM_DEF) for enumeration definitions.

**REQ-EN-codegen-060** The debug section Tag Registry entry for Tag 9 is:

| Tag | Name | Status | Description |
|-----|------|--------|-------------|
| 9   | ENUM_DEF | v1 | Enumeration type definitions (type name → ordered value names) |

**REQ-EN-container-061** The ENUM_DEF sub-table payload format is:

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

**REQ-EN-codegen-062** Value names appear in ordinal order: the first entry is ordinal 0, the second is ordinal 1, etc. The ordinal is implicit from position.

**REQ-EN-codegen-063** A reader that does not recognize Tag 9 skips it using the directory's `size` field (existing extensibility mechanism per the container format spec).

**REQ-EN-codegen-064** Only enumeration types are emitted in the ENUM_DEF table, one entry each: a named enumeration or alias under its type name, and an inline enumeration under the name REQ-EN-codegen-092 gives it.

## 8. Playground Display

**REQ-EN-codegen-070** When the playground displays a variable whose `type_name` matches an ENUM_DEF entry, it shows the value name followed by the ordinal in parentheses. For example: `GREEN (1)`.

**REQ-EN-codegen-071** When the raw ordinal does not match any entry in the ENUM_DEF table (e.g., out of range due to corruption), the playground falls back to showing the integer value formatted according to the `iec_type_tag`, per ADR-0019 graceful degradation.

**REQ-EN-codegen-072** When no ENUM_DEF table is present (older container, stripped debug section), the playground displays the integer value using the `iec_type_tag`, which is always valid per REQ-EN-codegen-012.

## 9. Enumeration Facts from the Analyzer

Ordinals are decided once, in the analyzer, so every code generator uses the same ones. Codegen does not walk the declarations, number members or keep a table of value names.

**REQ-EN-codegen-080** The analyzer records each enumeration type's members in declaration order, their ordinals (explicit member values included, numbered by `resolve_ordinal_values`) and its default with the type, in `SemanticType::Enumeration::members`, for named enumerations, aliases and the anonymous types of inline enumerations alike. Codegen reads an ordinal by `(TypeId, value)`.

**REQ-EN-codegen-081** The analyzer gives every unqualified enumerated value in an expression a type (`Expr::expr_type`): the assignment target's, the other comparison operand's, or the function block input's when that type declares the value, else the type of the one enumeration in scope that declares it. When several do, the analyzer reports P2043. Codegen looks the ordinal up in the value's `expr_type`.

**REQ-EN-codegen-082** The default ordinal is the type's declared default, or its first member's ordinal. An alias has its base's members and default, unless it declares a default of its own.

**REQ-EN-codegen-083** A value outside an expression is a member of the type of the place it appears in: a variable's initial value of the declared type (`VarDecl::type_id`), a CASE label of the selector's type, a structure field initializer of the field's type, and a function block member initializer of the member's declared type. A structure field without an initializer starts at its enumeration's default.

## 10. Inline Enumerations

An inline enumeration spells its values where a type name would go: `VAR e : (A, B) := B; END_VAR`. The analyzer enters it as an anonymous type of its own, recorded on `VarDecl::type_id`, so two declarations that spell the same list are two types (ADR-0055). It otherwise compiles as a named enumeration with the same value list does. It may be declared in any variable section of a program, function block, function or method.

**REQ-EN-codegen-090** The values of an inline enumeration take the ordinals a named enumeration with the same value list would give them (REQ-EN-codegen-001), explicit member values included: in `(X := 1, Y := 5)`, `X` is 1 and `Y` is 5.

**REQ-EN-codegen-091** A variable declared with an inline enumeration is allocated as a named enumeration variable is (REQ-EN-codegen-010, REQ-EN-codegen-011), and its `VarNameEntry` carries `iec_type_tag::DINT`.

**REQ-EN-codegen-092** An inline enumeration's debug type name is made from its type id when the debug section is written, as `(ANONYMOUS ENUMERATION 42)`. A parenthesis cannot start an identifier, so the name is never a declared type's. The variable's `VarNameEntry::type_name` is that name, and the ENUM_DEF table holds one entry under it per declaration.

**REQ-EN-codegen-093** An inline enumeration variable's explicit initial value is resolved as a member of the declaration's own type. Without an initial value, the variable starts at the type's default, its first member's ordinal.

**REQ-EN-codegen-094** A function- or method-local inline enumeration variable is re-initialized on every call, as REQ-EN-codegen-023 requires of a named one.

**REQ-EN-codegen-095** An unqualified value that an inline enumeration shares with another enumeration resolves in the type the analyzer gave it (REQ-EN-codegen-081): in `a := Y`, `Y` is a member of `a`'s type.
