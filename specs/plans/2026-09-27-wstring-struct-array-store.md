# Fix: store into a WSTRING element of a structure's array field

## Goal

`h.names[2] := "wide"`, where `names : ARRAY[..] OF WSTRING[..]` is a
structure field, traps V9014 (encoding mismatch). The store produces the
right-hand side at the default narrow encoding instead of the element's.
Make it produce the value at the element's encoding, as every other string
destination does (ADR-0034).

Part 3 of the split of #1792 (#1382). The array-of-struct STRING path added
there shares this store, so it needs the fix too.

## Architecture

- `StructStringElement` (`compile_array.rs`) carries the element's
  `char_width`, which the resolver sets from the array's element type.
- The store arm in `compile_stmt.rs` uses
  `compile_string_value(.., element.char_width)` instead of
  `compile_expr(.., DEFAULT_OP_TYPE)`.

## Prefactoring

None needed: `StructStringElement` (from #1778) is already the single place
that describes the access.

## Design doc reference

ADR-0034 (operand typing and encoding tags).

## File map

| File | Change |
| ---- | ------ |
| `compiler/codegen/src/compile_array.rs` | `char_width` on `StructStringElement` |
| `compiler/codegen/src/compile_stmt.rs` | Store uses `compile_string_value` |
| `compiler/codegen/tests/it/end_to_end_wstring.rs` | Regression test |

## Tasks

- [ ] Carry `char_width`; fix the store
- [ ] Regression test (fails on `main` with V9014)
- [ ] `git rm` this plan; `cd compiler && just` passes
