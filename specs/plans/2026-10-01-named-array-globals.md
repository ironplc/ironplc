# Named array type globals

## Goal

A `VAR_GLOBAL` declared with a named array type
(`TYPE A3 : ARRAY[1..3] OF DINT; END_TYPE` ... `g : A3;`) compiles and runs
like the same global declared with an inline `ARRAY[...] OF ...`, in a
`CONFIGURATION`, a `RESOURCE` and at the top level (issue #1924).

## Architecture

The `global_var_decl` grammar rule parses every named type as
`InitialValueAssignmentKind::Simple`. A `VAR` block parses the same
declaration as `LateResolvedType`, which
`xform_resolve_late_bound_type_initializer` turns into
`Array(SpecificationKind::Named(..))`. Codegen lays out an array only from the
`Array` form, so the global never reaches `array_vars` and the first subscript
fails with P9999.

The fix belongs in the analyzer, so that every backend sees one form:
`xform_resolve_decl_types` already records the declared `TypeId` on each
`VarDecl`. When that id's representation is `IntermediateType::Array` and the
initializer is a `Simple` without a value, the pass rewrites the initializer
to `Array(Named(type_name))` with no initial values, which is what a `VAR`
declaration of the same type has. The decision uses the declaration's type id,
not its name (tracking issue #1968, step 6a, A1-A2). Codegen does not change.

## Prefactoring

None needed. The change adds one normalization step to an existing pass and
removes no special case; codegen is untouched.

## Design doc reference

`specs/design/expression-type-resolution.md` (declaration type ids).

## File map

- `compiler/analyzer/src/xform_resolve_decl_types.rs`: the normalization and
  its unit tests.
- `compiler/codegen/tests/it/end_to_end_named_array_global.rs` (new) and
  `compiler/codegen/tests/it/main.rs`: end-to-end tests.
- `specs/design/expression-type-resolution.md`: one bullet.

## Tasks

- [ ] Unit tests in `xform_resolve_decl_types` (failing first)
- [ ] End-to-end tests: configuration global read, write, persistence across
      scans, multi-dimensional, array of structures, top-level `VAR_GLOBAL`,
      STRING elements (failing first)
- [ ] Normalization in `xform_resolve_decl_types`
- [ ] Design doc bullet
- [ ] `cd compiler && just`, `cd specs && just`
- [ ] Out of scope: an initial value on a named-array declaration
      (`g : A3 := [1, 2, 3]`) is a syntax error in `VAR` and `VAR_GLOBAL`
      alike; open an issue
