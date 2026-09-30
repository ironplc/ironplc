# Reject a Condition That Is Not BOOL

Issue: #1923

## Goal

Report a problem when the condition of an `IF`, `ELSIF`, `WHILE` or
`REPEAT ... UNTIL` statement is not a `BOOL`. Today `check` accepts
`IF d THEN` with `d : DINT` and codegen runs the `THEN` branch for any
non-zero value, a C idiom that IEC 61131-3 does not have: the standard
defines each of these statements over a Boolean expression (edition 2
3.3.2.3 and 3.3.2.4; edition 3 7.3.3.3 and 7.3.3.4, Table 72).

## Architecture

A new semantic rule, `rule_condition_type`, visits `If`, `ElseIf`, `While`
and `Repeat` and asks of each condition the same question an assignment
to a `BOOL` variable asks of its value: `value_type::check(types, BOOL,
expr, options)`. Using the shared relation means:

- a `BOOL`, an alias of `BOOL`, a comparison, a `BOOL` function or method
  result, a function block output of type `BOOL`, and a bit access `x.3`
  are accepted, since each resolves to `BOOL`;
- an untyped integer or real literal (`IF 1 THEN`) is rejected, as
  `b := 1` already is (P4035);
- an expression the analyzer left without a type is skipped, since there
  is nothing to compare; another rule has reported why.

A mismatch is reported as a new problem code, P4072
(`ConditionTypeInvalid`), on the condition's span with the actual type as
context, parallel to P4053 for a `CASE` selector.

### Relation to #1926

Open PR #1926 adds `ExprType::Error` and REQ-AO-analyzer-041, which says a
condition of the error type is not reported. `value_type::of` answers
`None` for `ExprType::Error` in that PR, so `value_type::check` answers
`Ok`, and this rule needs no change once both merge. The rule does not
match on `ExprType` itself, so the new variant causes no merge conflict.

### Dialects

No dialect is known to accept an integer condition: CODESYS and TwinCAT
report the condition's type as incompatible with `BOOL`, and Siemens SCL
defines conditions as `BOOL` expressions. The rule is therefore not gated
by a flag. The flag-gated conversions in `type_compat` (integer literal to
bit string, cross-family widening) never produce `BOOL`, so no existing
flag changes the outcome.

## Prefactoring

None needed. The relation the rule needs already exists as
`value_type::check`, shared by the assignment and call rules, and the
rule is a new module that touches no existing branching.

## Design doc reference

None. The rule is local to one module, as the `CASE` selector rule is;
its rationale lives in the module doc comment.

## File map

- `compiler/analyzer/src/rule_condition_type.rs` (new): the rule and its tests
- `compiler/analyzer/src/lib.rs`, `compiler/analyzer/src/stages.rs`: register it
- `compiler/problems/resources/problem-codes.csv`: add P4072
- `docs/reference/compiler/problems/P4072.rst` (new): problem documentation
- `docs/reference/language/structured-text/{if,while,repeat}.rst`: say the
  condition is `BOOL` and link P4072

## Tasks

- [ ] Commit this plan
- [ ] Write failing tests for the rule (BDD names, `rstest` cases per type)
- [ ] Add P4072 to the CSV and write its documentation
- [ ] Implement the rule and register it
- [ ] Update the IF, WHILE and REPEAT reference pages
- [ ] Run the analyzer tests, fix any fixture that relied on a non-BOOL
      condition (report which)
- [ ] Correct the IEC clause in the issue with a comment
- [ ] `git rm` this plan
- [ ] `cd compiler && just`, `cd specs && just`, docs build without warnings
