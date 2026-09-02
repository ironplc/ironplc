# Plan: Stop the peephole optimizer removing `x + 0.0` on REAL/LREAL

Fixes [#1589](https://github.com/ironplc/ironplc/issues/1589).

## Goal

`pass_arith_identity` removes `LOAD_CONST_F32 0.0; ADD_F32` (and the F64
twin) as an additive identity. On IEEE 754 floats `x + (+0.0)` is not an
identity: `(-0.0) + (+0.0) = +0.0`, so removing the add leaves `-0.0` in
place and `1.0 / y` flips from `+inf` to `-inf`. Restrict the additive
pattern to the integer widths, keep `x - 0.0` (which *is* an identity,
including for `-0.0`), and pin the behaviour with tests at every level.

## Architecture

The pass stays a table of `LOAD_CONST` opcode → arithmetic opcodes. The
additive table drops its F32/F64 rows so a float `ADD` never matches, and
`is_zero_constant` is narrowed to integer pool entries so the two agree. A
doc comment on the pass records the soundness condition so the next reader
knows the float row was removed on purpose.

## Prefactoring

None needed. The fix removes two table rows and two match arms; there is no
shape change that would make it smaller.

## Design doc reference

None. There is no design document for the codegen optimizer; its invariants
live as doc comments in `compiler/codegen/src/optimize/`, which is where the
soundness condition lands.

## File map

| File | Change |
|------|--------|
| `compiler/codegen/src/optimize/pass_arith_identity.rs` | Drop F32/F64 from the additive table; document why |
| `compiler/codegen/src/optimize/mod.rs` | Pass summary names the integer-only additive rule |
| `compiler/codegen/src/optimize/tests.rs` | Invert the F32/F64 `ADD` tests; add `SUB` F32/F64 tests |
| `compiler/codegen/tests/it/compile_arith_identity.rs` | **New** — bytecode-level: int `+ 0` removed, float `+ 0.0` kept, float `- 0.0` removed |
| `compiler/codegen/tests/it/end_to_end_arith_identity.rs` | **New** — REAL/LREAL `-0.0 + 0.0` yields `+0.0` (checked via `1.0 / y = +inf`); DINT/LINT `+ 0` still correct |
| `compiler/codegen/tests/it/main.rs` | Register the two new modules |

## Tasks

- [ ] Restrict additive identity to `LOAD_CONST_I32`/`LOAD_CONST_I64`
- [ ] Invert the unit tests that enshrined the wrong behaviour
- [ ] Add bytecode-level and end-to-end tests
- [ ] `cd compiler && just`
- [ ] Delete this plan
