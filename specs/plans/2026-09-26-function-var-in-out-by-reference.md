# Plan: Function `VAR_IN_OUT` parameters passed by reference

Issue: [#1658](https://github.com/ironplc/ironplc/issues/1658)

## Goal

A user `FUNCTION` that declares `VAR_IN_OUT` parameters can be called, and a
write to the parameter inside the function changes the caller's variable.

Today both halves are wrong:

- The analyzer drops `VAR_IN_OUT` from a function's argument list, so every call
  is refused with a false P4018 (arity), and P4026 checks arguments against the
  wrong parameter from the first `VAR_IN_OUT` onward.
- Codegen passes `VAR_IN_OUT` by value: the argument's value is copied into the
  parameter slot and never written back, so `data := data + n` inside the
  function has no effect on the caller. The end-to-end test that "passes"
  never checks the caller's variable.

## Architecture

`VAR_IN_OUT` is pass-by-reference, and IronPLC already has references
(`REF_TO`, see [ref-to.md](../design/ref-to.md)): a reference is a 64-bit
variable-table index, dereferenced with `LOAD_INDIRECT`/`STORE_INDIRECT`. A
`VAR_IN_OUT` parameter is compiled as an implicit, non-null, non-reassignable
reference:

- **Call site**: the argument must be a variable. The caller pushes the
  variable's table index (exactly what `REF(x)` pushes) instead of its value.
  If the argument is itself a `VAR_IN_OUT` parameter of the calling function,
  the caller forwards the reference it holds.
- **Callee**: the parameter slot holds the reference. A read of the parameter
  is `LOAD_VAR_I64 slot; LOAD_INDIRECT`; a write is `<value>; <truncate>;
  LOAD_VAR_I64 slot; STORE_INDIRECT`. `REF(param)` yields the reference held in
  the slot (the caller's variable), not the slot's own index.

No new opcodes and no container format change: this is only a different use
of existing instructions, so the VM's null and bounds checks apply unchanged.

The analyzer enforces what makes this sound:

- A `VAR_IN_OUT` argument must be a variable (new **P4057**); an expression or
  literal has no address.
- A `VAR_IN_OUT` argument's type must equal the parameter's type (new
  **P4058**), not merely be implicitly convertible: the callee writes through
  the reference at the parameter's width, so an `INT` variable bound to a
  `DINT` parameter would receive values `INT` cannot hold. P4026 stops
  checking `VAR_IN_OUT` arguments so a mismatch is reported once.

### Scope of this change

In scope: user `FUNCTION`s whose `VAR_IN_OUT` parameters have an elementary
type (integers, reals, bit strings, `BOOL`, time and date types), bound to a
named variable.

Out of scope, reported by codegen as not implemented rather than compiled
with by-value semantics (follow-up issue to be opened):

- `VAR_IN_OUT` of `STRING`/`WSTRING`, arrays, structures, `REF_TO` and
  function-block instances on functions (these live in the data region, not in
  one slot)
- a `VAR_IN_OUT` argument that is an array element or structure field
- `VAR_IN_OUT` on function blocks and methods (today by value, too)
- bit/partial access writes, `FOR` control variables and `=>` output targets
  naming a `VAR_IN_OUT` parameter

## Prefactoring

1. **Analyzer**: `FunctionSignature::input_parameters` /
   `input_parameter_count` iterate `is_input_compatible()` parameters, so the
   signature, `xform_named_to_positional_args`, codegen and the arity check all
   agree on one argument list. This *is* the issue's bug fix, and every other
   change builds on it, so it lands first with its tests.
2. **Codegen**: replace `UserFunctionInfo::param_string_info:
   Vec<Option<StringParamInfo>>` with `param_passing: Vec<ParamPassing>`, an
   enum with `Value` and `String(StringParamInfo)` variants. The call site
   matches on it rather than testing `Option`s, so by-reference passing is one
   new variant rather than a second parallel vector. Also derive the parameter
   op types from `input_parameters()` rather than re-filtering `is_input`,
   which has the same misbinding bug as the analyzer.

Behaviour is unchanged by each prefactor commit.

## Design doc reference

- [ref-to.md](../design/ref-to.md) — reference representation and runtime
  safety; gains a section on `VAR_IN_OUT` as an implicit reference.
- [user-defined-function-calls-design.md](../design/user-defined-function-calls-design.md)
  — calling convention; gains the by-reference argument.

## File map

- `compiler/analyzer/src/function_environment.rs` — input list fix
- `compiler/analyzer/src/rule_function_call_type_check.rs` — skip in/out
- `compiler/analyzer/src/rule_function_call_in_out_argument.rs` (new) — P4057, P4058
- `compiler/analyzer/src/stages.rs`, `lib.rs` — register the rule
- `compiler/problems/resources/problem-codes.csv` — P4057, P4058
- `docs/reference/compiler/problems/P4057.rst`, `P4058.rst` (new)
- `docs/reference/compiler/problems/P4054.rst`, `docs/reference/language/pous/function.rst` — drop "not supported" text
- `compiler/codegen/src/compile.rs` — `ParamPassing`, `CompileContext::in_out_params`
- `compiler/codegen/src/compile_fn.rs` — mark in/out params, reject unsupported types
- `compiler/codegen/src/compile_call.rs` — pass references
- `compiler/codegen/src/compile_array.rs` — `ResolvedAccess::InOut`
- `compiler/codegen/src/compile_expr.rs`, `compile_stmt.rs` — indirect read/write, refuse unsupported sites
- `compiler/codegen/tests/it/end_to_end_user_function.rs` — write-back tests
- `specs/design/ref-to.md`, `specs/design/user-defined-function-calls-design.md`

## Tasks

- [ ] Commit this plan
- [ ] Prefactor 1: analyzer input list includes `VAR_IN_OUT`; tests from the issue (both orders, positional and named, P4026 naming the right parameter)
- [ ] Prefactor 2: codegen `ParamPassing` enum; parameter op types from `input_parameters()`
- [ ] P4057/P4058 rule, problem codes and docs; P4026 skips in/out
- [ ] Codegen: in/out parameters as references (call site, reads, writes, `REF`, forwarding); refuse unsupported types and sites
- [ ] End-to-end tests asserting the caller's variable changes; forwarding through two functions; aliasing two in/out params to one variable
- [ ] Update docs and design docs
- [ ] Open follow-up issue for out-of-scope items
- [ ] `git rm` this plan
- [ ] `cd compiler && just`
