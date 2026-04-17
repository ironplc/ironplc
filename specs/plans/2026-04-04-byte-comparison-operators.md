# Plan: Add BYTE Comparison Operator Tests and Fix Stdlib Signatures

## Goal

Ensure BYTE comparison operators (>=, <=, >, <, =, <>) work correctly end-to-end and that stdlib comparison function signatures match IEC 61131-3 Table 33. This unblocks 7 OSCAT character classification functions (IS_ALNUM, IS_ALPHA, IS_LOWER, IS_UPPER, ISC_ALPHA, ISC_LOWER, ISC_UPPER).

## Architecture

BYTE comparisons already compile and execute correctly via operator syntax — the codegen maps BYTE to `(OpWidth::W32, Signedness::Unsigned)` and dispatches to unsigned comparison opcodes. The gaps are:

1. **Stdlib signatures**: Comparison functions (GT, GE, EQ, LE, LT, NE) use `ANY_NUM` parameters, formally excluding BYTE. IEC 61131-3 Table 33 specifies `ANY_ELEMENTARY`. Widening the signatures fixes standards compliance.
2. **Missing tests**: No end-to-end tests verify BYTE comparison correctness.
3. **Missing repro file**: The benchmark file referenced in the issue doesn't exist.

STRING comparison is out of scope — it requires new VM opcodes for string-specific comparison semantics.

## File Map

| File | Change |
|------|--------|
| `compiler/analyzer/src/intermediates/stdlib_function.rs` | Change comparison function params from `ANY_NUM` to `ANY_ELEMENTARY` |
| `compiler/codegen/tests/end_to_end_cmp.rs` | Add BYTE comparison end-to-end tests |
| `benchmarks/minimal_repros/42_string_comparison_operators.st` | Create repro file from issue description |

## Tasks

- [x] Write plan
- [ ] Update comparison function signatures from `ANY_NUM` to `ANY_ELEMENTARY` in `stdlib_function.rs`
- [ ] Add end-to-end tests for BYTE comparisons (GE, LE, GT, LT, combined range check)
- [ ] Create benchmark repro file `42_string_comparison_operators.st`
- [ ] Run full CI pipeline (`cd compiler && just`)
