# Plan: Report a comparison whose operands have no common type

Issue: [#1931](https://github.com/ironplc/ironplc/issues/1931)
Builds on: [#1933](https://github.com/ironplc/ironplc/pull/1933)

## Goal

A comparison (`=`, `<>`, `<`, `<=`, `>`, `>=` and the function forms `EQ`,
`NE`, `LT`, `LE`, `GT`, `GE`) of two operand types neither of which widens
to the other (`DINT` and `UDINT`, `DINT` and `REAL`) is reported by the
analyzer, so `check` reports it and codegen no longer compiles it at the left
operand's type with a wrong answer.

## Rule

IEC 61131-3 declares the comparison functions over `ANY_ELEMENTARY` with all
inputs of the same type (the comparison functions table). The project's implicit conversions
(ADR-0028, ADR-0029, ADR-0031) relax "the same type" to "one widens to the
other", which is what `comparison_operand_type` (#1933) answers. A pair with
no operand type is reported as P4049 (Operator is not defined for the operand
types), the code the arithmetic operators report for the same pair (`i + r`),
so a comparison and an arithmetic expression accept exactly the same operand
pairs. No new problem code.

- Dialect leniency comes through the relation itself: the flag-gated
  cross-family widening and conversion (`--allow-cross-family-widening`,
  `--allow-cross-family-conversion`) and `--allow-int-literal-to-bit-string`
  already make the pair acceptable, so the CODESYS and TwinCAT dialects accept
  `BYTE` and `INT`, `UDINT` and `DWORD`, a `WORD` and `0`.
- Untyped literals keep adapting to the other operand's type within their
  category (`ud > 3`, `r < 1`). A literal of another category (`d < 1.5`) is
  reported as it is for `d + 1.5`; today it fails in codegen with P9999.
- Operands the relation cannot judge (subranges, enumerations, structures,
  `NULL`) are skipped, as the other operand rules skip them. A `STRING` and
  `WSTRING` pair is left to P4034 (`rule_string_encoding_compat`).

## Architecture

- New rule module `analyzer/src/rule_comparison_operand_type.rs` (the
  existing `rule_operator_operand_type_check.rs` is 805 lines): visits
  `ExprKind::Compare` for the six comparisons and `ExprKind::Function` for the
  six function forms with two positional inputs, and reports P4049 at the
  expression (function name) naming the operator and both types.
- Registered in `stages.rs` next to `rule_operator_operand_type_check`.
- Codegen keeps its fallback for pairs the analyzer skips; its comment is
  updated.

## Prefactoring

None needed: `comparison_operand_type` and `operand_type_name` already give
the rule everything it needs.

## Docs and design

- `specs/design/comparison-operand-type.md`: new `REQ-CMP-analyzer-007..`
  for the diagnostic, Out of scope updated.
- `docs/reference/language/structured-text/comparison-operators.rst`: operand
  types section.
- `docs/reference/compiler/problems/P4049.rst`: comparison example.

## Tasks

- [ ] Tests first (rule tests, spec tests), seen failing
- [ ] Rule module, registration
- [ ] Design doc, docs, P4049 page
- [ ] `ironplcc check` over repo `.st` files and docs examples, before and after
- [ ] `cd compiler && just`, `cd specs && just`, docs build
