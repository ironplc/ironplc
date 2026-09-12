# Let functions with VAR_IN_OUT be called

## Goal

A function that declares a `VAR_IN_OUT` parameter cannot be called at all.
The arity check counts only `VAR_INPUT`, so every call looks like it passed
too many arguments:

```iecst
FUNCTION ADD_TEN : DINT
  VAR_IN_OUT data : DINT; END_VAR
  data := data + 10;
  ADD_TEN := data;
END_FUNCTION
...
  result := ADD_TEN(data := x);
```

```
error[P4018]: Function call has wrong number of arguments
              (function=ADD_TEN, expected=0, actual=1)
```

`expected=0` for a function with one parameter. The feature is declared,
parsed, and lowered by codegen, but unreachable from source.

## Architecture

`IntermediateFunctionParameter` already records `is_input`, `is_output` and
`is_inout` separately, and already carries the predicate this needs:

```rust
/// Returns true if this parameter receives a value from the caller
/// (either VAR_INPUT or VAR_IN_OUT).
pub fn is_input_compatible(&self) -> bool {
    self.is_input || self.is_inout
}
```

`xform_named_to_positional_args` already uses it, which is why the named
form `ADD_TEN(data := x)` gets as far as the arity check at all. Two places
in `function_environment.rs` use the narrower `is_input` instead:

- `input_parameter_count()` — what the arity check compares against.
- `input_parameters()` — what positional arguments bind against.

Both must move together. Counting `VAR_IN_OUT` without binding it would let
the call through and then never type-check the argument.

Nothing in codegen changes: it already lowers `VAR_IN_OUT` (the end-to-end
tests below assert the right runtime values), it was simply never reached.

## Prefactoring

The two functions duplicate the `filter(|p| p.is_input)` predicate, so the
fix would have to be made twice and could be made inconsistently — which is
the failure mode being fixed.

Extract the declared-input iterator both share, so the predicate lives in
one place and the change lands in one line. Committed on its own with the
predicate unchanged, so it is provably behaviour-preserving.

## The blind spot, in miniature

The three end-to-end tests for this feature **pass on `main`** while
`ironplcc` rejects the very same program:

```
$ cargo test -p ironplc-codegen --test it inout     # 3 passed
$ ironplcc check inout.st                           # error[P4018]
```

Codegen tests do not run the semantic rules (the gap #1663 closes), so they
never saw the rejection. The fix therefore needs **analyzer** rule tests to
prove the call is accepted; the codegen tests cannot show it, and would not
have caught the regression that introduced it.

## Design doc reference

None. `is_input_compatible`'s doc comment is the statement of intent.

## File map

| File | Change |
| ---- | ------ |
| `compiler/analyzer/src/function_environment.rs` | Prefactor the shared iterator; then count and bind `VAR_IN_OUT` |
| `compiler/analyzer/src/rule_function_call_declared.rs` (tests) | Accept a call to a function with `VAR_IN_OUT`, alone and beside a `VAR_INPUT` |
| `docs/reference/compiler/problems/P4018.rst` | Only if it claims inputs are `VAR_INPUT`-only |

## Tasks

- [ ] Prefactor: extract the shared declared-input iterator; no behaviour change
- [ ] Count and bind `VAR_IN_OUT` via `is_input_compatible()`
- [ ] Rule tests: `VAR_IN_OUT` alone; `VAR_INPUT` + `VAR_IN_OUT` in declaration
      order; wrong arity still reported
- [ ] Confirm end to end that `ironplcc` compiles and the VM returns 15
- [ ] Check `P4018.rst` for a now-wrong claim
- [ ] Full CI: `cd compiler && just` and `cd specs && just`
- [ ] `git rm` this plan
