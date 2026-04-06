# Fix STRING Struct Field Assignment (P9999)

## Context

When assigning to a STRING field of a struct (e.g., `MAKE_DATA.NAME := n`), codegen
emits P9999 "Not implemented". The root cause is `resolve_struct_field_access()` in
`compile_struct.rs:237` which calls `resolve_field_op_type()` — this returns `None` for
`IntermediateType::String`, causing the error. Reading STRING struct fields has the same
issue.

The ARRAY element case (`MAKE_DATA.VALUES[0] := t`) in the user's report actually works
already for primitive element types — it's only blocked because the STRING assignment
on the preceding line fails first, stopping compilation.

## Root Cause

`resolve_field_op_type()` (`compile_struct.rs:75-109`) maps field types to VM op-types.
STRING is a composite type (multiple slots: header + char data), so it correctly returns
`None`. But the two callers in the structured-variable code paths don't have a fallback
for STRING — they just emit P9999.

Affected code paths:
1. **Write**: `compile_stmt.rs:90-101` — calls `resolve_struct_field_access` → fails for STRING
2. **Read**: `compile_expr.rs:513-519` — calls `resolve_struct_field_access` → fails for STRING

## Fix

STRING fields in structs use the data region, just like standalone STRING variables.
The absolute byte offset is computable at compile time:
`struct_info.data_offset + field_slot_offset * 8`. We can pass this to the existing
`emit_str_store_var(byte_offset)` / `emit_str_load_var(byte_offset)` instructions.

### Step 1: Add STRING branch in struct field WRITE path

**File**: `compiler/codegen/src/compile_stmt.rs` (lines 89-101)

Before calling `resolve_struct_field_access`, call `walk_struct_chain` to inspect the
field type. If it's `IntermediateType::String`, compute the absolute byte offset and
emit `STR_STORE_VAR`:

```rust
if let Variable::Symbolic(SymbolicVariableKind::Structured(structured)) =
    &assignment.target
{
    let (root_name, slot_offset, field_type) =
        crate::compile_struct::walk_struct_chain(ctx, &structured.record, &structured.field, 0)?;

    if matches!(&field_type, IntermediateType::String { .. }) {
        let struct_info = ctx.struct_vars.get(&root_name).ok_or_else(|| {
            Diagnostic::problem(
                Problem::NotImplemented,
                Label::span(structured.span(), format!("Variable '{}' is not a structure", root_name)),
            )
        })?;
        let byte_offset = struct_info.data_offset + slot_offset.raw() * 8;
        compile_expr(emitter, ctx, &assignment.value, DEFAULT_OP_TYPE)?;
        emitter.emit_str_store_var(byte_offset);
        return Ok(());
    }

    // Existing non-STRING path (unchanged)
    let (var_index, desc_index, slot_offset, op_type, field_type) =
        crate::compile_struct::resolve_struct_field_access(ctx, structured)?;
    compile_expr(emitter, ctx, &assignment.value, op_type)?;
    crate::compile_struct::emit_truncation_for_field(emitter, &field_type);
    let idx_const = ctx.add_i32_constant(slot_offset.raw() as i32);
    emitter.emit_load_const_i32(idx_const);
    emitter.emit_store_array(var_index, desc_index);
    return Ok(());
}
```

Reuse: `walk_struct_chain` (compile_struct.rs:263), `emit_str_store_var` (emit.rs:350)

### Step 2: Add STRING branch in struct field READ path

**File**: `compiler/codegen/src/compile_expr.rs` (lines 513-519)

Same pattern — check field type before calling `resolve_struct_field_access`:

```rust
Variable::Symbolic(SymbolicVariableKind::Structured(structured)) => {
    let (root_name, slot_offset, field_type) =
        crate::compile_struct::walk_struct_chain(ctx, &structured.record, &structured.field, 0)?;

    if matches!(&field_type, IntermediateType::String { .. }) {
        let struct_info = ctx.struct_vars.get(&root_name).ok_or_else(|| {
            Diagnostic::problem(
                Problem::NotImplemented,
                Label::span(structured.span(), format!("Variable '{}' is not a structure", root_name)),
            )
        })?;
        let byte_offset = struct_info.data_offset + slot_offset.raw() * 8;
        ctx.num_temp_bufs += 1;
        emitter.emit_str_load_var(byte_offset);
        return Ok(());
    }

    // Existing non-STRING path (unchanged)
    let (var_index, desc_index, slot_offset, _op_type, _field_type) =
        crate::compile_struct::resolve_struct_field_access(ctx, structured)?;
    let idx_const = ctx.add_i32_constant(slot_offset.raw() as i32);
    emitter.emit_load_const_i32(idx_const);
    emitter.emit_load_array(var_index, desc_index);
    Ok(())
}
```

Key: `ctx.num_temp_bufs += 1` is required (matches pattern at compile_expr.rs:528).

Reuse: `walk_struct_chain` (compile_struct.rs:263), `emit_str_load_var` (emit.rs:359)

### Step 3: Add end-to-end tests

**File**: `compiler/codegen/tests/end_to_end_struct.rs`

Add 3 tests using existing `read_string` helper and `parse_and_run`:

1. **`end_to_end_when_struct_string_field_write_then_value_stored`**
   - Struct with STRING[10] field, write a literal, verify data region.

2. **`end_to_end_when_struct_string_field_read_then_correct_value`**
   - Write to STRING field, read back into a STRING variable, verify both.

3. **`end_to_end_when_function_return_struct_with_string_field_then_correct`**
   - Function returning struct with STRING and ARRAY fields (reproduces the original bug report).
   - Assigns to TYP (BYTE), NAME (STRING), VALUES[0] (ARRAY element) inside the function.
   - Caller assigns returned struct, verifies all fields.

## Files Modified

| File | Change |
|------|--------|
| `compiler/codegen/src/compile_stmt.rs` | Insert STRING branch before existing struct field write (~12 lines) |
| `compiler/codegen/src/compile_expr.rs` | Insert STRING branch before existing struct field read (~10 lines) |
| `compiler/codegen/tests/end_to_end_struct.rs` | Add 3 test functions (~90 lines) |

No new functions, modules, or abstractions needed. Only adds branches to existing code paths.

## Verification

1. `cd compiler && cargo test -p ironplc_codegen end_to_end_struct` — new tests pass
2. `cd compiler && just` — full CI pipeline passes (compile + coverage + lint)
