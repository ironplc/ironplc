# Check every CASE label against the selector's range

Issue: #1539

## Goal

A `CASE` label outside the range of the selector's type is reported by
`check` as P2026, whatever radix the label was written in. Today a decimal
label is checked by `rule_constant_range`, but a radix label (`16#FFFFFFFF:`)
and the bounds of a subrange label (`100..300:`) are not. The backend then
narrows a radix label as a bit pattern, so `16#FFFFFFFF` against a `DINT`
selector silently becomes `-1` and matches.

The backend also narrows a decimal label as a *signed* value whatever the
selector's signedness, so a valid label such as `4294967295:` against a
`UDINT` selector fails to compile with P2026. The same value spelled
`16#FFFFFFFF` compiles. Both spellings should behave the same.

## Architecture

- **Analyzer** (`rule_constant_range`): `check_case` checks every label
  that carries an integer value against the selector's range: a decimal
  label, a radix label, and both literal bounds of a subrange label. A bound
  that names a constant is not resolved here (the backend does not support it
  yet either). A label whose magnitude does not fit `i128` is reported the
  way `check_literal` reports one, rather than skipped.
- **Codegen** (`compile_stmt`): a label is narrowed by its *value* to the
  selector's operation type (width and signedness), the same way for every
  radix. `CaseLabelValue` collapses into one by-value representation. A value
  that does not fit stays a P2026 problem rather than an internal error,
  because the analyzer cannot type every selector (an untyped literal
  selector falls back to `DINT` in the backend).

## Prefactoring

Extract from `check_literal` the part that judges a sign and magnitude
against a range (`check_magnitude`), so `check_case` can reuse it for every
label kind without copying the report formatting. Behaviour-preserving.

## Design doc reference

None; the rule's module documentation and `P2026.rst` describe the check.

## File map

- `compiler/analyzer/src/rule_constant_range.rs` — prefactor and label checks
- `compiler/analyzer/src/rule_constant_range/tests.rs` — label tests
- `compiler/codegen/src/compile_stmt.rs` — by-value narrowing
- `compiler/codegen/tests/it/end_to_end_case.rs` — unsigned selector tests
- `compiler/plc2plc/src/tests/case.rs` — tighten a near-vacuous assertion
- `docs/reference/compiler/problems/P2026.rst` — radix and subrange labels
- `docs/reference/language/structured-text/case.rst` — label kinds, P2026

## Tasks

- [ ] Prefactor `check_literal` into `check_magnitude`
- [ ] Failing analyzer tests: hex/binary/octal labels, subrange bounds,
      unsigned and 64-bit selectors, subrange-typed selector
- [ ] Check radix labels and subrange bounds in `check_case`
- [ ] Failing codegen tests: decimal and hex `4294967295` against `UDINT`
- [ ] Narrow labels by value in codegen
- [ ] Docs: P2026 and CASE reference page; plc2plc assertion
- [ ] Open issues for the gaps not fixed here: radix subrange bounds do
      not parse; duplicate labels are not diagnosed
- [ ] `git rm` this plan; `cd compiler && just`; `cd specs && just`
