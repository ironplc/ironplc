# Plan: Diagnose an inverted CASE label range

Fixes [#1725](https://github.com/ironplc/ironplc/issues/1725). Follows
[#1673](https://github.com/ironplc/ironplc/issues/1673) / #1726.

## Goal

A `CASE` label range whose minimum is greater than its maximum (`10..1:`)
selects nothing, so the branch can never run. It is reported with its own
problem code, P4051, while a one-value label range (`5..5:`) stays accepted.

## Context

#1726 keyed `rule_decl_subrange_limits` on the node that owns a range, with
two cases: a subrange type (`min < max`, P2002) and an array dimension
(`min <= max`, P2024). The generic `visit_subrange` override went away, so
the rule stopped touching `CASE` label ranges altogether; before that they
were checked with the subrange-type relation and reported as P2002
"Subrange declaration ...", which both rejected `5..5:` and described
`10..1:` wrongly.

## Architecture

A third `RangeContext` variant, `CaseLabel`, accepting `min <= max` and
reporting `Problem::CaseLabelRangeInvalid` (P4051). The rule overrides
`visit_case_selection_kind` and checks the `Subrange` arm; the label
spans point at the two bounds like the other two cases. Everything else
(`LiteralBounds`, `check`) is reused unchanged, which is what the enum was
for.

P4051 sits in the semantic-analysis block (P4000–P5999) next to P4041, the
other `CASE` label problem, rather than in the type-system block where
P2002 and P2024 live: a `CASE` label is a statement, not a type
declaration.

## Prefactoring

The rule's file, module and struct are named `rule_decl_subrange_limits`
/ `RuleDeclSubrangeLimits`: "declaration" and "subrange" were accurate
when the rule only checked subrange types, and stayed near enough while it
checked declarations. A `CASE` label is neither a declaration nor a
subrange type, so the third case would make the name a lie the next reader
trips over.

Prefactor: `git mv` the module to `rule_range_limits.rs`, rename the struct
to `RuleRangeLimits`, and update the two references in `lib.rs` and
`stages.rs`. Behaviour-preserving; tests unchanged apart from the module
path they live under.

## Design doc reference

None. The reasoning is in the rule's module doc comment.

## File map

| File                                              | Change                                          |
| ------------------------------------------------- | ----------------------------------------------- |
| `compiler/analyzer/src/rule_range_limits.rs`      | Renamed from `rule_decl_subrange_limits.rs`; `CaseLabel` case; tests |
| `compiler/analyzer/src/lib.rs`, `stages.rs`       | Module rename                                   |
| `compiler/problems/resources/problem-codes.csv`   | P4051 `CaseLabelRangeInvalid`                   |
| `docs/reference/compiler/problems/P4051.rst`      | New page                                        |

## Tasks

- [x] Plan (this file)
- [ ] Prefactor: rename the rule module and struct
- [ ] P4051 in the problem-code registry
- [ ] `RangeContext::CaseLabel` and `visit_case_selection_kind`
- [ ] Tests: `CASE 10..1` P4051 pointing at the bounds, `CASE -1..1` ok,
      two inverted labels report twice, `5..5` still ok
- [ ] `docs/reference/compiler/problems/P4051.rst`
- [ ] `cd compiler && just`, `cd docs && just`, `cd specs && just`
- [ ] `git rm` this plan
