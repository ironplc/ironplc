# Check Constant Ranges in Method Call Arguments

Issue: #1823

## Goal

A constant passed to a method (`inst.M(300)`, `inst.M(q := 300)`) is checked
against the type of the parameter it binds to, as a function or function block
argument already is: P2026 for an integer out of range, P2040 for an untyped
real that a `REAL` parameter cannot hold.

## Architecture

`rule_constant_range` checks function arguments in `visit_function` and
function block arguments in `visit_fb_call`, and has no `visit_method_call`.
Add one:

1. Resolve the receiver. A named instance is looked up in the rule's
   `Declarations` (which also holds `VAR_EXTERNAL` and configuration
   `VAR_GLOBAL` declarations), and the method in its type's `EXTENDS` chain
   through `callee_resolution::FunctionBlocks::resolve_method`, as
   `rule_method_call_declared` does. `THIS^` / `SUPER^` receivers are not
   resolved by the analyzer yet (#1406; `rule_method_call_declared` reports
   P9999), so they are skipped.
2. Bind each argument to its declared parameter with
   `call_assignment_check::bind_inputs`, the binding `rule_method_call_declared`
   already validates against, and check its expression against the
   parameter's declared type (`variable_type::resolve_initializer`).
3. Positional binding disagrees between the analyzer (`VAR_INPUT` only) and
   codegen (`VAR_INPUT` and `VAR_IN_OUT`, in declaration order) for a method
   that declares a `VAR_IN_OUT`. No such positional call compiles today (one
   side or the other refuses the argument count), so the check skips positional
   arguments to such a method rather than check one against a parameter codegen
   would not bind it to.

The binding-and-check step is a helper taking any `HasVariables` owner, so the
function block path can use it too once #1904 (which binds user-defined
function block arguments by declaration) lands.

## Prefactoring

None needed: the rule already has `check_expr` for an argument against a type,
and the binding lives in `call_assignment_check`. The new behaviour is one
visitor method and two small helpers. `rule_constant_range.rs` stays under
1000 lines.

## Overlap with #1904

#1904 adds the same `FunctionBlocks` field and imports to
`rule_constant_range.rs`. This change adds them with the same text so the
overlap is as small as possible; a small conflict is still expected.

## Design doc reference

No design document covers the rule; its module doc comment describes where
constants are checked and is updated.

## File map

- `compiler/analyzer/src/rule_constant_range.rs` — `visit_method_call`, helpers, module doc
- `compiler/analyzer/src/rule_constant_range/tests.rs` — method argument tests
- `docs/reference/compiler/problems/P2026.rst` — mention method arguments

## Tasks

- [ ] Tests first: named, positional, in range, real (P2040), inherited method,
      method in expression position, `VAR_EXTERNAL` receiver, `VAR_IN_OUT`
      method positional skip, `THIS^` receiver skipped, CODESYS and TwinCAT
      dialects
- [ ] Implement `visit_method_call`
- [ ] Update module doc and P2026 page
- [ ] `git rm` this plan
- [ ] `cd compiler && just`, docs build
