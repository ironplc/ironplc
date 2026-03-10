# Math/Trig Standard Library Functions

## Goal

Add all IEC 61131-3 numeric and trigonometric standard library functions:
LN, LOG, EXP, SIN, COS, TAN, ASIN, ACOS, ATAN.

All are 1-argument REAL/LREAL functions following the existing SQRT pattern.

## Current State

SQRT is fully implemented across all layers (opcodes, VM dispatch, codegen
routing, analyzer signatures, docs, tests). The 9 new functions are identical
in shape — each takes one ANY_REAL input and returns ANY_REAL.

## Changes Per Layer

### container/src/opcode.rs
- Add 18 new `pub const` entries in the `builtin` module (0x036C–0x037D)
- Each function gets F32 and F64 variants
- Extend `arg_count()` to return 1 for all 18 new IDs

### codegen/src/compile.rs
- Add 9 new entries in `lookup_builtin()`, each routing F32→`*_F32`, F64→`*_F64`
- W32/W64 return None (integer types not applicable)

### vm/src/builtin.rs
- Add 18 new match arms in `dispatch()`
- Each calls the corresponding Rust method on the popped f32/f64 value:
  - LN: `a.ln()`
  - LOG: `a.log10()`
  - EXP: `a.exp()`
  - SIN: `a.sin()`
  - COS: `a.cos()`
  - TAN: `a.tan()`
  - ASIN: `a.asin()`
  - ACOS: `a.acos()`
  - ATAN: `a.atan()`

### analyzer/src/intermediates/stdlib_function.rs
- Add 9 `FunctionSignature::stdlib` entries in `get_numeric_functions()`
- All use `ANY_REAL` for input and return type, matching SQRT

### Documentation
- Update 9 individual .rst files from "Not yet supported" to "Supported"
- Update functions/index.rst entries for all 9 functions

### Tests
- VM-level tests for each new F32/F64 opcode
- End-to-end tests for REAL and LREAL variants of each function

## Opcode Assignments

| Opcode   | ID     | Function | Behavior |
|----------|--------|----------|----------|
| LN_F32   | 0x036C | LN       | f32::ln() |
| LN_F64   | 0x036D | LN       | f64::ln() |
| LOG_F32  | 0x036E | LOG      | f32::log10() |
| LOG_F64  | 0x036F | LOG      | f64::log10() |
| EXP_F32  | 0x0370 | EXP      | f32::exp() |
| EXP_F64  | 0x0371 | EXP      | f64::exp() |
| SIN_F32  | 0x0372 | SIN      | f32::sin() |
| SIN_F64  | 0x0373 | SIN      | f64::sin() |
| COS_F32  | 0x0374 | COS      | f32::cos() |
| COS_F64  | 0x0375 | COS      | f64::cos() |
| TAN_F32  | 0x0376 | TAN      | f32::tan() |
| TAN_F64  | 0x0377 | TAN      | f64::tan() |
| ASIN_F32 | 0x0378 | ASIN     | f32::asin() |
| ASIN_F64 | 0x0379 | ASIN     | f64::asin() |
| ACOS_F32 | 0x037A | ACOS     | f32::acos() |
| ACOS_F64 | 0x037B | ACOS     | f64::acos() |
| ATAN_F32 | 0x037C | ATAN     | f32::atan() |
| ATAN_F64 | 0x037D | ATAN     | f64::atan() |
