# Function Parameters of an Inline Type

Plan for [#1952](https://github.com/ironplc/ironplc/issues/1952): a function
parameter whose type is spelled out in place is missing from the function's
signature. It also fixes point 2 of
[#1946](https://github.com/ironplc/ironplc/issues/1946), which has the same
cause, and is a first slice of step 6a of
[#1968](https://github.com/ironplc/ironplc/issues/1968).

## Goal

Every `VAR_INPUT`, `VAR_OUTPUT` and `VAR_IN_OUT` declaration of a `FUNCTION`
is a parameter of its signature, whatever form its type takes. These
functions then pass `check`:

```
FUNCTION f : DINT
  VAR_INPUT a : ARRAY[1..3] OF DINT; i : DINT; END_VAR
  f := a[i];
END_FUNCTION

FUNCTION g : DINT
  VAR_INPUT m : (M0, M1, M2); END_VAR
  g := 0;
END_FUNCTION

FUNCTION h : DINT
  VAR_IN_OUT r : INT (1..2); END_VAR
  h := r;
END_FUNCTION
```

Today each call to them reports P4018 (one argument too many), and a P4026
for the arguments that are then bound to the wrong parameter.

## Cause

`EnvironmentResolver::visit_function_declaration`
(`xform_resolve_symbol_and_function_environment.rs`) builds each parameter
from `VarDecl::type_name()`. A type spelled out in place has no name
(`TypeReference::Inline`), and only the `REF_TO` case of it is handled; an
inline array, enumeration or subrange reaches `_ => continue` and is dropped.
All later consumers inherit the shorter parameter list:

- the arity check and `bind_inputs` (the false P4018/P4026);
- `xform_named_to_positional_args` (a named argument matches no input, P4023);
- `xform_mark_unwritten_constants`, which does not see the `VAR_IN_OUT`
  argument as written;
- `compile_user_function`, which pairs declarations with
  `input_parameters()` in order, so every later parameter gets the wrong
  passing mode.

Function block inputs and method parameters are not affected: they are read
from the declarations, not from a `FunctionSignature` (checked: the same
inline array input on a `FUNCTION_BLOCK` and on a `METHOD` passes `check`).

## Architecture

The parameter's type is the type its declaration declares, which the
analyzer already records as `VarDecl::type_id` (ADR-0055); an inline type has
an anonymous id there. So the signature takes the id rather than a name:

- `IntermediateFunctionParameter` gains `type_id: Option<TypeId>`, the
  declared type of a user function parameter (`None` when it does not
  resolve, and for the standard library's generic `ANY_*` parameters, which
  are not types in the environment).
- `param_type` becomes `Option<TypeName>`: the name the declaration uses,
  or the generic category; `None` for a type spelled out in place. The
  standard library helpers keep building `Some(name)`.
- The signature builder no longer matches on the initializer: every
  parameter declaration is kept, with `type_id: var_decl.type_id`.
  `is_reference` stays as it is (`REF_TO` parameters keep the referenced
  type name), so the reference rules are unchanged.
- The checks that read a parameter's type ask for it by id when there is
  one, and by name otherwise:
  - `value_type::check` (P4026) takes the expected type as an id or a
    generic category name. An anonymous array accepts an array value of the
    same shape (as `composite_accepted` already does for a named one); an
    anonymous subrange compares by its base type, as `of` already does for a
    value; an anonymous enumeration accepts only itself. The `expected`
    context shows `value_type::describe`, e.g. `ARRAY[1..3] OF DINT`.
  - `rule_function_call_in_out_argument` (exact match of an elementary
    `VAR_IN_OUT`) and `rule_constant_range` take the representation by id.
  - `compile_user_function` picks the passing width from the id.

Return types stay `TypeName`: a function cannot return an inline type in the
supported grammar, so nothing is wrong there. Moving them to `TypeId` is the
rest of #1968 step 6a.

### Code generation

Passing an array by value to a function is not implemented in code
generation, for a named array type too: `check` passes and `compile` fails
with P9999 at the first use of the input. This change does not alter that;
it is tracked by [#1977](https://github.com/ironplc/ironplc/issues/1977)
(next to #1483 for array locals). Accepting the inline form in `check`
makes it behave like the named form, which `check` already accepts.

Inline enumerations do not compile yet (#1948 and #1968 step 1), so the
end-to-end test of the passing-mode fix uses a `VAR_IN_OUT` inline subrange
followed by a scalar input.

## Prefactoring

**Record declaration type ids before building the function environment.**
`xform_resolve_decl_types` runs after
`xform_resolve_symbol_and_function_environment` today, so the signature
builder cannot read `VarDecl::type_id`. The two passes do not read each
other's results (the first only folds `VarDecl::type_id` in, the second only
reads names and builds environments), and nothing between them allocates a
type id, so running `xform_resolve_decl_types` first leaves every id and
every diagnostic unchanged. Prefactor PR #1980; the existing tests pass untouched
(the `xform_resolve_expr_types` test helper mirrors the new order).

No other prefactor: the parameter construction shrinks rather than grows,
and the consumer changes are one call each.

## Design doc reference

`specs/design/user-defined-function-calls-design.md` describes the argument
checks against `FunctionSignature`; it gains a sentence that each parameter
declaration is a parameter of the signature and is compared by its declared
type id, including an inline type.

## File map

Prefactor PR:

- `compiler/analyzer/src/stages.rs` — move the `xform_resolve_decl_types`
  call.
- `compiler/analyzer/src/xform_resolve_expr_types/tests.rs` — same order in
  the test helper.

Core change PR:

- `compiler/analyzer/src/intermediate_type.rs` —
  `IntermediateFunctionParameter::type_id`, `param_type: Option<TypeName>`.
- `compiler/analyzer/src/xform_resolve_symbol_and_function_environment.rs` —
  keep every parameter, with its type id.
- `compiler/analyzer/src/value_type.rs` — expected type by id or generic
  name.
- `compiler/analyzer/src/rule_function_call_type_check.rs`,
  `rule_function_call_in_out_argument.rs`, `rule_constant_range.rs`,
  `xform_resolve_expr_types.rs`, `intermediates/operator_function_form.rs`,
  `function_environment.rs`, `intermediates/stdlib_*.rs`,
  `spec_conformance_keyword_function_forms.rs`,
  `xform_named_to_positional_args.rs` — follow the field changes.
- `compiler/codegen/src/compile_fn.rs` — passing width by id.
- `compiler/analyzer/src/rule_function_call_type_check/` and
  `xform_resolve_symbol_and_function_environment` tests — new cases.
- `specs/design/user-defined-function-calls-design.md` — one sentence.
- `docs/` — none: the change makes a valid program pass `check`; no
  documented behaviour changes.

## Tasks

### Prefactor PR: record declaration type ids before the function environment

- [ ] Move `xform_resolve_decl_types` before
      `xform_resolve_symbol_and_function_environment` in `stages.rs` and in
      the `xform_resolve_expr_types` test helper
- [ ] `cd compiler && just`

### Core change PR: function parameters of an inline type

- [ ] Failing tests first, named
      `function_when_condition_then_result`:
  - [ ] signature builder: inline array, inline enumeration and inline
        subrange parameters are kept, in declaration order, with their
        `type_id`, for `VAR_INPUT`, `VAR_OUTPUT` and `VAR_IN_OUT`
  - [ ] `check` of the program from #1952 has no diagnostics
  - [ ] an inline array input given a scalar argument, and given an array
        of another shape, reports P4026 with
        `expected=ARRAY[1..3] OF DINT`
  - [ ] a named argument to an inline enumeration input (#1946 point 2)
        binds, with no P4023/P4018
  - [ ] a `VAR_IN_OUT` inline subrange parameter: arity correct and the
        argument counts as written (not inferred CONSTANT)
  - [ ] codegen: a function with a `VAR_IN_OUT` inline subrange followed
        by a scalar input compiles and runs, each argument reaching its
        parameter
- [ ] Add `type_id`, make `param_type` optional, keep every parameter
- [ ] Expected type by id in `value_type::check`, the in-out rule, the
      constant range rule and `compile_user_function`
- [ ] Design doc sentence
- [ ] `cd compiler && just`, `cd specs && just`

## Out of scope

- Array (and structure) function inputs in code generation: #1977.
- An inline subrange in `VAR` or `VAR_INPUT` is a syntax error (#1966), so
  the inline subrange case is only reachable through `VAR_IN_OUT` today.
- Method call arguments are not type-checked against the method's
  parameters (a swapped `inst.m(3, arr)` passes `check`); that is a
  separate gap from this one, next to #1823.
- Return types and standard library parameters by `TypeId`: the rest of
  #1968 step 6a.
