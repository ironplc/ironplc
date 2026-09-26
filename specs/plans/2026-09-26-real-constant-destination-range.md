# Report real constants outside the range of the REAL they are stored into

Issue: #1785

## Goal

`b : REAL := 1.0E300` and `b := 1.0E30 * 1.0E30` compile with no diagnostic
and store infinity at runtime. Report a real constant that cannot be
represented by the `REAL` it is stored into, in both assignments and `VAR`
initializers.

## Architecture

`rule_constant_range` already answers "does this constant fit the type it is
stored into" for integers (P2026 `ConstantOverflow`), pushing the destination
type down through operators, into comparisons, and into `VAR` initializers.
`rule_real_literal_range` (P2040) answers a different question: does a real
literal fit its *own* type (`LREAL`, or `REAL` for `REAL#`). That mirrors the
integer split between `INT#40000` (own type) and `x : INT := 100000`
(destination).

So the destination check extends P2026 to reals rather than adding a code:
`check_constant` also accepts a `RealLiteral` when the destination is a
32-bit `REAL`, and reports it when `value as f32` is not finite. Constant
folding runs before the rules, so `1.0E30 * 1.0E30` arrives as one literal.
A value that is not finite in `f64` is left to P2040, which already reports
it and has no meaningful value to print.

## Prefactoring

`report_out_of_range` takes an `(i128, i128)` range. Generalise it to take the
minimum and maximum as anything `Display`, so a real range reports through the
same function. Behaviour-preserving.

## File map

- `compiler/analyzer/src/rule_constant_range.rs` — prefactor + real check + tests
- `docs/reference/compiler/problems/P2026.rst` — document the real case

## Tasks

- [ ] Prefactor `report_out_of_range`
- [ ] Check real constants against a `REAL` destination, with tests
- [ ] Update P2026 docs
- [ ] Remove this plan; run `cd compiler && just`
