# Fix: Global Struct Field in Conditions (P9999)

## Problem

Using a global struct field in an IF condition (e.g., `IF setup.FLAG THEN`)
from within a FUNCTION body fails with P9999 "Not implemented" at
`codegen/src/compile.rs:2150`. Simple struct field reads via assignment work
(e.g., `result := phys.T0`), but any usage that requires `op_type()` —
IF/WHILE/REPEAT conditions, comparison operands, CASE selectors — fails.

## Root Cause

The analyzer's expression type resolution pass (`xform_resolve_expr_types.rs`)
returns `None` for `StructuredVariable` expressions, leaving `expr.resolved_type`
unpopulated. Codegen's `op_type()` then fails because it requires
`resolved_type` to determine the operation width.

Additionally, global variables were not registered in the `ExprTypeResolver`'s
`var_types` map when folding FUNCTION/FB/PROGRAM bodies, so even after adding
struct field resolution logic, the resolver couldn't look up the root variable's
type.

## Changes

### 1. Add `elementary_type_name_for` to TypeEnvironment

**File**: `compiler/analyzer/src/type_environment.rs`

Maps an `IntermediateType` back to its elementary `TypeName` by scanning the
elementary type table. Returns `None` for complex types.

### 2. Resolve struct field types in ExprTypeResolver

**File**: `compiler/analyzer/src/xform_resolve_expr_types.rs`

- Added `global_var_types` field to persist global variable type mappings across
  POU folds.
- Override `fold_library` to pre-collect top-level `VAR_GLOBAL` variable types.
- Modified `seed_implicit_globals` to inject global variable types into each POU
  scope.
- Added `resolve_structured_variable_type` and `resolve_parent_struct_type`
  helpers that walk the struct chain to resolve the leaf field's type.
- Changed `resolve_variable_type` to handle `StructuredVariable` via the new
  helpers.

### 3. End-to-end test

**File**: `compiler/codegen/tests/end_to_end_global.rs`

Test that a global struct BOOL field used as an IF condition in a FUNCTION
compiles and executes correctly.
