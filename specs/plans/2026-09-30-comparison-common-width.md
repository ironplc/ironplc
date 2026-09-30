# Plan: Compute a comparison at the common type of its operands

Issue: [#1920](https://github.com/ironplc/ironplc/issues/1920)

## Goal

A comparison (`=`, `<>`, `<`, `<=`, `>`, `>=` and their function forms `EQ`,
`NE`, `LT`, `LE`, `GT`, `GE`) is computed at the type both operands widen to,
whichever side the wider one is on. Today `compile_compare` computes it at the
left operand's type, so `DINT 1 < LINT 4294967297` truncates the `LINT` to 32
bits and gives FALSE, and `DT < LDT#2107-...` does the same. The function form
is worse: `compile_left_fold` computes it at the type of the enclosing
expression (the `BOOL` target), so even `GT(l1, l2)` on two `LINT`s truncates.

## Architecture

- **Analyzer.** A comparison has no overload of its own to resolve, but the
  question "which of these two operand types does the other widen to" is the
  one the numeric arithmetic overload already answers with
  `are_types_compatible` (lossless integer and real widening, bit strings,
  the short-to-long temporal widening, the flag-gated cross-family widening,
  and an untyped literal taking the concrete operand's type). A new pure
  function `comparison_operand_type(left, right, options)` answers it for a
  comparison, in a new module `intermediates/comparison_operand.rs`, and is
  exported for codegen as `resolve_arithmetic_overload` is.
- **Codegen.** A new module `compile_comparison.rs` compiles a comparison of
  two operands: it asks the analyzer for the operand type, compiles each
  operand at its own type and converts it (so an unsigned or date operand is
  zero-extended, a signed one sign-extended, an integer converted to a real),
  then emits the comparison at the common type. When there is no common type
  (a pair the analyzer does not judge, such as `DINT` and `UDINT`), it keeps
  today's choice: the concrete left operand, then the right. Both
  `compile_compare` (operator) and `compile_operator_form` (function form)
  call it, so the two spellings cannot diverge.

## Prefactoring

- Extract from `numeric_overload` the choice of the operand the other is
  acceptable as (`common_operand`, returning which side), so the comparison
  reuses the same rule rather than restating it. No behaviour change.
- Generalise `compile_arith::compile_at` (compile at own type, convert when
  the width differs) to take the operand's own operation type from the type
  name, including the temporal types, as a shared helper. Behaviour for
  arithmetic stays the same because arithmetic only passes numeric types.

## Design doc

New `specs/design/comparison-operand-type.md` with `REQ-CMP-analyzer-*` and
`REQ-CMP-codegen-*`, registered in both crates' `build.rs`.

## File map

- `compiler/analyzer/src/intermediates/common_operand.rs` (new, prefactor)
- `compiler/analyzer/src/intermediates/arithmetic_overload.rs` (prefactor)
- `compiler/analyzer/src/intermediates/comparison_operand.rs` (new)
- `compiler/analyzer/src/lib.rs`, `intermediates/mod.rs`, `build.rs`
- `compiler/codegen/src/compile_comparison.rs` (new)
- `compiler/codegen/src/compile_expr.rs`, `compile_call.rs`, `lib.rs`, `build.rs`
- `compiler/codegen/tests/it/end_to_end_comparison_width.rs` (new)
- `specs/design/comparison-operand-type.md` (new)

## Tasks

- [ ] Prefactor: extract `common_operand` from `numeric_overload`
- [ ] Design doc with requirement IDs
- [ ] Analyzer: unit tests, then `comparison_operand_type`
- [ ] Codegen: end-to-end tests per type family (integer, unsigned, bit
      string, real, time, date and time), operator and function form, operand
      on either side, values that differ only above the narrow width
- [ ] Codegen: `compile_comparison.rs`, used by the operator and function form
- [ ] Open an issue for the comparison pairs the analyzer accepts but that
      have no common type (`DINT` and `UDINT`, `DINT` and `REAL`)
- [ ] `git rm` this plan; `cd compiler && just`; `cd specs && just`
