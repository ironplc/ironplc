# Plan: Reject Variable Redeclaration the Compiler Cannot Execute

Issue: [#1562](https://github.com/ironplc/ironplc/issues/1562)

## Goal

Settle how IronPLC treats a variable declared with the same name as one
in an enclosing or inherited scope, and make the compiler, the docs and
the ADR record agree.

Two scope pairs are rejected because neither has an executable path:

1. A derived function block redeclaring a field it inherits via
   `EXTENDS`. Already rejected by `P4044`, unconditionally. Real
   CODESYS/TwinCAT reject it (`C0097`), no known compiler accepts it,
   and codegen never lays out inherited fields. Keep the rule; fix the
   docs that call it legal.
2. A `PROGRAM` declaring a variable with the same name as a global
   variable. Currently accepted by analysis and miscompiled: codegen
   keys its name table by identifier, so the program local overwrites
   the global's entry, the global's initial value lands in the local's
   slot, and functions or function blocks that read the global fail
   with `P4007`. New problem `P4050`.

Hiding that compiles correctly stays allowed: a method local or
parameter over a function block field (end-to-end tested), and a
function or function block local over a global (end-to-end verified,
and required by `REQ-CL-analyzer-004` so a user constant can hide a
library global such as `Tc2_System.PI`).

## Architecture

- New semantic rule `rule_program_var_hides_global.rs`: collect every
  `VAR_GLOBAL` declaration in the merged library (configuration,
  resource, top-level extension, activated libraries), then for each
  `ProgramDeclaration` flag every non-`VAR_EXTERNAL` variable whose
  name matches a global. Case-insensitive via `Id` equality. Secondary
  label on the global's declaration.
- `P4044` unchanged in behaviour. Its doc no longer calls `EXTENDS` a
  vendor extension, since the Edition 3 dialect enables it.
- ADR-0051 records the decision and the evidence, including the
  standard-side caveat (vendor behaviour confirmed; standard text not
  retrievable in this environment).

## Prefactoring

`rule_var_decl_global_const_requires_external_const.rs` already walks
the library to collect global declarations by name through a private
visitor. The new rule needs the same collection. Extract
`intermediates::global_vars::collect_global_var_decls(lib)` and rewrite
the existing rule's collection pass on top of it, in its own commit,
with its tests unchanged.

## Design doc reference

None. The decision lands as ADR-0051.

## File map

| File | Change |
|------|--------|
| `compiler/analyzer/src/intermediates/global_vars.rs` | New shared collector (prefactor) |
| `compiler/analyzer/src/intermediates/mod.rs` | Register module |
| `compiler/analyzer/src/rule_var_decl_global_const_requires_external_const.rs` | Use the collector (prefactor) |
| `compiler/problems/resources/problem-codes.csv` | New `P4050` |
| `compiler/analyzer/src/rule_program_var_hides_global.rs` | New rule |
| `compiler/analyzer/src/lib.rs`, `stages.rs` | Register the rule |
| `compiler/analyzer/src/rule_assignment_aggregate_type_compat.rs` | Move the local-hides-global test to a function block |
| `docs/reference/compiler/problems/P4050.rst` | New problem doc |
| `docs/reference/compiler/problems/P4044.rst` | Drop "vendor extension" wording |
| `docs/explanation/object-orientation.rst` | Rewrite "Hiding inherited names" and glossary row |
| `docs/reference/language/variables/scope.rst` | State the program/global rule |
| `specs/adrs/0051-reject-variable-redeclaration-without-an-executable-path.md` | New ADR |

## Tasks

- [ ] Write plan (this document)
- [ ] Prefactor: shared global-variable collector
- [ ] `P4050` problem code, rule, registration, tests
- [ ] Update the aggregate-compat test that used a program local
- [ ] Docs: P4050, P4044, object-orientation, scope
- [ ] ADR-0051
- [ ] `cd compiler && just`, `cd docs && just compile`, `cd specs && just`
- [ ] `git rm` this plan
