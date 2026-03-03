# Complete Numeric Standard Library Functions

## Goal

Make all numeric standard library functions (ABS, MIN, MAX, LIMIT, SEL, EXPT, SQRT)
fully supported across all IEC 61131-3 type variants. Update documentation to
accurately reflect support status.

## Current State

Functions have partial implementations covering I32, F32, and F64 variants.
Missing: I64 (for LINT), U32 (for UDINT comparison), U64 (for ULINT comparison).

SQRT is already complete (REAL/LREAL only). SEL is documented as polymorphic ANY
and already works for W32 types.

## What Already Works (doc-only changes)

These types route through existing I32 signed handlers and produce correct results:

| Function | Types that already work via I32 |
|----------|---------------------------------|
| ABS      | SINT, INT (signed, fit in i32)  |
| MIN      | SINT, INT, USINT, UINT          |
| MAX      | SINT, INT, USINT, UINT          |
| LIMIT    | SINT, INT, USINT, UINT          |

USINT (0-255) and UINT (0-65535) are correct with signed comparison because
their values always fall in i32's positive range.

## New Opcodes Needed

### I64 signed variants (for LINT)

| Opcode     | Function | Behavior |
|------------|----------|----------|
| EXPT_I64   | EXPT     | a.wrapping_pow(b as u64), trap on b < 0 |
| ABS_I64    | ABS      | a.wrapping_abs() |
| MIN_I64    | MIN      | a.min(b) signed |
| MAX_I64    | MAX      | a.max(b) signed |
| LIMIT_I64  | LIMIT    | in_val.clamp(mn, mx) signed |
| SEL_I64    | SEL      | if g==0 { in0 } else { in1 } |

### U32 unsigned variants (for UDINT)

| Opcode     | Function | Behavior |
|------------|----------|----------|
| MIN_U32    | MIN      | (a as u32).min(b as u32) |
| MAX_U32    | MAX      | (a as u32).max(b as u32) |
| LIMIT_U32  | LIMIT    | (in_val as u32).clamp(mn as u32, mx as u32) |

### U64 unsigned variants (for ULINT)

| Opcode     | Function | Behavior |
|------------|----------|----------|
| MIN_U64    | MIN      | (a as u64).min(b as u64) |
| MAX_U64    | MAX      | (a as u64).max(b as u64) |
| LIMIT_U64  | LIMIT    | (in_val as u64).clamp(mn as u64, mx as u64) |
| SEL_U64    | SEL      | if g==0 { in0 } else { in1 } |

Total: 16 new opcodes.

Note: SEL does not need U32 because SEL_I32 works correctly for UDINT (no
comparison, just selection). SEL does need a U64/I64 variant because ULINT/LINT
are 64-bit and the current I32 handler can't hold 64-bit values. We use SEL_I64
for both signed and unsigned 64-bit since the operation is identical.

ABS, EXPT: No unsigned variants needed (ABS is identity for unsigned; EXPT is
not defined for unsigned types in the standard).

## Changes Per Layer

### container/src/opcode.rs
- Add 16 new `pub const` entries in the `builtin` module (0x0360-0x036F)
- Extend `arg_count()` match arms

### codegen/src/compile.rs
- Change `lookup_builtin` signature to accept `(name, op_width, signedness)`
- Add routing for W64 → I64 variants
- Add routing for W32+Unsigned → U32 variants (MIN, MAX, LIMIT only)
- Add routing for W64+Unsigned → U64 variants

### vm/src/builtin.rs
- Add 16 new match arms in `dispatch()`
- I64 handlers use `as_i64()`/`from_i64()`
- U32 handlers cast via `as_i32() as u32` then back
- U64 handlers cast via `as_i64() as u64` then back

### Documentation
- Update individual .rst files: mark all integer variants as "Supported"
- Update functions/index.rst: change ABS/MIN/MAX/LIMIT from
  "Not yet supported" to "Supported"
- SQRT index entry: change from "Not yet supported" to "Supported"
  (already fully implemented)

### Tests
- End-to-end tests for LINT variants of each function
- End-to-end tests for UDINT/ULINT MIN/MAX/LIMIT
- VM-level tests for each new opcode
