# Plan: One definition for the function form of an operator

Fixes [#1567](https://github.com/ironplc/ironplc/issues/1567).

Delivered as two pull requests. PR 1 is a prefactor with no behaviour
change; PR 2 closes #1567 and is stacked on PR 1.

## Problem

Verified against `c2fc6eb` with `ironplcc check`:

1. **`AND(w1, w2)` on `WORD` is rejected.** Two P4026 diagnostics
   (`expected=BOOL, actual=word`), one per argument. The operator form
   `w1 AND w2` checks clean. Same for `OR` and `XOR`. (#1567)

2. **`NOT(IN := w1)` is rejected the same way.** `NOT(x)` parses as the
   unary operator applied to `(x)` and never consults the signature, but the
   named-argument spelling is a function call and does. Not in the issue.

3. **Widening the signature alone would turn defect 2 into a miscompile.**
   The codegen for the `NOT` function form (`compile_not_function` in
   `compiler/codegen/src/compile_call.rs`) always emits `BOOL_NOT`. The
   unary operator picks `BIT_NOT_32`/`BIT_NOT_64` for unsigned operand types
   and truncates to the operand's storage width. Once the analyzer accepts
   `NOT(IN := w1)`, the function form evaluates as a boolean complement of a
   `WORD`. Not in the issue.

The cause of defect 1 is four signatures in
`compiler/analyzer/src/intermediates/stdlib_function.rs` that say `BOOL`
where IEC 61131-3 says `ANY_BIT`. That is the fix, and it is four tokens.
The reason it happened, and the reason fixing four tokens leaves the next
one to be found the same way, is the shape around them:

- **Three hand-written copies say what the function form of an operator
  is**, and nothing ties them together. The parser lists the keywords that
  may be called (`function_name()`), the analyzer lists the signatures, and
  codegen lists the names and which emitter each maps to (16 arms in
  `compile_function_call`). The analyzer's copy states a family's category
  nowhere but a comment (`// All take BOOL inputs and return BOOL`), then
  restates it twelve times as string literals.
- **The return type is typed separately from the operands.** The expression
  type resolver infers a generic return type from the first parameter whose
  declared type equals it, so a family whose return and operands disagree
  resolves to the wrong type or to `None`, silently. The fix has to change
  the operands and the return in step; the shape does not make it do so.
- **The function form has its own codegen.** Defect 3 is the first
  prefactoring signal in
  [Development Standards](../steering/development-standards.md#signals-that-a-change-needs-prefactoring):
  the new behaviour needs a new arm in more than one place.

## Prefactoring (PR 1)

**A function form of an operator is one row in one table, and both the
analyzer and codegen read that row.** No behaviour change; the existing
suites pass unchanged.

### Analyzer

Replace `get_arithmetic_functions`, `get_comparison_functions` and
`get_boolean_functions` in `stdlib_function.rs` with a table of
`OperatorFunctionForm` rows and one builder:

| Column | Meaning |
|---|---|
| `name` | The function name (`ADD`, `GT`, `AND`, `NOT`, …) |
| `operator` | The DSL operator it is a form of: `Operator::Add`, `CompareOp::Gt`, `UnaryOp::Not`, … |
| `operands` | The category every operand has, stated once per row |
| `result` | `Operand` (the result is the operand type) or `Bool` |

The builder derives the parameter list (`IN` for unary, `IN1`/`IN2` for
binary) and the return type from the row. A family's category is now one
cell, and a return type that disagrees with its operands cannot be written.
The rows keep today's categories exactly, `AND`/`OR`/`XOR`/`NOT` at `BOOL`
included, because this PR changes shape and not behaviour. A table-driven
unit test pins every row's signature so the fix's diff shows the cells it
changes and nothing else.

`operator_function_form(name)` is exported from the crate so codegen can
ask which operator a name is a form of. `MOVE`, `EXPT`, the shift/rotate
functions and the time arithmetic functions are not operator forms in this
sense (they have no operator expression path in codegen, or a different
parameter shape) and stay as they are.

### Codegen

`compile_function_call` asks the analyzer's table before its own `match`.
A hit compiles the arguments and emits through the same dispatchers the
operator expression path uses: the existing `emit_add`…`emit_mod` for
`Operator`, `emit_eq`…`emit_xor` for `CompareOp`, and a new `emit_not`
extracted from the `UnaryOp::Not` arm of `compile_expr` for `UnaryOp::Not`,
so the operator and its function form share it. The 16 name arms and
`compile_not_function` go away.

For `BOOL` operands, `emit_not` reaches the same `BOOL_NOT` the deleted
function did (`BOOL` compiles as `W32`/`Signed`), so today's programs
produce today's bytecode. The function form keeps compiling its operands at
the enclosing expression's operation type, as it does now; whether it
should derive the type from the operands as the operator path does is a
separate question and is not changed here.

### What this does and does not prevent

After PR 1 the fact "`AND(…)` is the `AND` operator" exists once, and both
consumers read it. What remains hand-written is the category cell, sitting
beside `ADD … ANY_NUM` and `GT … ANY_ELEMENTARY` where `AND … BOOL` is
visibly the odd row out. PR 2 pins the cells against the standard with
requirement IDs, which is the mechanism this project uses to keep a
declared fact from drifting.

## Fix (PR 2)

- Change the `operands` cell of `AND`, `OR`, `XOR` and `NOT` to `ANY_BIT`.
  With `result: Operand`, `AND(w1, w2)` resolves to `WORD` with no further
  change.
- Add a *Signatures and code generation* section to
  [keyword-function-forms.md](../design/keyword-function-forms.md) with
  requirement IDs, one per family, and conformance tests in the analyzer
  and codegen crates; register the design doc in both crates' `build.rs`.
- Tests:
  - `rule_function_call_type_check`: `AND`/`OR`/`XOR` accept `BOOL`, `BYTE`,
    `WORD`, `DWORD`, `LWORD`; reject `INT` with P4026; `NOT(IN := w)` accepts
    `WORD`.
  - `xform_resolve_expr_types`: `AND(w1, w2)` resolves to `WORD`.
  - Codegen end-to-end: `AND`/`OR`/`XOR` on `WORD` give the bitwise result;
    `NOT(IN := w)` on `WORD` gives the 16-bit complement; `NOT(IN := b)` on
    `BOOL` still gives the boolean complement.
- Documentation: add `AND`, `OR`, `XOR` and `NOT` reference pages under
  `docs/reference/standard-library/functions/` and a *Bit String Functions*
  section in the index, following the existing `ADD` page.
- `git rm` this plan.

## Out of scope

Each of these is filed as an issue in the PR that deletes this plan:

- `AND`, `OR`, `XOR`, `ADD` and `MUL` are extensible in the standard (any
  number of inputs); IronPLC declares two.
- `MOD` is declared `ANY_NUM`; the standard says `ANY_INT`, and codegen
  already has no float arm for it.

## File map

PR 1:

- `compiler/analyzer/src/intermediates/stdlib_function.rs` — table, builder, lookup, tests
- `compiler/analyzer/src/lib.rs` — export the lookup
- `compiler/codegen/src/compile_call.rs` — dispatch through the table; delete the name arms and `compile_not_function`
- `compiler/codegen/src/compile_expr.rs` — extract `emit_not`

PR 2:

- `compiler/analyzer/src/intermediates/stdlib_function.rs` — four cells
- `compiler/analyzer/src/rule_function_call_type_check.rs`, `xform_resolve_expr_types.rs` — tests
- `compiler/analyzer/src/spec_conformance_keyword_function_forms.rs`, `compiler/analyzer/build.rs`
- `compiler/codegen/src/spec_conformance_keyword_function_forms.rs`, `compiler/codegen/build.rs`
- `compiler/codegen/tests/it/end_to_end_func_forms.rs` — `WORD` cases
- `specs/design/keyword-function-forms.md`
- `docs/reference/standard-library/functions/{and,or,xor,not}.rst`, `index.rst`

## Tasks

PR 1:

- [ ] Table, builder and lookup in `stdlib_function.rs`; table-driven test pinning every row
- [ ] Export the lookup from the analyzer crate
- [ ] Extract `emit_not`; route function forms through the table in `compile_call.rs`
- [ ] `cd compiler && just` passes; open the PR

PR 2:

- [ ] Change the four cells
- [ ] Design doc section with requirement IDs; conformance tests; `build.rs` registration
- [ ] Analyzer rule and resolver tests
- [ ] Codegen end-to-end tests
- [ ] Docs pages and index section
- [ ] File the out-of-scope issues; `git rm` the plan
- [ ] `cd compiler && just` and `cd docs && just ci` pass; open the PR
