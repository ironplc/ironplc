# Plan: Implement ATAN2 Standard Function

## Goal

Add the IEC 61131-3 `ATAN2` standard function to IronPLC. `ATAN2(Y, X)` computes the two-argument arctangent — the angle in radians from the positive X-axis to the point (X, Y). Unlike the single-argument `ATAN`, `ATAN2` takes two `ANY_REAL` parameters, following the same pattern as `EXPT`.

## Architecture

ATAN2 follows the existing two-argument builtin function pattern (EXPT):

- **Analyzer**: Declare function signature with two `ANY_REAL` inputs
- **Container**: Define `ATAN2_F32` and `ATAN2_F64` opcodes (no integer variants — arctangent is only meaningful for floats)
- **Codegen**: Map "ATAN2" to the correct opcode based on `OpWidth`
- **VM**: Execute using Rust's `f32::atan2` / `f64::atan2`

## File Map

| File | Change |
|------|--------|
| `compiler/analyzer/src/intermediates/stdlib_function.rs` | Add `ATAN2` signature after `ATAN` |
| `compiler/container/src/opcode.rs` | Add `ATAN2_F32` (0x039B), `ATAN2_F64` (0x039C); update `arg_count()` |
| `compiler/codegen/src/compile.rs` | Add `"ATAN2"` arm in `lookup_builtin()` |
| `compiler/vm/src/builtin.rs` | Add dispatch arms for `ATAN2_F32` and `ATAN2_F64` |
| `compiler/codegen/tests/end_to_end_atan2.rs` | New file: 4 end-to-end tests (REAL and LREAL) |

## Tasks

- [x] Write plan
- [ ] Declare ATAN2 in analyzer (`stdlib_function.rs`)
- [ ] Define ATAN2 opcodes (`opcode.rs`)
- [ ] Add ATAN2 to codegen (`compile.rs`)
- [ ] Implement ATAN2 VM execution (`builtin.rs`)
- [ ] Create end-to-end tests (`end_to_end_atan2.rs`)
- [ ] Run full CI pipeline (`cd compiler && just`)
