# Range Type Compilation and Execution Support

## Goal

Enable IronPLC to compile and run programs that use subrange types as variable types. Currently, ironplc can parse subrange type declarations (`TYPE MY_RANGE : INT (1..100); END_TYPE`) and validate them in the analyzer, but using a subrange type as a variable type fails at code generation because the `InitialValueAssignmentKind::Subrange` variant is not handled in the codegen pipeline.

## Background

IEC 61131-3 §2.3.3.1 defines subrange types as derived data types that restrict an integer base type to a subset of values. For example:

```
TYPE
  MY_RANGE : INT (1..100) := 50;
END_TYPE

PROGRAM main
  VAR
    x : MY_RANGE;    (* variable declared with subrange type *)
    y : MY_RANGE := 75;
  END_VAR
  x := 42;
END_PROGRAM
```

**What works today:**
- Parsing subrange type declarations (parser)
- Parsing variables declared with subrange type names (parser — these become `LateResolvedType`)
- Subrange type validation: bounds checking, base type validation (`intermediates/subrange.rs`)
- Subrange as struct fields in codegen (`compile_struct.rs` — follows `base_type` recursion)
- CASE statement subrange selectors (`compile_stmt.rs` — `CaseSelectionKind::Subrange`)
- plc2plc round-trip for subrange type declarations

**What is missing (the gaps this plan fills):**

1. **Late-bound resolution** — When a variable is declared as `x : MY_RANGE`, the parser produces `InitialValueAssignmentKind::LateResolvedType("MY_RANGE")`. The resolver in `xform_resolve_late_bound_type_initializer.rs` checks the type table, finds `TypeDefinitionKind::Subrange`, and falls through to the wildcard arm (`_ => Err(Diagnostic::todo_with_type(...))`). It needs to produce `InitialValueAssignmentKind::Subrange(SpecificationKind::Named(type_name))`.

2. **Variable allocation in codegen** — `compile_setup.rs::assign_variables()` handles `Simple`, `String`, `FunctionBlock`, `Array`, `Reference`, `Structure`, and `EnumeratedType` initializers but hits the wildcard `_ =>` arm for `Subrange`. It needs to resolve the subrange's base type via the type environment and assign the correct `VarTypeInfo` (width, signedness, storage bits).

3. **Variable initialization in codegen** — `compile_setup.rs::emit_initial_values()` similarly has no arm for `Subrange`. It needs to emit the initial value (explicit or default). Per IEC 61131-3 §2.4.3.1, the default value for a subrange type is the lower bound of the range.

4. **End-to-end execution tests** — No test currently declares a variable with a subrange type and runs it through the full pipeline.

## Architecture

The approach treats subrange variables as their base type for storage and operations, but with subrange-aware default initialization. This mirrors how `compile_struct.rs` already handles subrange fields — it resolves `IntermediateType::Subrange { base_type, .. }` by recursing into `base_type` for OpType resolution.

**No new opcodes or VM changes are needed.** A subrange variable occupies the same number of slots as its base type and uses the same load/store operations.

**Runtime bounds checking** (clamping or trapping when a value assigned to a subrange variable is outside the declared range) is **out of scope** for this plan. This is a significant runtime feature that requires new VM instructions and should be designed separately. Many real-world PLC implementations also defer or omit runtime bounds checking.

## File Map

| File | Change |
|------|--------|
| `compiler/analyzer/src/xform_resolve_late_bound_type_initializer.rs` | Handle `TypeDefinitionKind::Subrange` in LateResolvedType resolution |
| `compiler/analyzer/src/type_environment.rs` | Add `resolve_subrange_type()` helper (following `resolve_struct_type`/`resolve_array_type` pattern) |
| `compiler/codegen/src/compile_setup.rs` | Handle `InitialValueAssignmentKind::Subrange` in `assign_variables()` and `emit_initial_values()` |
| `compiler/codegen/tests/end_to_end_subrange.rs` | New end-to-end test file |
| `compiler/codegen/tests/compile_subrange.rs` | New bytecode-level test file (optional, if needed) |

## Tasks

### Phase 1: Late-Bound Type Resolution

- [ ] **1.1** In `xform_resolve_late_bound_type_initializer.rs`, add a `TypeDefinitionKind::Subrange` arm to the match at line ~180 that produces `InitialValueAssignmentKind::Subrange(SpecificationKind::Named(name))`
- [ ] **1.2** Add a unit test: `fold_initial_value_when_subrange_type_then_resolves_to_subrange`

### Phase 2: Type Environment Helper

- [ ] **2.1** In `type_environment.rs`, add `resolve_subrange_type(&self, type_name: &TypeName) -> Option<&IntermediateType>` following the same pattern as `resolve_struct_type` and `resolve_array_type`
- [ ] **2.2** Add unit tests for the new method

### Phase 3: Codegen — Variable Allocation

- [ ] **3.1** In `compile_setup.rs::assign_variables()`, add an `InitialValueAssignmentKind::Subrange` arm that:
  - Resolves the subrange type from the type environment (both `Named` and `Inline` variants)
  - Extracts the `IntermediateType::Subrange { base_type, .. }` 
  - Calls `resolve_field_op_type(base_type)` to get the correct `VarTypeInfo`
  - Inserts the type info into `ctx.var_types`
  - Produces the correct debug type tag (inheriting from the base type)

### Phase 4: Codegen — Variable Initialization

- [ ] **4.1** In `compile_setup.rs::emit_initial_values()`, add an `InitialValueAssignmentKind::Subrange` arm that:
  - For inline subrange specs with a declared default: emits the constant
  - For named subrange types: looks up the `IntermediateType::Subrange { min_value, .. }` and emits `min_value` as the default (per IEC 61131-3 §2.4.3.1 — leftmost value)
  - Emits truncation if the base type is narrower than the register width
  - Stores the value into the variable
- [ ] **4.2** Handle the `emit_function_local_prologue` path for subrange variables in function locals

### Phase 5: End-to-End Tests

- [ ] **5.1** Create `compiler/codegen/tests/end_to_end_subrange.rs` with tests:
  - `end_to_end_when_subrange_var_no_init_then_default_is_lower_bound` — verifies default = min_value
  - `end_to_end_when_subrange_var_with_init_then_uses_init_value` — verifies explicit initial value
  - `end_to_end_when_subrange_var_assigned_then_stores_value` — verifies assignment works
  - `end_to_end_when_subrange_var_in_expression_then_computes_correctly` — verifies arithmetic
  - `end_to_end_when_subrange_alias_var_then_works` — verifies `ALIAS : BASE_RANGE := 25;`
  - `end_to_end_when_inline_subrange_var_then_works` — verifies `x : INT (1..100);`

### Phase 6: CI Verification

- [ ] **6.1** Run `cd compiler && just` to verify all checks pass

## Design Decisions

### Subrange variables are stored as their base type

A variable of type `MY_RANGE : INT (1..100)` occupies the same memory as an `INT`. The VM sees no difference. This is consistent with how `compile_struct.rs` already handles subrange fields (line 100: `IntermediateType::Subrange { base_type, .. } => resolve_field_op_type(base_type)`).

### Default value is the lower bound

IEC 61131-3 §2.4.3.1 specifies that when no initial value is given, the default is the "leftmost value" of the subrange. For `INT (1..100)`, the default is `1`, not `0`. This matches the existing behavior in `compile_struct.rs::emit_default_for_field()` (line 381).

### No runtime bounds checking in this phase

Adding runtime bounds checking (trap or clamp when assigning a value outside the range) requires:
- A new VM instruction or a check sequence after every store
- Design decisions about behavior (trap vs. clamp vs. no-op)
- Significant performance implications

This is deferred to a future plan. The current implementation provides the same behavior as most PLC runtimes — the subrange type is used for documentation and static analysis, with the actual variable stored using the base type's full range.

### Both inline and named subrange specifications are supported

Variables can be declared with either:
- Named: `x : MY_RANGE;` (references a TYPE declaration)
- Inline: `x : INT (1..100);` (subrange defined at the variable declaration)

Both paths must work through `InitialValueAssignmentKind::Subrange(SpecificationKind::Named(...))` and `InitialValueAssignmentKind::Subrange(SpecificationKind::Inline(...))` respectively.
