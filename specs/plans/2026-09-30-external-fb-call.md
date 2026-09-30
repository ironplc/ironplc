# Call a Function Block Instance Declared VAR_EXTERNAL

Issue: #1858

## Goal

A program calls a global function block instance through `VAR_EXTERNAL`
(`g(IN := TRUE, PT := T#1s); q := g.Q;`) and it works end to end: `check`
accepts it, and the compiled program calls the one global instance, so its
state persists across scans and its outputs read through the external.

Both sources of a global are covered:

- a `VAR_GLOBAL` of a `CONFIGURATION`, reached through `VAR_EXTERNAL`;
- a top-level `VAR_GLOBAL` (`--allow-top-level-var-global`), reached through
  `VAR_EXTERNAL` or, as that extension allows for other globals, directly.

Both standard library (`TON`) and user-defined function block types.

## Architecture

Two independent defects.

1. **Analyzer (P4012).** `rule_function_block_invocation` and
   `rule_method_call_declared` find an instance's type through
   `callee_resolution::InstanceTypes`, which only records declarations whose
   initializer is `InitialValueAssignmentKind::FunctionBlock`. The parser
   gives a `VAR_EXTERNAL` a `Simple` initializer (it cannot tell a function
   block type from any other named type), and type resolution leaves it so --
   rightly, since an external declares no instance (`rule_abstract_not_instantiated`
   treats a `FunctionBlock` initializer as an instantiation). Top-level
   `VAR_GLOBAL` instances are recorded, but the walk clears them on leaving the
   first POU.

   `InstanceTypes` becomes two layers:
   - the unit's own declarations, now including a `VAR_EXTERNAL`, recorded by
     the type name it gives. A type that is not a function block then fails the
     function-block lookup exactly as an undeclared instance does (P4012).
   - the top-level `VAR_GLOBAL` instances, collected once from the library,
     never cleared, and shadowed by a unit's own declaration.

   `VAR_GLOBAL` declarations met during the walk are no longer recorded as the
   unit's own: a configuration's globals are reached only through
   `VAR_EXTERNAL`.

2. **Codegen.** Globals get their slots before the user function block types
   are registered, so a global instance of a user-defined function block gets
   no `fb_instances` entry and a call to it fails. Register the types first.
   A function block body starts from an empty `fb_instances`, so a
   `VAR_EXTERNAL` instance inside a function block body is not found either:
   re-insert the global instances as the other global mappings already are.

## Prefactoring

- `compile_fn.rs`: `compile_user_function` and `compile_user_function_block`
  each carry the same five loops re-inserting the global entries of the saved
  maps. Extract one generic helper (`global_entries`), so the function block
  body gains global instances with one more line instead of a sixth loop.
- `compile.rs` (2193 lines, over the limit): move the user function block type
  pre-scan out of `compile_program_with_functions` into its own function in a
  new module, so the fix is to move one call.

Both behaviour-preserving, in their own commit.

## Design doc reference

`specs/design/function-block-infrastructure-design.md` (no change expected).

## File map

- `compiler/analyzer/src/callee_resolution.rs` -- `InstanceTypes` layers
- `compiler/analyzer/src/rule_function_block_invocation.rs`,
  `rule_method_call_declared.rs` -- seed the global layer; tests
- `compiler/codegen/src/compile_fn.rs` -- `global_entries`, FB body instances
- `compiler/codegen/src/compile.rs`, new `compile_fb_types.rs` -- pre-scan
- `compiler/codegen/tests/it/end_to_end_global_fb.rs` -- end-to-end tests
- `docs/compiler/problems/P4012.rst` -- mention `VAR_EXTERNAL` if useful

## Tasks

- [ ] Prefactor: `global_entries` helper in `compile_fn.rs`
- [ ] Prefactor: extract the user FB type pre-scan
- [ ] Analyzer tests (fail first): external TON, external user FB, top-level
      global via external and directly, external of a non-FB type still P4012,
      method call through an external
- [ ] Analyzer fix in `InstanceTypes`
- [ ] End-to-end tests (fail first): state persists across scans, outputs read
      through the external, configuration and top-level globals, user FB,
      external inside a function block body
- [ ] Codegen fix
- [ ] Check the CLI (`ironplcc compile` + `ironplcvm run`) on the issue's program
- [ ] `git rm` this plan; `cd compiler && just`
