# Unresolved Arithmetic Has the Error Type

## Goal

An arithmetic expression that no overload applies to is reported once with
P4049 and nothing else: no second P4049 on an enclosing expression, no P4035
or P4027 on the assignment, no P4026 on an enclosing call, no P4053 on a
`CASE` selector ([#1876](https://github.com/ironplc/ironplc/issues/1876)).

## Architecture

Today such an expression keeps its left operand's type
(REQ-AO-analyzer-022), so every enclosing check judges the wrong type again.

`ExprType` gains an `Error` variant: the expression's value has no valid type
and a diagnostic has already been reported for it. `xform_resolve_expr_types`
gives it

- to an arithmetic binary expression, or a call to `ADD`, `SUB`, `MUL` or
  `DIV`, that no overload applies to (the resolver answers `None`, which the
  operator rule always reports), and
- to an expression whose type is computed from an operand of the error type:
  an arithmetic or bitwise operator, a negation, a parenthesised expression,
  a dereference, a call to an overloaded name, and a call to a generic
  function whose return type binds to that operand.

A relational comparison is still `BOOL`, and a function with a concrete
return type still has it: the enclosing context of those is well typed.

Every consumer already skips a value without a judgeable type
(`value_type::of`, `operand_type_name`, `representation_of_expr` return
`None`), so the error type answers `None` there and the enclosing checks
skip it. The distinction from `None` ("not typed") matters because an
enclosing operator falls back to its other operand when one has no type
(`prefer_concrete`, generic return binding), which would re-type the
enclosing expression from the good operand and report it again.

`Unchecked` resolution (REQ-AO-analyzer-012) is unchanged: an operand the
predicate cannot judge still gives the left operand's type, which codegen
needs.

## Prefactoring

None needed beyond the variant itself: every `match` on `ExprType` is a
two-line arm, and the consumers already treat a missing type as "skip".

## Design doc reference

`specs/design/arithmetic-operator-overloads.md`: narrow REQ-AO-analyzer-022
to the unchecked case, add requirements for the error type and for each
enclosing check that skips it, and replace "may be reported as well" in
*Rules*.

## File map

- `compiler/dsl/src/textual.rs` — `ExprType::Error`
- `compiler/analyzer/src/xform_resolve_expr_types.rs` — assign and propagate it
- `compiler/analyzer/src/value_type.rs`, `type_environment.rs`,
  `rule_case_selector_type.rs` — skip it
- `compiler/codegen/src/type_info.rs` — treat it as untyped
- `compiler/analyzer/src/spec_conformance_arithmetic_operator_overloads/` —
  conformance tests for the new requirements (a child module; the parent is
  near the size limit)
- `specs/design/arithmetic-operator-overloads.md`
- `docs/reference/compiler/problems/P4049.rst`

## Tasks

- [ ] Failing conformance tests: nested arithmetic, assignment (P4035),
      user and stdlib call return (P4027), call argument (P4026), `CASE`
      selector (P4053), `AND` operand, overloaded-call fold, `IF` condition
- [ ] Add `ExprType::Error` and its arms
- [ ] Assign and propagate it in `xform_resolve_expr_types`
- [ ] Amend the design doc and P4049 page
- [ ] `cd compiler && just`, `cd specs && just`, docs build
- [ ] `git rm` this plan
