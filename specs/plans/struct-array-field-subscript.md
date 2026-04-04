# Plan: Array Subscript on Struct Field Members

## Context

Accessing an array field of a struct variable (e.g., `math.FACTS[x]`) fails
with P9999 "Not implemented" in codegen. The AST for this expression is
`Array(subscripted_variable: Structured(...), subscripts: [...])`, but
`resolve_access()` in `compile_array.rs:154` only handles `Named` and `Deref`
as the base of an array chain, not `Structured`.

This blocks 6 OSCAT functions (FACT, DATE_ADD, DAY_OF_MONTH, DT2_TO_SDT,
DT_TO_SDT, MONTH_BEGIN).

## Approach

### Key Insight

Structs are stored as flat slot arrays in the data region. An array field
within a struct starts at a known `slot_offset`. To access element `[x]` of
that array field, the total slot index is `slot_offset + flat_index(subscripts)`.
We reuse the struct's `var_index` and `desc_index` with `LOAD_ARRAY`/`STORE_ARRAY`
at this combined offset.

### Changes

1. **compile_struct.rs** – Make `walk_struct_chain` `pub(crate)`.

2. **compile_array.rs** – Add `StructFieldArrayElement` variant to
   `ResolvedAccess`. In `resolve_access()`, handle
   `SymbolicVariableKind::Structured` at line 154: walk the struct chain to
   get the field's `slot_offset` and `IntermediateType::Array { dimensions }`,
   convert dimensions to `DimensionInfo`, and return the new variant.

3. **compile.rs** – Add dispatch arms for `StructFieldArrayElement` in both
   the read path (`compile_variable_read`) and write path
   (`compile_statement` assignment). Emit `flat_index + slot_offset`, then
   `LOAD_ARRAY`/`STORE_ARRAY` with the struct's variable/descriptor indices.

4. **Tests** – End-to-end tests for struct array field read, write, variable
   index, and global struct array field access.

5. **Benchmark repro** – `benchmarks/minimal_repros/40_global_array_subscript.st`.

## Files

| File | Change |
|------|--------|
| `compiler/codegen/src/compile_struct.rs` | `pub(crate) fn walk_struct_chain` |
| `compiler/codegen/src/compile_array.rs` | New variant + Structured handling + dimension converter |
| `compiler/codegen/src/compile.rs` | Read/write dispatch arms |
| `compiler/codegen/tests/end_to_end_struct.rs` | End-to-end tests |
| `benchmarks/minimal_repros/40_global_array_subscript.st` | Regression repro |
