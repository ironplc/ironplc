# Prefactor: one emission path for STRING elements of struct array fields

## Goal

Give the STRING-element access `ResolvedAccess::StructFieldStringArrayElement`
a single emission path, with no change in behaviour or emitted bytecode.

This is the second of two prefactors for #1382 (STRING fields of
array-of-struct elements). Today the "compute the array base into the scratch
variable, then push the flat index" sequence is written out twice: in the
read arm of `compile_expr.rs` and in the write arm of `compile_stmt.rs`. The
#1382 fix resolves STRING leaves of array-of-struct elements to this same
access, so each difference it needs would otherwise be made in both places.

## Architecture

- Replace the variant's named fields with a payload struct,
  `StructStringElement`, in `compile_array.rs`.
- Give it `emit_load` and `emit_store`, which share a private
  `emit_base_and_index`.
- The read and write arms each become a single call. The write arm still
  compiles the right-hand side first, because the store consumes it.

## Prefactoring

This change is itself the prefactoring. No further reshaping is needed.

## Design doc reference

`specs/design/structure-codegen-memory-layout.md`

## File map

| File | Change |
| ---- | ------ |
| `compiler/codegen/src/compile_array.rs` | `StructStringElement` and its emit methods; the variant wraps it |
| `compiler/codegen/src/compile_expr.rs` | Read arm calls `emit_load` |
| `compiler/codegen/src/compile_stmt.rs` | Write arm calls `emit_store` |

## Tasks

- [ ] Introduce `StructStringElement` and its emit methods
- [ ] Replace both arms with calls
- [ ] Existing tests pass unchanged
- [ ] `git rm` this plan
- [ ] `cd compiler && just` passes
