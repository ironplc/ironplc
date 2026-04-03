# String Type Conversion Functions

## Summary

Add all W32 numeric↔STRING conversion functions from IEC 61131-3 Section 2.5.1.5.
This includes 20 functions (10 *_TO_STRING + 9 STRING_TO_* + REAL_TO_STRING)
using 5 new BUILTIN func_ids.

## Functions

### *_TO_STRING (11 functions → 3 VM func_ids)

- SINT/INT/DINT_TO_STRING → CONV_I32_TO_STR (signed decimal)
- USINT/UINT/UDINT/BYTE/WORD/DWORD_TO_STRING → CONV_U32_TO_STR (unsigned decimal)
- REAL_TO_STRING → CONV_F32_TO_STR (float decimal)

### STRING_TO_* (9 functions → 2 VM func_ids)

- STRING_TO_SINT/INT/DINT/USINT/UINT/UDINT → CONV_STR_TO_I32 (parse + truncate)
- STRING_TO_REAL → CONV_STR_TO_F32 (parse float)

## Design

- Use BUILTIN func_ids (u16), not new top-level opcodes
- Handle in VM main loop's BUILTIN arm (needs temp_buf/data_region access)
- Codegen: new `parse_string_conversion()` router since `resolve_type_name()`
  returns None for STRING
- Parse failures return 0 / 0.0
- Future: add W64 variants (LINT, ULINT, LWORD, LREAL) with same pattern

## Files Modified

- `compiler/container/src/opcode.rs` — 5 BUILTIN func_id constants
- `compiler/vm/src/vm.rs` — 5 inline handlers in BUILTIN dispatch
- `compiler/analyzer/src/intermediates/stdlib_function.rs` — 20 function signatures
- `compiler/codegen/src/compile.rs` — string conversion routing + compilation
- `compiler/codegen/tests/end_to_end_conv_string.rs` — end-to-end tests
- `docs/reference/standard-library/functions/type-conversions.rst` — mark as supported
