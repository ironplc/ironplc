# Implementation Plan: Fill In Not-Implemented Bit-Access Paths

**Builds on:** [`specs/design/partial-access-bit-syntax.md`](../design/partial-access-bit-syntax.md) (PR #916)

## Goal

PR #916 introduced support for the IEC 61131-3:2013 partial-access bit syntax
`.%Xn` and, along the way, added bit access on array elements. Three paths
were deferred to a `NotImplemented` diagnostic or silently wrong codegen:

1. Bit write on an LWORD/LINT array element (W64 width).
2. Bit read on a struct field that isn't a simple BYTE/WORD/DWORD/INT/DINT.
3. Bit write on a struct field (any integer width).

This change fills in all three paths so that `x.n` and `x.%Xn` work
uniformly across array elements, struct fields, and plain scalars — for
both 32-bit and 64-bit integer bases, and for both reads and writes.

## Architecture

All three paths reuse the existing read-modify-write template used by
`compile_bit_access_assignment` on scalars:

```
load  current
and   clear_mask          (~(1 << n))
load  rhs
and   1
shl   n
or
truncate-to-element-width
store back
```

Differences per path:

- **Array element** (existing): uses `LOAD_ARRAY` / `STORE_ARRAY` with a
  runtime-computed flat index. For W64 elements, the clear-and-or ops use
  the 64-bit opcodes, and the shifted bit is first widened from W32 to W64
  (the RHS BOOL lands as an i32 1 or 0).
- **Struct field** (new): uses `LOAD_ARRAY` / `STORE_ARRAY` too — a struct
  is stored as a flat slot array — but with a compile-time slot index
  derived from `walk_struct_chain`. The index constant is allocated once
  in the pool and reused for load and store.
- **Struct field read**: the existing read path recurses through
  `compile_variable_read`, which already handles `Structured`. The only
  fix needed was deriving `base_op_type` from the leaf field's
  `IntermediateType` (via `resolve_field_op_type`) instead of falling back
  to `DEFAULT_OP_TYPE`, so the shift-right uses the correct width for
  LWORD/LINT struct fields.

No new AST node, no new opcode, no new analyzer rule.

## File Map

| Action | File |
|--------|------|
| Add `StructuredVariable` to imports | `compiler/codegen/src/compile_expr.rs` |
| Use `resolve_field_op_type` for struct-field bit-read width | `compiler/codegen/src/compile_expr.rs` |
| Dispatch struct-field bit writes before the named-scalar fallback | `compiler/codegen/src/compile_expr.rs` |
| Extend `compile_bit_access_assignment_on_array` to handle W64 elements | `compiler/codegen/src/compile_expr.rs` |
| Add `compile_bit_access_assignment_on_struct_field` helper | `compiler/codegen/src/compile_expr.rs` |
| Examples/e2e tests for each previously-NotImplemented path | `compiler/codegen/tests/end_to_end_bit_access_not_impl.rs` (new) |

## Tasks

- [x] Create failing e2e tests that reproduce each NotImplemented path.
- [x] Implement W64 array-element bit write (no longer errors out).
- [x] Implement struct-field bit write via LOAD_ARRAY/STORE_ARRAY.
- [x] Fix struct-field bit read to use the field's op_width for shift-right.
- [x] Confirm `.%Xn` piggybacks on the same paths (tests cover both spellings).
- [x] `cd compiler && just` passes.

## Verification

The following programs — any of which previously produced P9999
NotImplemented or runtime faults — now compile and execute correctly:

```
arr : ARRAY[0..1] OF LWORD;
arr[0].0  := TRUE;                 (* W64 array-element bit write *)
arr[1].40 := TRUE;

s : MY_STRUCT;                     (* MY_STRUCT.flags : BYTE *)
s.flags.0 := TRUE;                 (* struct-field bit write  *)
r := s.flags.%X2;                  (* struct-field bit read   *)
```

Out-of-scope (intentionally not addressed here):

- Bit access on an array that is nested inside a struct field
  (`s.my_array[0].0`). This is the `array_vars.get(root_name)` miss on
  line ~878 of `compile_expr.rs`. It requires plumbing
  `StructFieldArrayElement` through the bit-access path and is tracked
  separately.
- `.%Bn`, `.%Wn`, `.%Dn`, `.%Ln` byte/word/dword/lword partial access,
  which produces a non-bit view of the underlying data and requires a
  distinct AST node. The design doc for `.%Xn` explicitly defers these.
