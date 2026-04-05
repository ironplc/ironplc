# Fix resolved_type for array subscript and dereference expressions

## Problem

The expression type resolution pass (`xform_resolve_expr_types`) returns `None`
for `SymbolicVariableKind::Array` and `SymbolicVariableKind::Deref` variable
kinds. This means `resolved_type` is not populated on expressions like `pt^[i]`
(deref + array subscript) or `arr[i]` (direct array subscript).

Codegen functions `op_type()` and `storage_bits()` require `resolved_type` to
select correct opcodes. When these expressions appear in comparisons, unary ops,
or stdlib function arguments, the missing type causes a P9999 error.

## Solution

1. Add `elementary_type_name_from_repr()` to `TypeEnvironment` for reverse
   `IntermediateType` to `TypeName` lookup.

2. Add `array_element_types: HashMap<Id, TypeName>` to `ExprTypeResolver` to
   track the element type for array and REF_TO array variables.

3. Populate `array_element_types` in `insert()` for all array-like declarations:
   inline arrays, named array types, REF_TO inline arrays, REF_TO named arrays,
   simple type aliases that resolve to arrays, and late-resolved types.

4. Implement `resolve_variable_type()` for `Array` (returns element type by
   walking to the base variable) and `Deref` (returns target type of the
   reference).

5. Add `find_base_variable_name()` helper to walk nested variable kinds
   (Array/Deref/Named chains) to the root named variable.

## Files changed

- `compiler/analyzer/src/type_environment.rs` - new method
- `compiler/analyzer/src/xform_resolve_expr_types.rs` - core fix + unit tests
- `compiler/codegen/tests/end_to_end_ref.rs` - end-to-end test
