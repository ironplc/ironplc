# Prefactor: initialize structure variables through one function

## Goal

Replace the three copies of structure-variable initialization in
`compiler/codegen/src/compile_setup.rs` with one
`initialize_struct_variable` in `compile_struct_init.rs`. No behaviour change;
the bytecode is byte-identical.

Part 1 of the split of #1792 (STRING fields of array-of-struct elements,
#1382). The #1382 fix adds a STRING header initialization step to every
structure variable. Without this prefactor, that step would be repeated at
all three sites.

## Architecture

The three sites handle globals, initialized locals, and function struct
returns. Each one:

1. stores the data offset into the variable slot,
2. maps the fields to `FieldInitInfo`,
3. calls `initialize_struct_fields`.

The new function does all three steps. Each site becomes a single call.

## Prefactoring

This change is itself the prefactoring.

## Design doc reference

`specs/design/structure-codegen-memory-layout.md`

## File map

| File | Change |
| ---- | ------ |
| `compiler/codegen/src/compile_struct_init.rs` | New `initialize_struct_variable` |
| `compiler/codegen/src/compile_setup.rs` | Three sites call it |

## Tasks

- [ ] Add `initialize_struct_variable`; replace the three sites
- [ ] Bytecode byte-identical for a program covering all three sites
- [ ] `git rm` this plan; `cd compiler && just` passes
