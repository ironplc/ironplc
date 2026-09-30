# Plan: Bind Positional Arguments of a Function Block Call

## Goal

Issue #1855: a function block called with non-formal (positional) arguments,
`i(1, 2);`, passes `check` and `compile`, and code generation drops the
arguments, so the inputs keep their previous values. Implement the IEC 61131-3
non-formal call: each positional argument binds to the block's `VAR_INPUT` in
declaration order. Every call the compiler cannot bind that way is refused by
the analyzer, so `check` reports it; code generation only guards the invariant.

## Architecture

- **Analyzer.** `call_assignment_check::bind_inputs` already binds a positional
  argument to the `VAR_INPUT` at its position, and `check_assignments` already
  refuses a mixed call (P4001) and a positional count different from the
  number of inputs (P4003) for a user function block. Two gaps remain:
  - a standard-library function block (`TON`, `CTU`, ...) is skipped by
    `rule_function_block_invocation`, so `t(TRUE)` or `t(TRUE, T#1s, 3)` is
    accepted. The rule checks its positional arguments against the inputs of
    the block's type in the type environment, with the same P4001/P4003.
  - a user function block that declares `VAR_IN_OUT`: the standard puts
    `VAR_IN_OUT` in the non-formal order, but function block `VAR_IN_OUT` is
    not implemented (see `specs/design/var-in-out-parameters.md`) and the
    binding skips it, so `i(5)` binds `5` to an input that the standard would
    bind to the in-out. Refuse a positional call to such a block as not
    implemented (P9999) instead of binding it differently from the standard.
- **Codegen.** `compile_fb_call` stores only `NamedInput` arguments. It binds
  the `n`th positional argument to the `n`th `VAR_INPUT` field, from an
  ordered list of input names kept beside the field indices, for user and
  intrinsic blocks alike. A positional count other than the number of inputs
  is an internal error: the analyzer rejects it first.

## Prefactoring

- Codegen: `FbInstanceInfo::field_indices` / `UserFbTypeInfo::field_indices`
  are bare `HashMap<String, u8>`, and the intrinsic layouts are built by
  inserting names one by one, so the declaration order of the inputs is lost.
  Introduce an `FbFields` type built from the ordered input and output names;
  the positional binding then needs only one accessor.
- Analyzer: split the mixed-call check and the positional-count check out of
  `check_assignments` into functions of their own, so the standard-library
  path reuses them rather than restating them.
- Codegen tests: move `check_and_run` (full analysis, then run) from
  `end_to_end_user_function_in_out.rs` into `common/mod.rs`, since the new
  end-to-end file needs the same shape.

## Design doc reference

- `specs/design/function-block-infrastructure-design.md` (FB call emission)
- `specs/design/var-in-out-parameters.md` (status table)

## File map

- `compiler/codegen/src/compile.rs`, `compile_call.rs`, `compile_setup.rs`,
  `compile_stmt.rs`, `compile_expr.rs`, `compile_fb_init.rs`
- `compiler/analyzer/src/call_assignment_check.rs`,
  `rule_function_block_invocation.rs`
- `compiler/codegen/tests/it/common/mod.rs`,
  `end_to_end_user_function_in_out.rs`, new `end_to_end_fb_positional_args.rs`,
  `main.rs`
- `docs/reference/language/structured-text/function-call.rst`
- `specs/design/function-block-infrastructure-design.md`,
  `specs/design/var-in-out-parameters.md`

## Tasks

- [ ] Prefactor codegen `FbFields`
- [ ] Prefactor analyzer positional checks, and `check_and_run` in tests
- [ ] Analyzer tests (failing), then standard-library arity and the
      `VAR_IN_OUT` refusal
- [ ] End-to-end tests (failing), then the codegen binding and its guard
- [ ] Docs and design docs
- [ ] `git rm` this plan, `cd compiler && just`, `cd specs && just`, docs build
