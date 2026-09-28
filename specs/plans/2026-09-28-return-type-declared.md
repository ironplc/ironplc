# Report an undeclared return type

## Goal

A function or method whose return type names no declared type is reported
(#1893). Today `FUNCTION F : E_Missing` passes `check` and `compile` without
a diagnostic, while a variable of the same type gets P2008.

## Architecture

A new semantic rule, `rule_return_type_declared`, visits every
`FunctionDeclaration` and `MethodDeclaration`. For a
`FunctionReturnType::Named` return type it looks the name up in the type
environment (`context.types()`), which holds elementary types, declared data
types, function blocks and interfaces. A miss is a new problem, **P2042
ReturnTypeNotDeclared**, pointing at the type name.

A dedicated code follows the precedent of the other "X references an
undeclared type" problems (P2011 parent type, P2013 array element type, P2021
structure field type) rather than reusing P2008, whose message is about a
variable's type.

`STRING`/`WSTRING` return types are always declared and are not looked up.

Not covered: properties (their `GET` accessor is a `MethodDeclaration`, so
they are covered once #1871 lands), interface method prototypes (#1892;
they get the same check when that PR is rebased), and the undeclared element
type of an `ARRAY OF` or `POINTER TO` inside a `VAR` block, which is a
separate gap found while writing this.

## Prefactoring

None needed. The rule is new and self-contained; no existing code has to
change shape for it.

## File map

- `compiler/problems/resources/problem-codes.csv`: P2042.
- `docs/reference/compiler/problems/P2042.rst`.
- `compiler/analyzer/src/rule_return_type_declared.rs` (new), registered in
  `lib.rs` and `stages.rs`.

## Tasks

- [ ] Problem code and documentation page.
- [ ] Rule with tests: undeclared function return type, undeclared method
      return type, elementary / declared / function block / string return
      types pass, a method without a return type passes.
- [ ] Check the end-to-end behaviour with `ironplcc check` and `compile`.
- [ ] `git rm` this plan, `cd compiler && just`, PR fixing #1893.
