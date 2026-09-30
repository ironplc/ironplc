# Plan: Report an Undeclared Element or Target Type in a Variable Declaration

Issue: #1896

## Goal

`ironplcc check` reports an undeclared type written inside a variable
declaration's type the same way it reports one inside a `TYPE` declaration:

| Declaration | Reported as |
|---|---|
| `x : ARRAY[1..2] OF E_Missing;` | P2013 (array element type) |
| `x : REF_TO E_Missing;` / `POINTER TO` / `REFERENCE TO` | P2011 (referenced type) |
| `x : REF_TO ARRAY[1..2] OF E_Missing;` | P2013 |
| `x : ARRAY[1..2] OF REF_TO E_Missing;` / `OF POINTER TO` | P2013 |

Today `check` accepts all of these; `compile` stops with P9999 ("Unsupported
array element type") or P2011 from code generation, and some (an array of
pointers, an unused function's input) compile without any diagnostic.

## Architecture

The check belongs to type resolution, not to a semantic rule: a `TYPE`
declaration's element and referenced types are reported while the type
environment is built (`xform_resolve_type_decl_environment`), and a bare
undeclared variable type (P2008) while variable initializers are resolved
(`xform_resolve_late_bound_type_initializer`). The variable forms go to the
second pass, which already walks every variable declaration against the type
environment after `TYPE` declarations and function blocks are in it.

`TypeResolver` in `xform_resolve_late_bound_type_initializer` gains a
`fold_var_decl` that, before folding the declaration as today:

- for an inline `ARRAY ... OF` initializer, resolves the element type with the
  helper `TYPE` declarations use (P2013);
- for a reference initializer (`REF_TO`, `REFERENCE TO`, `POINTER TO`), resolves
  a named target with `TypeEnvironment::resolve_reference_target` (P2011) and an
  inline array target with the array element helper (P2013).

Diagnostics join the pass's best-effort diagnostics like P2008; the declaration
is left unchanged. Only the element or target *type name* is checked; array
bounds and a bare undeclared type (P2008) are reported elsewhere.

The check lives in a submodule, `xform_resolve_late_bound_type_initializer/nested_type.rs`,
with its tests, so the parent module stays under 1000 lines.

Structure fields and `TYPE` declarations are already reported. Function and
method return types cannot be arrays or references in the grammar; a named
return type is #1893 / #1895.

## Prefactoring

`intermediates::array::try_from` computes the element representation inline.
Extract it into `array::element_type(node_name, subranges, env)` so the pass
reuses the exact check and diagnostic instead of copying it. Behaviour
preserving; existing tests unchanged.

## Design doc reference

None. The rationale lives in the module doc comment.

## File map

- `compiler/analyzer/src/intermediates/array.rs` — extract `element_type`
- `compiler/analyzer/src/xform_resolve_late_bound_type_initializer.rs` — `fold_var_decl`
- `compiler/analyzer/src/xform_resolve_late_bound_type_initializer/nested_type.rs` — the check and its tests
- `compiler/ironplc-cli` tests — `check` on the issue's program, default and TwinCAT dialects
- `docs/reference/compiler/problems/P2011.rst`, `P2013.rst` — mention variable declarations

## Tasks

- [ ] Prefactor: extract `array::element_type`
- [ ] Failing tests (array, `REF_TO`, `POINTER TO`, `REFERENCE TO`, nested forms, `VAR_INPUT`, globals, positive cases)
- [ ] Implement the check in type resolution
- [ ] CLI tests with `--dialect twincat` and the default dialect
- [ ] Update P2011 / P2013 documentation
- [ ] `git rm` this plan
- [ ] `cd compiler && just`, `cd specs && just`, docs build
