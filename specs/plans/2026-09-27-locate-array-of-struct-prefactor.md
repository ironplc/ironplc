# Prefactor: extract `locate_array_of_struct`

## Goal

Split `resolve_struct_array_element_field` in
`compiler/codegen/src/compile_array_struct.rs` into two steps: locating the
array of structures that `<array>[i].field` indexes, and building the access
from it. No behaviour change; the bytecode is byte-identical.

Part 2 of the split of #1792 (#1382). The #1382 fix needs the location step
from a second place: `string_width.rs` works out the encoding of a STRING
field `a[i].s`, and needs the element type but not an access. Without this
prefactor, the fix would copy the subscript walk and base lookup.

## Architecture

- `locate_array_of_struct` returns a `LocatedArrayOfStruct` holding:
  - the region's variable index and descriptor
  - the slot offset of element 0
  - the element type and dimensions
  - the element subscripts
  - a span
- `resolve_struct_array_element_field` calls it, then
  `struct_array_element_field` as before.

## Prefactoring

This change is itself the prefactoring.

## Design doc reference

`specs/design/structure-codegen-memory-layout.md` §2.3

## File map

| File | Change |
| ---- | ------ |
| `compiler/codegen/src/compile_array_struct.rs` | Extract `locate_array_of_struct` |

## Tasks

- [ ] Extract; resolver calls it
- [ ] Bytecode byte-identical for accesses through a structure field and a top-level array
- [ ] `git rm` this plan; `cd compiler && just` passes
