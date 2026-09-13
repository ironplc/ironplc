# Check Standard-Library Function Return Types

Fixes [#1709](https://github.com/ironplc/ironplc/issues/1709): two test names
contradict what their bodies assert. Resolving the first of them turns out to
require a behaviour change, because the name describes behaviour the compiler
should no longer have.

## Goal

Remove the `signature.is_stdlib()` early return in `check_return_type`
(`compiler/analyzer/src/rule_function_call_type_check.rs:107`) so that P4027
applies to standard-library calls, then correct the two test names in #1709.

## Why the guard goes rather than gets a test

The guard was a deliberate deferral, not an oversight.
`specs/plans/assignment-and-stdlib-arg-type-check.md` — the plan that made
stdlib **arguments** checked — lists under "Out of scope":

> Return-type checking for stdlib function calls (the argument check already
> covers the reported cases and avoids inference-quirk false positives).

The "inference-quirk false positives" concern no longer applies. Generic
return types are resolved to a concrete type before this rule runs:
`xform_resolve_expr_types.rs:292-326` infers a generic return (`ANY_NUM`,
`ANY_REAL`, …) from the first argument whose parameter type matches the
return type, and where it cannot, `resolved_type` stays `None` and
`check_return_type` skips the call anyway. Evidence gathered before writing
this plan, with the guard deleted:

- The full workspace test suite passes — ~4,500 tests, zero failures.
- All 47 `.st` programs in the repository that compile today still compile.
- `ADD`, `MIN`, `MAX`, `LIMIT`, `SEL`, `MUX`, `MOVE`, `SQRT`, `ABS`, `GT`,
  `AND`, `TRUNC` and the conversion functions all resolve to a concrete
  return type and produce no diagnostic when assigned to a matching target.

Meanwhile the guard suppresses a real diagnostic. This compiles silently
today and is exactly the shape P4027 exists to catch:

```iecst
VAR r : INT; x : INT; END_VAR
r := INT_TO_REAL(x);   (* REAL value stored in an INT variable *)
```

The same program with a user-defined `REAL`-returning function is P4027.

## Prefactor

With the guard gone, `check_return_type` no longer needs the signature at
all — it reads only the call's span and name for the diagnostic, and the
already-resolved `value.resolved_type`. Drop the
`self.context.functions.get(&func_call.name)` lookup, which removes a
nesting level and the now-unused binding.

This is safe for a call to a function that is not in the environment:
`xform_resolve_expr_types.rs:293` returns early for an unknown name, so such
a call has `resolved_type == None` and is skipped by the existing
`if let Some(ref return_type)` guard.

## Scope

1. `compiler/analyzer/src/rule_function_call_type_check.rs`
   - Prefactor `check_return_type` as above (own commit).
   - Delete the `is_stdlib()` early return.
   - Rename `apply_when_stdlib_function_then_skipped` to
     `apply_when_stdlib_arg_matches_param_then_ok`, per #1709.
   - Add a `rule_ctx_err1!` case asserting P4027 for a stdlib return type
     assigned to an incompatible target, and a `rule_ctx_ok!` case locking in
     that integer widening of a stdlib return is still accepted (ADR-0029,
     ADR-0031).

2. `compiler/ironplc-cli/src/lsp_project.rs`
   - `tokenize_when_first_steps_then_has_tokens` asserts only `is_ok()`.
     Make it check the actual tokens: reconstruct each token's absolute
     position from the LSP deltas and assert the span lands on a real lexeme
     in `first_steps.st`, that every `token_type` is within
     `TOKEN_TYPE_LEGEND`, and anchor the first and last tokens. The existing
     token-content tests (`:855`, `:1010`) only cover two-line synthetic
     snippets, so nothing checks the tokenizer against a real program.

3. `docs/reference/compiler/problems/P4027.rst`
   - Note that the check applies to standard-library functions and give the
     `INT_TO_REAL` example, since this is the form users will hit.

## Compatibility

This makes the compiler stricter: source that stored a standard-library
call's result in a narrower or otherwise incompatible variable compiled
before and is now P4027. The fix is the explicit conversion the error already
recommends. No program in the repository's corpus is affected.

## Out of scope

`ADD(t1, t2)` on `TIME` operands is rejected with P4026 (`expected=ANY_NUM,
actual=time`). This reproduces on unmodified `main`, is in the **argument**
check rather than the return check, and is a signature-modelling gap —
IEC 61131-3 defines `ADD` over `TIME`. It needs its own issue.

## Verification

`cd compiler && just` (compile, coverage ≥ 85%, clippy, fmt).
