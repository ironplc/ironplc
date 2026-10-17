# Type `THIS^` and `SUPER^` (#1888, #1406)

## Goal

Inside a function block and its methods (including property accessors),
`THIS^` has the type of the enclosing function block and `SUPER^` the type of
its `EXTENDS` base. Member access (`THIS^.x`, `SUPER^.x`) and method calls
(`THIS^.M()`, `SUPER^.M()`) through them are analyzed and compiled. `THIS^`
outside a function block and `SUPER^` in a block without a base are reported
(#1406).

Out of scope, still P9999: `THIS^` or `SUPER^` used as a value on its own
(passed, assigned, compared), `SUPER^()` (calling the base body), and bare
`THIS` (the `FB_init(THIS)` pattern).

## Corpus

In the #1199 corpus, 40 files use these forms: `THIS^.x` 119 times,
`THIS^.M(...)` 16, `SUPER^.M(...)` 4, `SUPER^()` 6, `THIS^` as a value 0, bare
`THIS` 18 (out of scope). 17 files fail on `THIS^` alone today.

## Current behaviour

`THIS^`/`SUPER^` parse (`SymbolicVariableKind::SelfRef`, receiver
`MethodReceiver::SelfRef`) and are rejected with P9999 in several places that
each say "see #1406": `xform_resolve_late_bound_expr_kind` (assignment target),
`xform_resolve_expr_types` (`fold_self_ref_variable`), `rule_method_call_declared`
(receiver), `rule_unsupported_extension` (any use), and in codegen
`compile_expr` and `compile_method`.

## Architecture

- **Self type.** The type `THIS^` names is the enclosing function block;
  `SUPER^` names its base. Member access then goes through the existing
  path for an instance: `xform_resolve_expr_types::resolve_parent_struct_type`
  gets the block's `IntermediateType` from the type environment, and
  `rule_method_call_declared` resolves the method along the `EXTENDS` chain
  (`FunctionBlocks::resolve_method`), starting at the base for `SUPER^`.
- **Inherited members.** `THIS^.x` must find a field inherited through
  `EXTENDS`, as an unqualified `x` already does (`inherited_fields`). The
  type environment's function block type holds only the block's own fields,
  so the lookup uses the same inherited field list.
- **Shadowing.** In `METHOD SetX VAR_INPUT x : INT; END_VAR THIS^.x := x;`
  the left side is the block's field and the right side the parameter. In
  codegen a method already runs on its block's fields, so `THIS^.x` is the
  field's variable slot, looked up in the block's fields rather than the
  method's scope.
- **#1406.** With the enclosing block known, `THIS^` outside a function
  block (in a program or function) and `SUPER^` in a block without
  `EXTENDS` get a new problem code each, or one shared code.
- `rule_unsupported_extension` stops flagging `THIS^`/`SUPER^` with a member
  or method after it, and keeps flagging them as a value on their own.

## Prefactoring

**One prefactor PR:** the enclosing function block, as one helper. Each pass
that will resolve `THIS^` (expression types, late-bound kinds, the method
call rule, codegen) needs "which function block am I in, and what is its
base". Today none tracks it. A small `EnclosingBlock` in the analyzer (pushed
on `ScopeNode::FunctionBlock`, kept through its methods, popped on exit),
with `self_type(SelfRefKind) -> Option<TypeName>`, gives every pass the same
answer. Today `xform_mark_unwritten_constants` reconstructs the enclosing
block by hand (`self.scope.first()` in `declaring_scope`); the prefactor
replaces that with `EnclosingBlock`, so the helper lands with a real user.
No behaviour change.

## Design doc reference

- [ADR-0041](../adrs/0041-staged-method-and-interface-dispatch.md) Phase 1
  names `THIS^`/`SUPER^` as static.
- `specs/design/beckhoff-twincat-dialect.md` §1.6 (`THIS^` and `SUPER^`):
  update what is supported.
- `specs/design/oop-dispatch.md` (#1870) says Phase 1 shipped `THIS^` and
  `SUPER^`; that is wrong until this lands, to correct there.

## PRs and tracking

More than one core change PR, so #1888 tracks them:

1. Prefactor: `EnclosingBlock`.
2. Core, analysis: types, member access, method calls, #1406 diagnostics.
   `check` passes for the supported forms; `compile` still reports P9999 at
   codegen.
3. Core, codegen: `THIS^.x`, `THIS^.M()`, `SUPER^.x`, `SUPER^.M()`.

If 2 and 3 turn out small, they become one PR.

## File map

- `analyzer/src/enclosing_block.rs` (new), `lib.rs`
- `analyzer/src/xform_resolve_expr_types.rs`,
  `xform_resolve_late_bound_expr_kind.rs`, `rule_method_call_declared.rs`,
  `rule_unsupported_extension.rs`, `variable_type.rs`,
  `xform_mark_unwritten_constants.rs`
- `problems/resources/problem-codes.csv`, new `docs/reference/compiler/problems/P####.rst`
- `codegen/src/compile_expr.rs`, `compile_method.rs`
- `docs/reference/language/object-orientation/this-and-super.rst`,
  `specs/design/beckhoff-twincat-dialect.md`

## Tasks

Prefactor PR:
- [ ] `EnclosingBlock` with `self_type`; unit tests
- [ ] `xform_mark_unwritten_constants` uses it instead of `self.scope.first()`

Core PR (analysis):
- [ ] Self type in `xform_resolve_expr_types` and `xform_resolve_late_bound_expr_kind`
- [ ] Inherited fields through `THIS^.x`
- [ ] `THIS^.M()` / `SUPER^.M()` in `rule_method_call_declared`
- [ ] #1406 diagnostics and docs page
- [ ] `rule_unsupported_extension`: only a bare `THIS^`/`SUPER^`
- [ ] Tests: shadowing parameter, inherited field, method call, accessor body, `SUPER^`, each #1406 case
- [ ] Corpus rerun

Core PR (codegen):
- [ ] `THIS^.x` / `SUPER^.x` as the block's field slots
- [ ] `THIS^.M()` / `SUPER^.M()` as static calls on the current instance
- [ ] End-to-end tests that run the compiled code
