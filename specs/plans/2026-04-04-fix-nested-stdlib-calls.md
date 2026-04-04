# Plan: Fix Nested Stdlib Function Calls in Codegen

## Goal

Enable nested standard library string function calls as arguments to other
string functions (e.g., `FIND(haystack, MID(needle, 1, maxlen))`). Currently
fails with P9999 "Not implemented" in codegen.

## Architecture

`resolve_string_arg()` in `codegen/src/compile.rs` returns a `data_offset`
pointing to a string in the data region. It only handles `Variable` and
`CharacterString` expressions. Any other expression (including nested function
calls) hits a wildcard todo error.

The fix adds a catch-all arm that:
1. Allocates a temporary slot in the data region
2. Emits `STR_INIT` to initialize the header
3. Calls `compile_expr()` to compile the expression (pushes `buf_idx`)
4. Emits `STR_STORE_VAR` to materialize the result into the data region
5. Returns the slot's `data_offset`

No VM changes needed — purely a codegen fix.

## File Map

| File | Change |
|------|--------|
| `compiler/codegen/src/compile.rs` | Add catch-all arm to `resolve_string_arg()` |
| `compiler/codegen/tests/end_to_end_find.rs` | Add nested-call test |
| `benchmarks/minimal_repros/41_nested_function_call_in_function_arg.st` | New repro file |

## Tasks

- [ ] Write plan
- [ ] Add catch-all arm to `resolve_string_arg()`
- [ ] Add end-to-end test for `FIND` with nested `MID`
- [ ] Create benchmark reproduction file
- [ ] Run full CI pipeline
