# String Comparison Operators

## Problem

IEC 61131-3 requires comparison operators (`=`, `<>`, `<`, `>`, `<=`, `>=`) to
work on STRING types. The codegen previously returned `None` from
`resolve_type_name()` for STRING/WSTRING, causing string comparisons to fall
through to integer comparison logic. This blocked patterns like
`MID(str, 1, i) = ch` used in OSCAT functions (CHARCODE, FINDB, TRIM1, etc.).

## Approach

Use the existing `BUILTIN` opcode (0xC4) with a new builtin function ID
`CMP_STR` (0x03A2). This performs a three-way lexicographic comparison
(returning -1/0/+1 as i32), then reuses existing integer comparison opcodes
(`EQ_I32`, `LT_I32`, etc.) against zero to derive the boolean result.

This avoids adding any new top-level opcodes.

## Changes

- `compiler/container/src/opcode.rs` — Added `builtin::CMP_STR` constant
- `compiler/codegen/src/compile.rs` — Added `expr_is_string()` helper and
  `compile_string_compare()` function; modified `ExprKind::Compare` arm to
  detect string operands
- `compiler/vm/src/vm.rs` — Added `CMP_STR` handler in `BUILTIN` dispatch
- `compiler/codegen/tests/end_to_end_string_compare.rs` — 12 end-to-end tests
