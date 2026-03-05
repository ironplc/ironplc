# Type Conversion Standard Library Functions

## Goal

Implement all 90 numeric type conversion functions defined in IEC 61131-3 Section 2.5.1.5.1. These follow the `<SOURCE>_TO_<TARGET>` naming pattern and are already defined in the analyzer's function environment.

## Scope

90 conversion functions across four categories:

| Category | Count | Examples |
|----------|-------|---------|
| Int → Int | 56 | SINT_TO_INT, DINT_TO_LINT, INT_TO_UINT |
| Int → Real | 16 | INT_TO_REAL, UDINT_TO_LREAL |
| Real → Int | 16 | REAL_TO_INT, LREAL_TO_UDINT |
| Real → Real | 2 | REAL_TO_LREAL, LREAL_TO_REAL |

**Out of scope**: Boolean conversions (not in analyzer), string conversions (no string runtime).

## Architecture

### Key Insight: Source and Target Types Differ

Unlike existing stdlib functions (ABS, SIN, etc.) where input and output share the same OpType, conversion functions have different source and target types. This requires a dedicated codegen handler that:

1. Parses the function name to extract source and target type names
2. Compiles the argument with the **source** type's OpType
3. Emits a conversion opcode when source and target cross domain boundaries
4. Emits truncation when the target is sub-32-bit

### Conversion Categories at the VM Level

**No-op conversions (50 functions)**: Same-domain integer conversions where the Slot representation already handles the conversion:
- Signed W32 → W64: sign-extension in `from_i32` provides correct i64 value
- W64 → W32: `as_i32()` truncation at store time provides correct i32 value
- Same-width sign reinterpretation: same bit pattern, different interpretation
- Sub-32-bit targets get TRUNC_I8/U8/I16/U16 from the conversion handler

**Zero-extend conversions (6 functions)**: Unsigned W32 → W64 where sign-extension would corrupt the value (e.g., UDINT 0xFFFFFFFF sign-extends to 0xFFFFFFFFFFFFFFFF instead of 0x00000000FFFFFFFF). Uses 1 new opcode.

**Cross-domain conversions (34 functions)**: Integer ↔ float and float ↔ float conversions that require actual computation. Uses 18 new opcodes.

### New BUILTIN Opcodes (19 total)

Starting at 0x037E (next available after ATAN_F64):

| ID | Name | VM Operation |
|----|------|-------------|
| 0x037E | CONV_I32_TO_F32 | `from_f32(as_i32() as f32)` |
| 0x037F | CONV_I32_TO_F64 | `from_f64(as_i32() as f64)` |
| 0x0380 | CONV_I64_TO_F32 | `from_f32(as_i64() as f32)` |
| 0x0381 | CONV_I64_TO_F64 | `from_f64(as_i64() as f64)` |
| 0x0382 | CONV_U32_TO_F32 | `from_f32((as_i32() as u32) as f32)` |
| 0x0383 | CONV_U32_TO_F64 | `from_f64((as_i32() as u32) as f64)` |
| 0x0384 | CONV_U64_TO_F32 | `from_f32((as_i64() as u64) as f32)` |
| 0x0385 | CONV_U64_TO_F64 | `from_f64((as_i64() as u64) as f64)` |
| 0x0386 | CONV_F32_TO_I32 | `from_i32(as_f32() as i32)` |
| 0x0387 | CONV_F32_TO_I64 | `from_i64(as_f32() as i64)` |
| 0x0388 | CONV_F64_TO_I32 | `from_i32(as_f64() as i32)` |
| 0x0389 | CONV_F64_TO_I64 | `from_i64(as_f64() as i64)` |
| 0x038A | CONV_F32_TO_U32 | `from_i32((as_f32() as u32) as i32)` |
| 0x038B | CONV_F32_TO_U64 | `from_i64((as_f32() as u64) as i64)` |
| 0x038C | CONV_F64_TO_U32 | `from_i32((as_f64() as u32) as i32)` |
| 0x038D | CONV_F64_TO_U64 | `from_i64((as_f64() as u64) as i64)` |
| 0x038E | CONV_F32_TO_F64 | `from_f64(as_f32() as f64)` |
| 0x038F | CONV_F64_TO_F32 | `from_f32(as_f64() as f32)` |
| 0x0390 | CONV_U32_TO_I64 | `from_i64((as_i32() as u32 as u64) as i64)` |

### Codegen Routing

In `compile_function_call`, detect conversion functions by checking if the name contains `_TO_` and both parts resolve to known type names via `resolve_type_name`. Route to `compile_type_conversion` instead of `compile_generic_builtin`.

The handler determines which opcode to emit based on source and target VarTypeInfo:

```
match (source.op_width, source.signedness, target.op_width) {
    // Same OpWidth: no conversion opcode needed
    (W32, _, W32) | (W64, _, W64) => {}

    // W32 Signed → W64: no-op (sign extension correct)
    (W32, Signed, W64) => {}

    // W32 Unsigned → W64: zero-extend
    (W32, Unsigned, W64) => emit CONV_U32_TO_I64

    // W64 → W32: no-op (as_i32 truncates)
    (W64, _, W32) => {}

    // Int → Float: emit CONV_{I,U}{32,64}_TO_F{32,64}
    // Float → Int: emit CONV_F{32,64}_TO_{I,U}{32,64}
    // Float → Float: emit CONV_F{32,64}_TO_F{64,32}
}
```

After the conversion opcode, emit truncation if target storage_bits < native width.
