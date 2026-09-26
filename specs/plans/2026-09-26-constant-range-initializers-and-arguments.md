# Check Out-of-Range Constants in Initializers, Type Defaults and Call Arguments

Issue: https://github.com/ironplc/ironplc/issues/1803

## Goal

A constant that does not fit the type it is stored into is reported (P2026
for integers, P2040 for an untyped real stored into a `REAL`) wherever it is
stored, not only in assignments and simple `VAR` initializers: array
initializers (including repeated elements), structure and function block
instance initializers, structure field defaults, type alias defaults, and
function / function block call arguments.

## Architecture

All work stays in `compiler/analyzer/src/rule_constant_range.rs`, which
already pushes a target type down to the constants beneath it.

- One `check_initializer(&InitialValueAssignmentKind)` replaces the `Simple`
  arm of `visit_var_decl` and also handles `Array`, `Structure` and
  `FunctionBlock` initializers by resolving the declared type and walking the
  initializer against it (element type for arrays, field type for
  structure element inits, recursively).
- `TYPE` declarations reuse the same walk: `SimpleDeclaration`,
  `StructureElementDeclaration` (through `check_initializer`),
  `ArrayDeclaration` and `StructureInitializationDeclaration` (against the
  declared type from the type environment).
- `visit_function` binds positional inputs through
  `FunctionSignature::bind_inputs` (named inputs are already rewritten to
  positional) and checks each argument against the parameter type. A generic
  parameter (`ANY_NUM`) is not in the type environment and is skipped.
- `visit_fb_call` resolves the instance's function block type and checks each
  input argument against the matching `VAR_INPUT`/`VAR_IN_OUT` field, by name
  or by position among the inputs.

## Prefactoring

- Move the rule's tests into `rule_constant_range/tests.rs`: the module is
  789 lines and would cross the 1000-line limit.
- Extract the `Simple` arm of `visit_var_decl` into `check_initializer` so the
  new initializer kinds and the `TYPE` declarations drop into one place.

## Design doc reference

None.

## File map

- `compiler/analyzer/src/rule_constant_range.rs` — modified
- `compiler/analyzer/src/rule_constant_range/tests.rs` — created
- `compiler/analyzer/src/lib.rs` or `stages.rs` — pass the function
  environment if needed

## Tasks

- [ ] Prefactor: move tests out; extract `check_initializer`
- [ ] Array, structure and function block initializers
- [ ] `TYPE` declaration defaults
- [ ] Function and function block call arguments
- [ ] Tests for each context, integer and real
- [ ] Remove this plan; run `cd compiler && just`
