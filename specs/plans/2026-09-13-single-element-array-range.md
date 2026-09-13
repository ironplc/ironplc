# Plan: Accept single-element array index ranges

Fixes [#1673](https://github.com/ironplc/ironplc/issues/1673).

## Goal

`ARRAY[0..0] OF LWORD` is a one-element array and IEC 61131-3 permits it,
but `ironplcc check` rejects it with P2002 "Subrange declaration minimum
value is not less than the maximum". After this change a single-element
array index range is accepted, while a subrange *type* with a single value
(`INT(5..5)`) keeps being rejected, and an inverted array range
(`ARRAY[1..0]`) keeps being rejected.

## Cause

`rule_decl_subrange_limits` overrides `visit_subrange`, which the visitor
calls for every `Subrange` node in the AST. The DSL reuses the one
`Subrange` struct in three places:

| Where the `Subrange` appears        | Owner node              | Valid when   |
| ----------------------------------- | ----------------------- | ------------ |
| Subrange type, `INT(-10..10)`       | `SubrangeSpecification` | `min < max`  |
| Array dimension, `ARRAY[0..0]`      | `ArraySubranges`        | `min <= max` |
| `CASE` label range, `5..5:`         | `CaseSelectionKind`     | `min <= max` |

The rule applies the first row's `min < max` to all three, which is why a
one-element array and a one-value `CASE` label are both rejected, with a
message that talks about a "subrange declaration".

The array row cannot simply be left to the type-environment builder.
`intermediates::array::validate_array_bounds` already checks `min <= max`
(P2024) but only runs for a `TYPE`-declared array; an array declared inline
in a `VAR` block (the shape in the issue) reaches no other check, so the
rule is the only thing that reports `ARRAY[1..0]` there.

## Architecture

Divide what the rule accepts into two cases, keyed by the node that owns the
range rather than by the bare `Subrange`:

- **Subrange type** (`visit_subrange_specification`): `min < max`, else
  P2002. Unchanged behaviour.
- **Array dimension** (`visit_array_subranges`): `min <= max`, else P2024
  `ArrayDimensionInvalid`. This is the same problem code the type-environment
  builder already reports for a `TYPE`-declared inverted array, so the
  defect gets one code regardless of where the array is declared.

The distinction is one small enum in the rule, `RangeContext`, whose two
variants each know the ordering they accept and the problem they report.
The generic `visit_subrange` override goes away, so the rule stops touching
`CASE` label ranges: `5..5:` is accepted (it is a range of one value). An
inverted `CASE` label range (`10..1:`) stops being reported by this rule;
it was only ever reported with the wrong wording, it is harmless at runtime
(the label never matches) and it wants its own problem code, so that is
opened as a follow-up issue rather than folded in here.

A `TYPE`-declared inverted array is reported twice (once by the
type-environment builder, once by the rule), exactly as a `TYPE`-declared
inverted subrange type is today; that double report predates this change and
is out of scope.

## Prefactoring

`visit_subrange` does two things in one body: it turns the two
`SignedIntegerRef` bounds into `i128` values (returning early when either is
an unresolved constant), then compares them and pushes the diagnostic. The
feature needs the comparison in two places with two relations, so the
prefactor separates the bound extraction from the check:

- `LiteralBounds::of(&Subrange) -> Option<LiteralBounds>` returns the two
  literal ends together with their `i128` values, so the diagnostic can still
  point at both ends. `None` when either end is a named constant.
- `visit_subrange` becomes: extract, then compare, then report.

It also drops the two `expect("Value in range i128")` calls (the standards
forbid panicking constructs); a bound that does not fit `i128` is skipped
like an unresolved constant is. That path has no test today and is not
reachable from the parser, which is why the existing tests pass unchanged.

## Design doc reference

None. The reasoning lives in the rule's module doc comment, which is where
the next reader of this rule will look.

## File map

| File                                                   | Change                                            |
| ------------------------------------------------------ | ------------------------------------------------- |
| `compiler/analyzer/src/rule_decl_subrange_limits.rs`   | Prefactor extraction; two-case rule; tests        |
| `docs/reference/compiler/problems/P2002.rst`           | Say the code is for subrange types, not arrays    |
| `docs/reference/compiler/problems/P2024.rst`           | Inline `VAR` example; single-element is valid     |

## Tasks

- [x] Plan (this file)
- [ ] Prefactor: split bound extraction from the comparison, drop `expect`,
      existing tests unchanged
- [ ] Feature: `RangeContext` with the two cases; override
      `visit_subrange_specification` and `visit_array_subranges`; remove
      `visit_subrange`
- [ ] Tests in the rule: `ARRAY[0..0]` ok, `ARRAY[0..0, 1..1]` ok,
      `ARRAY[1..0]` P2024 pointing at the bounds, `TYPE` array `[1..1]` ok,
      `INT(5..5)` P2002, `INT(-10..10)` ok, `CASE 5..5` ok
- [ ] Docs for P2002 and P2024
- [ ] Open the follow-up issue for the inverted `CASE` label range
- [ ] `cd compiler && just`, `cd docs && just`, `cd specs && just`
- [ ] `git rm` this plan
