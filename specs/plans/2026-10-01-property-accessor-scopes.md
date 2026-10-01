# Property follow-ups: accessor scopes and inherited properties

## Goal

Fix two bugs in `PROPERTY` support (#1871), found in the review of #1901:

1. **GET and SET share one scope.** Both accessors are `MethodDeclaration`s
   named after the property, and every pass keys a scope by that name, so a
   local declared in both (`tmp : INT` in GET and in SET) is P4014. TwinCAT
   accepts it.
2. **Inherited properties are not recognised.** `rule_use_declared_symbolic_var`
   collects only the function block's own properties, so using a base block's
   property in a derived block is P4007 ("Variable not defined") instead of
   the intended P9999 ("Use of a PROPERTY").

Both reproduce on main (91a579557) with `ironplcc check --allow-fb-inheritance`.

## Architecture

**Accessor scopes.** A scope's name comes from three hand-written matches
on `ScopeNode` (`xform_resolve_symbol_and_function_environment.rs:125`,
`xform_mark_unwritten_constants.rs:320` and `:581`), each taking a method's
`name`. The fix gives an accessor its own scope name:

- `MethodDeclaration` gets `accessor: Option<Accessor>` (`Get` or `Set`),
  set only by `PropertyDeclaration::get_accessor`/`set_accessor`. The
  method keeps the property's name, because GET's result variable is the
  property name.
- An accessor's scope name is the property name plus the accessor
  (`Position.GET`, `Position.SET`). The `.` cannot appear in an identifier,
  so it never collides with a method's scope.

**Inherited properties.** In `enter_scope` for a function block, the rule
collects the property names of the whole `EXTENDS` chain instead of the
block's own. `FunctionBlocks` (`callee_resolution.rs`) already walks that
chain for method resolution; its private `chain` helper becomes
`pub(crate)` for this.

## Prefactoring

**One prefactor PR:** a `ScopeNode::scope_name()` method in
`dsl/src/scope.rs`, replacing the three matches above. No behaviour
change: it returns exactly what they return today. The accessor change then
touches one place instead of three, and a future scope kind cannot be named
differently in different passes.

Inherited properties need no prefactor: the chain walk exists, it only
needs to be reachable.

## Design doc reference

`specs/design/beckhoff-twincat-dialect.md` §1.2 (`PROPERTY`). No change to
what it states as supported; the two fixes make the code match it.

## File map

Prefactor PR:
- `dsl/src/scope.rs`: `ScopeNode::scope_name()`
- `analyzer/src/xform_resolve_symbol_and_function_environment.rs`,
  `analyzer/src/xform_mark_unwritten_constants.rs`: use it

Core change PR (stacked on the prefactor PR):
- `dsl/src/oop.rs`, `dsl/src/common.rs`: `Accessor`, the `accessor` field,
  set by the accessor constructors
- `dsl/src/scope.rs`: accessor scope names
- `parser/src/parser.rs` and the tests that build a `MethodDeclaration`:
  `accessor: None`
- `analyzer/src/callee_resolution.rs`: `chain` reachable from the rule
- `analyzer/src/rule_use_declared_symbolic_var.rs`: properties of the chain
- tests in `analyzer` (both bugs) and `sources` (a `.TcPOU` property whose
  GET and SET both declare `tmp`)

## Tasks

Prefactor PR:
- [ ] `ScopeNode::scope_name()` and its three callers; existing tests unchanged

Core change PR:
- [ ] `Accessor` and `MethodDeclaration::accessor`; accessor scope names
- [ ] Test: GET and SET each declaring `tmp` pass `check`
- [ ] Inherited property names in `rule_use_declared_symbolic_var`
- [ ] Test: a base block's property used in a derived block is P9999, not P4007
- [ ] `cd compiler && just`
