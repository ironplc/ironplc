# Apply the field defaults of a STRUCT type declaration

Issue: #1523

## Goal

`TYPE Motor : STRUCT speed : INT := 100; END_STRUCT; END_TYPE` gives every
`Motor` variable `speed = 100` unless the variable's own initializer sets
it, instead of zero.

## Architecture

The parser keeps each field's initializer (`StructureElementDeclaration.init`)
but the analyzer reduces it to `IntermediateStructField::has_default: bool`,
so codegen never sees the value. `initialize_struct_fields` (codegen) takes
the value from the variable's initializer or falls back to zero.

- `IntermediateStructField` gains `default:
  Option<StructInitialValueAssignmentKind>`: the declared value in the shape
  a variable's field initializer already has (a constant, an enumerated
  value, a nested structure). `intermediates/structure.rs` fills it.
- `FieldInitInfo` carries it; `initialize_struct_fields` uses the variable's
  initializer first, then the field default, then zero. A nested structure
  field with a partial initializer keeps the defaults of the fields it does
  not set.

## Prefactoring

`IntermediateStructField` is built in 58 places (most of them tests). Adding
the field with `None` everywhere is a mechanical, behaviour-preserving
commit of its own; the feature commit then sets it in the one place that
reads a type declaration.

## Design doc reference

`specs/design/structure-codegen-memory-layout.md` if it states how fields are
initialized; otherwise none.

## File map

- `compiler/analyzer/src/intermediate_type.rs` (field), every constructor
- `compiler/analyzer/src/intermediates/structure.rs` (fill it)
- `compiler/codegen/src/compile_struct_init.rs` (use it)
- `compiler/codegen/tests/it/end_to_end_struct_type_defaults.rs` (new)

## Tasks

- [ ] Prefactor: the `default` field, `None` everywhere
- [ ] Tests: #1523 reproduction (INT, DINT, LREAL), a variable initializer
      overriding a default, a nested structure with defaults, a partial
      nested initializer, an enum and a STRING field default
- [ ] Fill and use the default
- [ ] `cd compiler && just`
- [ ] Delete this plan
