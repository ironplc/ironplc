# Fix `commasep_oneplus` Trailing-Comma Grammar Bug

## Context

`compiler/parser/src/parser.rs` defines a family of separated-list combinators.
`commasep_oneplus` is broken:

```
rule commasep_oneplus<T>(x: rule<T>) -> Vec<T> = v:(x() ++ (_ comma() _)) comma() {v}
```

The `x() ++ (_ comma() _)` fragment already matches a one-or-more
comma-separated list. The trailing `comma()` demands a *spurious* trailing
comma, so the combinator only matches input that ends in a comma. IEC 61131-3
does not allow a trailing comma in these lists, so the fix is to remove the
trailing `comma()` (not make it optional):

```
rule commasep_oneplus<T>(x: rule<T>) -> Vec<T> = v:(x() ++ (_ comma() _)) {v}
```

## Callers

`commasep_oneplus` has exactly two callers:

1. **`prog_conf_elements()`** (used by `program_configuration()`), the `( ... )`
   element list of `PROGRAM inst WITH task : ptype ( elem, ... )`. This is a
   genuine user-facing parse failure: any program configuration with conf
   elements currently requires an illegal trailing comma. No test covers it
   today — all program-config tests in `tests/tasks.rs` use the no-element
   form.

2. **`fb_name_list()`**, used only by `fb_name_decl()`, which is dead code
   because of the trailing-comma bug.

## Why the one-line fix alone regresses tests

Removing the trailing comma makes `fb_name_decl()` reachable. It is tried
before the late-bound fallback in `var_init_decl()` and `var_declaration()`,
so it eagerly commits bare `x : SomeType` declarations to
`InitialValueAssignmentKind::FunctionBlock` instead of deferring via
`LateResolvedType`. This misclassifies non-FB types (structs, enums, aliases)
as function blocks and regresses ~6 parser tests.

`fb_name_decl` is fundamentally unsound: at parse time we cannot know a bare
type name refers to a function block — that is exactly why the
`LateResolvedType` placeholder + `xform_resolve_late_bound_type_initializer`
pipeline exists.

## Changes

1. Fix `commasep_oneplus` — remove the trailing `comma()`.
2. Remove `fb_name_decl()` and `fb_name_list()`, and remove the
   `fb_name_decl()` alternative from `var_init_decl()` and `var_declaration()`.
   Bare and `:=`-initialized FB declarations already flow correctly through
   `structured_var_init_decl__without_ambiguous()` (`:=`) and
   `var1_init_decl__with_ambiguous_struct()` -> `LateResolvedType` (bare).
   Keep the `fb_name()` rule — still used by `fb_task()`, `fb_invocation()`,
   and the config data-source path.
3. Add a regression test in `tests/tasks.rs` proving a program configuration
   WITH conf elements parses without a trailing comma.
4. Run `cd compiler && just`; all checks must pass (the previously-failing
   tests pass once the misclassification path is gone).

## Notes

Unblocks #1273 (the call-style FB initializer PR), which currently sidesteps
this bug by using `var1_list()` in its call-style rule.
