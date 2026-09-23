# Prefactor: move structure initialization into its own module

## Goal

Move the structure-field initialization code out of
`compiler/codegen/src/compile_struct.rs` into a new
`compiler/codegen/src/compile_struct_init.rs`, with no change in behaviour.

This is the first of two prefactors for #1382 (STRING fields of
array-of-struct elements). The #1382 fix adds an array-of-structure arm to
`initialize_struct_fields` so that STRING headers inside array elements are
written. `compile_struct.rs` is 918 lines, so that arm would push it past the
1000-line module limit.

## Architecture

A pure move. The moved items keep their names, signatures and doc comments:

- `emit_default_for_field`
- `compile_struct_field_init`
- `FieldInitInfo`
- `initialize_struct_fields`

Callers in `compile_setup.rs` change their path from
`crate::compile_struct::` to `crate::compile_struct_init::`.

## Prefactoring

This change is itself the prefactoring. No further reshaping is needed.

## Design doc reference

`specs/design/structure-codegen-memory-layout.md`

## File map

| File | Change |
| ---- | ------ |
| `compiler/codegen/src/compile_struct_init.rs` | New: the moved initialization code |
| `compiler/codegen/src/compile_struct.rs` | Remove the moved code and unused imports |
| `compiler/codegen/src/compile_setup.rs` | Update call paths |
| `compiler/codegen/src/lib.rs` | Register the module |

## Tasks

- [ ] Move the four items into `compile_struct_init.rs`
- [ ] Update callers and imports
- [ ] Existing tests pass unchanged
- [ ] `git rm` this plan
- [ ] `cd compiler && just` passes
