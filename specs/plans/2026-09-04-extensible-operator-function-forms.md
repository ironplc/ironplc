# Extensible Operator Function Forms

Fixes [#1618](https://github.com/ironplc/ironplc/issues/1618).

## Goal

`ADD`, `MUL`, `AND`, `OR` and `XOR` accept two or more inputs, as IEC 61131-3
defines them (Tables 24 and 26 mark them *extensible*), so that
`b4 := AND(b1, b2, b3)` checks clean and computes `(b1 AND b2) AND b3`.

## Architecture

The function form of an operator is one row of `OPERATOR_FUNCTION_FORMS` in
`compiler/analyzer/src/intermediates/operator_function_form.rs`; the analyzer
derives the signature from the row and codegen compiles the call as the row's
operator. The row gains an **arity** column: unary (`NOT`), binary (`SUB`,
`GT`, ...) or extensible (`ADD`, `MUL`, `AND`, `OR`, `XOR`). An extensible
row registers through the same `FunctionSignature::stdlib_extensible`
mechanism `MUX` already uses, with no upper bound on the input count.

Everything the analyzer does with an extensible signature is shared with
`MUX` rather than re-implemented per function:

- **Argument count** — `rule_function_call_declared` already accepts at
  least the declared count for an extensible signature. No change.
- **Argument types** — `rule_function_call_type_check` stops checking at the
  declared parameter count, so a third argument to `MUX` (and, without this
  change, to `ADD`) is never checked. `FunctionSignature` gains
  `input_parameters()`, an iterator that continues past the declared list for
  an extensible signature by numbering on from the last declared input
  (`IN1`, `IN2` → `IN3`, `IN4`, ...), and the rule zips the arguments against
  it. That closes the gap for `MUX` too.
- **Named arguments** — `xform_named_to_positional_args` skips extensible
  signatures, which is why `MUX(K := 0, IN0 := a, IN1 := b)` reaches codegen
  with no positional arguments and fails to compile. With the same iterator
  the transform can place `IN3 := c` as well, so `ADD(IN1 := a, IN2 := b)`
  keeps working once `ADD` is extensible, and the `MUX` spelling starts to.

Codegen compiles an n-input call as the operator folded from the left:
`ADD(a, b, c)` emits exactly what `(a + b) + c` emits. The fold handles the
binary rows as the two-input case, so codegen does not need to know which
rows are extensible; the analyzer has already enforced the count.

Out of scope: IEC 61131-3 also marks `GT`, `GE`, `EQ`, `LE` and `LT` as
extensible, meaning a monotonic sequence (`GT(a, b, c)` is `a > b AND b > c`),
which is not a left fold and not what #1618 asks for. Those rows stay binary
and a three-input call stays P4018.

## Prefactoring

1. **`compile_call.rs` collects positional arguments four ways.**
   `compile_two_arg_operator`, the `NOT` arm of `compile_operator_form`,
   `compile_mux` and `extract_two_positional_args` each repeat the
   `filter_map` that `collect_positional_args` already provides. All four use
   the shared function, so the fold has one arg list to work on.
2. **The arity of a function form is implied by its operator.**
   `OperatorFunctionForm::signature` derives the parameter list by matching
   `FormOf::Not` against the rest. Making arity an explicit column of the row
   (`Arity::Unary`, `Arity::Binary`) is a shape-only change that the
   extensible variant then drops into as a third value.
3. **`FunctionSignature::stdlib_extensible` requires an upper bound.**
   `max_inputs` is already `Option<usize>` on the signature; the constructor
   takes the `Option` so a row without a natural bound can say so.

Each is behaviour-preserving and lands in its own commit ahead of the feature.

## Design doc reference

`specs/design/keyword-function-forms.md` — gains the extensible rows as
requirements (analyzer: n-input acceptance, binary rows stay binary, every
argument type-checked; codegen: the left fold).

## File map

| File | Change |
|---|---|
| `compiler/codegen/src/compile_call.rs` | Prefactor 1; fold-left `compile_operator_form` |
| `compiler/analyzer/src/intermediates/operator_function_form.rs` | Prefactor 2; `Arity::Extensible` on five rows |
| `compiler/analyzer/src/function_environment.rs` | Prefactor 3; `input_parameters()` |
| `compiler/analyzer/src/intermediates/stdlib_function.rs` | `MUX` passes `Some(17)` |
| `compiler/analyzer/src/rule_function_call_type_check.rs` | Check every argument of an extensible call |
| `compiler/analyzer/src/xform_named_to_positional_args.rs` | Place named arguments of an extensible call |
| `compiler/analyzer/src/rule_function_call_declared.rs` | Tests: three inputs accepted / rejected per row |
| `compiler/analyzer/src/spec_conformance_keyword_function_forms.rs` | Conformance tests for the new requirements |
| `compiler/codegen/src/spec_conformance_keyword_function_forms.rs` | Conformance test for the fold |
| `compiler/codegen/tests/it/end_to_end_func_forms_extensible.rs` | End-to-end results for n-input calls |
| `compiler/codegen/tests/it/main.rs` | Register the new test file |
| `specs/design/keyword-function-forms.md` | Requirements |
| `docs/reference/standard-library/functions/{add,mul,and,or,xor}.rst` | Two or more inputs |

## Tasks

- [ ] Prefactor: one positional-argument collector in `compile_call.rs`
- [ ] Prefactor: explicit `Arity` column on the operator-form row
- [ ] Prefactor: `stdlib_extensible` takes `Option<usize>`
- [ ] `FunctionSignature::input_parameters()` with unit tests
- [ ] Type-check every argument of an extensible call
- [ ] Named arguments on extensible calls
- [ ] Mark `ADD`, `MUL`, `AND`, `OR`, `XOR` extensible; codegen left fold
- [ ] Requirements in the design doc with conformance tests
- [ ] Docs for the five functions
- [ ] `cd compiler && just`; delete this plan
