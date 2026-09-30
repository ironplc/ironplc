# Zero-Extend a Short Date or Time Result Stored into a Long Target

Issue: #1877

## Goal

A `DATE`, `TIME_OF_DAY` or `DATE_AND_TIME` value read for a 64-bit
operation — assigned to an `LDATE`, `LTIME_OF_DAY` or `LDATE_AND_TIME`, or
used as a 64-bit operand — is zero-extended, as a `UDINT` already is, however
it is written: a variable, an operator expression (`dt + t`), a typed call
(`ADD_DT_TIME(dt, t)`), a conversion (`DT_TO_DATE(dt)`), a user function, or
an array element. A `TIME` stays sign-extended (ADR-0021: a duration is
signed).

## Architecture

`compile_expr(expr, op_type)` leaves a value at `op_type`. A 32-bit value
lives in its 64-bit slot sign-extended, so widening it to 64 bits is a no-op
when it is signed and needs `CONV_U32_TO_I64` when it is unsigned
(`emit_conversion_opcode`). Today only two paths ask for that conversion: a
variable read of a *numeric* type (`numeric_op_type`, which excludes the
temporal types) and the operands of a long typed time routine. Everything
else — a temporal variable, a typed call's result, `DT_TO_DATE`, a user
function's result, `MOVE`, `SEL`, an array element — is pushed at 32 bits and
reaches a 64-bit store sign-extended, so a date after 2038 goes negative.

The widening kind is chosen once, at the entry of `compile_expr`: when the
expression has a concrete type whose operation type is 32-bit unsigned and
the requested `op_type` is 64-bit, the expression compiles at its own type and
is zero-extended. A signed 32-bit value needs nothing, so `TIME → LTIME` is
unchanged. The rule is stated on signedness, not on the temporal types, so a
`UDINT` user-function result assigned to a `LINT` is fixed by the same line.

## Prefactoring

`compile_time_arith::compile_operand` zero-extends a short date operand of a
long form by hand. Once `compile_expr` does it for every expression, that
special case is redundant: `compile_operand` becomes a plain
`compile_expr(operand, op_type)`. This is removed in the fix commit, not
before it, because it is only behaviour-preserving once the new rule exists.
No prefactor is needed before the fix: the change is one guard in one
function.

## Design doc reference

`specs/design/arithmetic-operator-overloads.md` — REQ-AO-codegen-005 covers a
short operand of a long form. Add REQ-AO-codegen-013: a short date or time
*result* of an operator or typed call stored into a long target is widened by
its signedness.

## File map

- `compiler/codegen/src/compile_expr.rs` — the guard at the entry of
  `compile_expr`.
- `compiler/codegen/src/compile_time_arith.rs` — drop the now-redundant
  special case in `compile_operand`.
- `compiler/codegen/tests/it/end_to_end_short_temporal_widening.rs` (new) —
  end-to-end tests, values above 2^31, every type pair and spelling.
- `compiler/codegen/tests/it/main.rs` — register the module.
- `compiler/codegen/tests/it/end_to_end_arithmetic_overloads.rs` — the
  REQ-AO-codegen-013 conformance test.
- `specs/design/arithmetic-operator-overloads.md` — REQ-AO-codegen-013.

## Tasks

- [ ] Reproduce on `main` with an end-to-end test (typed call, operator,
      assignment, conversion, user function, array element).
- [ ] Write the failing end-to-end tests (TDD).
- [ ] Add the guard in `compile_expr`; drop the special case in
      `compile_operand`.
- [ ] Add REQ-AO-codegen-013 and its conformance test.
- [ ] `cd compiler && just`, `cd specs && just`.
- [ ] `git rm` this plan.
