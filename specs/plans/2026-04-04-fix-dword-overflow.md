# Fix DWORD constant overflow (P2026) when literal on left side of comparison

## Goal

Fix P2026 (ConstantOverflow) false positive when a large unsigned integer literal
(e.g., 4294967292) appears on the left side of a comparison with a DWORD variable.

## Architecture

The codegen comparison handler at `compile.rs:3119` derives the operand type from
the left operand's resolved type. When the left operand is an untyped integer
literal, its resolved type is `ANY_INT` (generic), which `resolve_type_name` maps
to `DINT` (signed 32-bit). Values above `i32::MAX` then fail the `i32::try_from`
overflow check even though they fit in the unsigned target type.

The fix adds a helper that distinguishes concrete from generic resolved types, and
updates the comparison handler to prefer a concrete type from either operand before
falling back to the generic default.

## File Map

| File | Change |
|------|--------|
| `compiler/codegen/src/compile.rs` | Add `concrete_op_type_from_expr` helper; update comparison operand type resolution |
| `compiler/codegen/tests/end_to_end_bitstring.rs` | Add 3 end-to-end tests for large DWORD literals in comparisons and initializers |

## Tasks

- [ ] Add `concrete_op_type_from_expr` helper after `op_type_from_expr` (line ~2104)
- [ ] Update comparison operand type resolution (line ~3117-3119) to prefer concrete types
- [ ] Add end-to-end tests for DWORD large literal on left/right of comparison and as initializer
- [ ] Run full CI pipeline and verify all checks pass
