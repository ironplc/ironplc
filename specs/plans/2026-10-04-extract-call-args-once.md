# Plan: Extract and Count-Check Call Arguments Once

Checked against `main` at `7951b20`; line numbers are as of that commit.

## Goal

Make the standard-function compilers in `compiler/codegen` take their operands
instead of the AST call node. Today every one of them takes `&Function`, pulls
the arguments out itself, and re-checks the count that analysis
(`rule_function_call_declared`) has already enforced. After this change the
dispatcher, `compile_intrinsic`, extracts and count-checks the arguments once,
and each fixed-arity leaf compiler receives `args: [&Expr; N]`.

This is a **behaviour-preserving prefactor**. It delivers one PR, the
prefactor PR. There is no core change PR: the change it prepares for is not
on `main`.

## Why

[ironplc/ironplc#2011](https://github.com/ironplc/ironplc/pull/2011) is an
unmerged design that splits codegen into a lowering stage and a backend. Its
backend receives already-resolved argument lists, not the AST. If the leaf
compilers keep taking `&Function`, that split has to rewrite each of the roughly twenty fixed-arity ones. If they
take operands, the split replaces only the dispatcher's argument extraction.

That design is not on `main`, so neither the code nor the docs in the
prefactor PR may refer to it. This plan is the only place it is named, and a
plan is never merged (see
[Development Process](../steering/development-standards.md#development-process)).

The prefactor stands on its own merits as well: `collect_positional_args` is
defined twice, and the same five-line `args.len() != N` check is written 17
times, with a slice-pattern variant in `FormOf::Not` and a helper for N=2.

## Prefactoring

This plan *is* the prefactor, so there is nothing to prefactor before it. Two
signals from the development standards apply, and both are why it is one PR:

- "You would copy an existing function and change a few lines": the second
  `collect_positional_args` is exactly that.
- "A similar bug could occur rather than being prevented at compile time": a
  leaf compiler can check `args.len() != 3` and then index `args[3]`. With
  `[&Expr; N]` the arity is in the type.

Not done, to stay mechanical (see
[When *not* to prefactor](../steering/development-standards.md#when-not-to-prefactor)):
the variadic sites do not move to `&[&Expr]` operands, and `CompileContext`,
`ParamPassing` and the analyzer are untouched.

## Architecture

### The new module: `compiler/codegen/src/call_args.rs`

Owns everything about getting arguments out of a `Function`:

```rust
/// The positional input arguments of a call.
pub(crate) fn collect_positional_args(func: &Function) -> Vec<&Expr>;

/// The error for a call whose argument count its signature does not allow.
#[track_caller]
pub(crate) fn wrong_arg_count(func: &Function) -> Diagnostic;

/// The `N` positional arguments of a call, or P9998 for any other count.
#[track_caller]
pub(crate) fn fixed_args<const N: usize>(func: &Function) -> Result<[&Expr; N], Diagnostic>;
```

Bodies of the first two move unchanged. `fixed_args` is `collect_positional_args`
converted with `<[&Expr; N]>::try_from(vec)` and mapped to `wrong_arg_count(func)`
when it fails. It replaces `extract_two_positional_args` as `fixed_args::<2>`.
It allocates one `Vec`, as the sites it replaces do.

The module doc states the rule for the next reader: a fixed-arity compiler
takes operands, and only the dispatcher and the sites that match arguments
against a runtime-sized or per-parameter shape call `collect_positional_args`.

**`#[track_caller]` and closures.** `Diagnostic::internal_error_at` records
`Location::caller()`. `#[track_caller]` does not pass through a closure, so
`fixed_args` must not be written as `.try_into().map_err(|_| wrong_arg_count(func))`:
the recorded location would be that closure inside `call_args.rs`. It is written
with `let ... else { return Err(wrong_arg_count(func)) }`, which keeps the chain
`wrong_arg_count` → `fixed_args` → call site. The location test below exists to
catch this.

### The dispatcher

`compile_intrinsic` calls `fixed_args(func)?` in each fixed-arity arm. `N` is
inferred from the leaf's parameter type, so no arm spells it out:

```rust
Intrinsic::Move => compile_move(emitter, ctx, fixed_args(func)?, op_type),
Intrinsic::String(StringFunction::Len) => {
    compile_len(emitter, ctx, fixed_args(func)?, &func.name.span())
}
```

A leaf that used `func.name.span()` for a diagnostic or for
`resolve_string_arg` gets `span: &SourceSpan`, built in its arm. It is built
per arm rather than once at the top of `compile_intrinsic`, so calls that do not
need it (operators, numeric functions) still do not clone a span.

### Leaf signatures

| Leaf | File | N | Takes `span` | Notes |
|---|---|---|---|---|
| `compile_len` | `compile_string.rs` | 1 | yes | |
| `compile_find` | `compile_string.rs` | 2 | yes | |
| `compile_concat` | `compile_string.rs` | 2 | yes | |
| `compile_insert` | `compile_string.rs` | 3 | yes | |
| `compile_replace` | `compile_string.rs` | 4 | yes | |
| `compile_left`, `compile_right` | `compile_string.rs` | 2 | yes | wrap `compile_string_2arg` |
| `compile_delete`, `compile_mid` | `compile_string.rs` | 3 | yes | wrap `compile_string_3arg` |
| `compile_string_2arg`, `compile_string_3arg` | `compile_string.rs` | 2, 3 | yes | |
| `compile_move` | `compile_call.rs` | 1 | no | |
| `compile_trunc` | `compile_call.rs` | 1 | no | |
| `compile_sizeof` | `compile_call.rs` | 1 | no | |
| `compile_dt_to_date`, `compile_dt_to_tod` | `compile_call.rs` | 1 | no | |
| `compile_bcd_to_int`, `compile_int_to_bcd` | `compile_call.rs` | 1 | yes | span is for `todo_with_span` |
| `compile_conversion` | `compile_call.rs` | 1 | yes | span is for the `type_info` `todo_with_span`; passed on |
| `compile_type_conversion` | `compile_call.rs` | 1 | yes | |
| `compile_string_conversion` | `compile_call.rs` | 1 | yes | |
| `compile_shift_rotate` | `compile_builtin.rs` | 2 | no | |

The bodies keep their `args[0]`, `args[1]` indexing. Indexing a `[&Expr; N]`
with a constant is checked at compile time, and leaving the bodies alone keeps
the diff to signatures and deleted count checks.

### What stays on the variadic helper, and why

These keep `&Function` and call `collect_positional_args` (now from
`call_args`). Their imports change; their bodies do not.

| Site | Why it stays variadic |
|---|---|
| `compile_user_function_call` (`compile_call.rs:135`) | Matches each argument to its `ParamPassing`; the count is the user function's, not the intrinsic's. |
| `compile_mux` (`compile_call.rs:604`) | `args.len() >= 3`, and the count picks the opcode (`MUX_*_BASE + n`); also bounded by `MUX_MAX_INPUTS`. |
| `compile_left_fold` (`compile_call.rs:316`) | Two or more arguments; reached from the arithmetic fold and from the extensible comparison forms. |
| `compile_arith_fold` (`compile_arith.rs:99`) | Same fold, plus typed-overload resolution over the first two arguments. |
| `compile_numeric` (`compile_builtin.rs:178`) | The expected count is `opcode::builtin::arg_count(func_id)`, a runtime value from the opcode table. Its `lookup_builtin` `todo_with_span` also precedes the count check, so moving the count would reorder them. |
| `string_function_shape` (`string_width.rs:274`, `:294`) | Sizes the string a call yields from its first argument (and the second for CONCAT/INSERT/REPLACE). It does not compile the call, may be asked about a call nested as an argument, and reports a missing argument as its own diagnostic (a test feeds it a call with none). |

### `compile_operator_form`: a second level of dispatch

`compile_operator_form` is the match on `FormOf` that `compile_intrinsic` hands
off to. It has two variadic arms (`Arithmetic`, and `Compare` for the
non-comparison operators) and two fixed ones that the task does not list:
`Compare` for the comparison operators (already `extract_two_positional_args`,
becomes `fixed_args::<2>`) and `Not` (an inline `[term]` match with its own
`wrong_arg_count`, becomes `fixed_args::<1>`).

It stays as it is, taking `&Function`, and calls `fixed_args` for those two arms.
Treat it as part of the dispatcher: it is the one other place the later split
replaces. The alternative, inlining the `FormOf` match into `compile_intrinsic`,
makes the dispatcher harder to read for no gain. This is the first question for
the reviewer.

### Behaviour-preservation

Emitted bytecode is unchanged: every leaf receives the same expressions in the
same order and emits the same instructions. Two differences are visible, both
only on a path analysis has already rejected, which no compiled program reaches:

1. **Recorded Rust location.** A P9998 for a wrong count records the
   `file!()`/`line!()` of the `fixed_args` call, which is now an arm of
   `compile_intrinsic`, instead of the leaf compiler's `wrong_arg_count` call.
   For the string and shift/rotate leaves that also moves the recorded file
   from `compile_string.rs`/`compile_builtin.rs` to `compile_call.rs`. The
   code, the message and the label span are unchanged. This is inherent to
   extracting in the dispatcher.
2. **Order of two errors in one call.** Only `compile_conversion` computes
   something fallible before its count check: `type_info`, which can return a
   `todo_with_span`. A call that has the wrong count *and* a type codegen cannot
   place now returns P9998 first instead of the `todo_with_span`. Every other
   converted leaf checked the count first, so for them nothing moves.

No existing test depends on either. The P9998 assertions in `tests/it` are about
time-literal widths, `CASE` and `CONTINUE`; the one test that feeds a call the
wrong number of arguments (`string_expr_shape_when_string_function_has_no_arguments_then_internal_error`)
goes through `string_function_shape`, which stays variadic. If a reviewer would rather keep (2)
exactly, the conversion arm can call `fixed_args` after `type_info` at the cost
of a fixed-arity leaf that takes `&Function`; this plan does not do that.

## Design doc reference

None. The change is internal to `compiler/codegen`, adds no requirement, and no
file under `specs/`, `docs/` or `CLAUDE.md` mentions the moved helpers (the one
`wrong_arg_count` hit in `specs/design/bytecode-verifier-rules.md` is a VM
verifier reason string, unrelated). The rationale that should outlive this plan
goes in the `call_args.rs` module doc, where its reader is.

## File map

| File | Change |
|---|---|
| `compiler/codegen/src/call_args.rs` | **New.** The three functions above, module doc, unit tests. |
| `compiler/codegen/src/lib.rs` | `mod call_args;` (alphabetical, before `call_graph`). |
| `compiler/codegen/src/compile_call.rs` | Delete `collect_positional_args`, `wrong_arg_count`, `extract_two_positional_args`. Convert the leaves in the table. `compile_intrinsic` and `compile_operator_form` call `fixed_args`. Drop the now-unused `ParamAssignmentKind` import. Must end **below** its current 989 lines. |
| `compiler/codegen/src/compile_string.rs` | Delete `collect_positional_args` and the `wrong_arg_count` import. Convert the leaves. Drop the unused `Function` and `ParamAssignmentKind` imports. |
| `compiler/codegen/src/compile_builtin.rs` | Convert `compile_shift_rotate`; `compile_numeric` re-imports from `call_args`. |
| `compiler/codegen/src/compile_arith.rs` | Import `collect_positional_args` from `call_args`. |
| `compiler/codegen/src/string_width.rs` | Import `collect_positional_args` from `call_args`. |

No change to `compile.rs`, `CompileContext`, `ParamPassing`, the analyzer, the
problem registry, `docs/` or the VS Code extension.

## Tasks

The prefactor PR is one PR, branched from `main`, in four commits so each step
can be read on its own. Every commit builds and passes `cargo clippy` on its own.

### Prefactor PR: take operands in the standard-function compilers

- [ ] **Before editing:** build `ironplcc` on clean `main` and keep a copy as the
  "before" binary for the bytecode comparison below.
- [ ] **Commit 1: one definition.** Add `call_args.rs` with
  `collect_positional_args`, `wrong_arg_count`, `fixed_args` and the tests.
  Delete both existing `collect_positional_args` definitions and the old
  `wrong_arg_count`; repoint the imports in `compile_call.rs`,
  `compile_string.rs`, `compile_builtin.rs`, `compile_arith.rs` and
  `string_width.rs`. Replace `extract_two_positional_args` with `fixed_args::<2>`
  (its two callers: the `Time` arm and the comparison arm).
- [ ] **Commit 2: string functions.** Convert the `compile_string.rs` leaves and
  their nine arms in `compile_intrinsic`.
- [ ] **Commit 3: `compile_call.rs` leaves.** Convert `compile_move`,
  `compile_trunc`, `compile_sizeof`, `compile_dt_to_date`, `compile_dt_to_tod`,
  `compile_bcd_to_int`, `compile_int_to_bcd`, the conversions, and the `Not` arm.
- [ ] **Commit 4: shift and rotate.** Convert `compile_shift_rotate` and its arm.
- [ ] Remove the dead `args.len()` checks and `wrong_arg_count` returns at every
  converted site (done in each commit above, with the leaf they belong to).
- [ ] Confirm `grep -rn "fn collect_positional_args" compiler/codegen/src` finds
  one definition, and that no function other than the dispatcher, the sites in
  the table above and `compile_operator_form` takes a `&Function` to extract
  arguments.
- [ ] Confirm `wc -l compiler/codegen/src/compile_call.rs` is below 989.

### Unit tests (in `call_args.rs`)

New tests for `fixed_args` only; no existing test changes. They build a
`Function` the way the `string_width.rs` tests do (`Id::from`,
`ParamAssignmentKind::PositionalInput`), with `rstest` for the counts.

- [ ] `fixed_args_when_count_matches_then_returns_arguments_in_order`, for
  `N` = 1, 2 and 4.
- [ ] `fixed_args_when_count_differs_then_p9998`, for too few and too many.
- [ ] `fixed_args_when_count_differs_then_error_location_is_the_caller`: record
  `line!()` immediately before the call and assert `source_line` equals it. The
  helper and the test share a file, so the line, not the file, is what proves
  the location is the caller's and not a line inside `call_args.rs`. This covers
  `wrong_arg_count` too: if either function lost `#[track_caller]`, or `fixed_args`
  used a closure, the line would fall inside `call_args.rs`.

### Verification

- [ ] `cd compiler && just` passes, including the 85% coverage gate. A baseline
  run on clean `main` is being taken first so a failure can be attributed.
- [ ] `cd specs && just` passes (plan-citation check: nothing outside
  `specs/plans/` names this file).
- [ ] `compiler/codegen/tests/it` passes with **no** edits to its sources or
  assertions (`git diff main -- compiler/codegen/tests` is empty).
- [ ] **Bytecode is identical.** The end-to-end tests check behaviour, not
  bytes, so also compile a throwaway program (kept in the scratchpad, not
  committed) with the before and after `ironplcc` and compare the `.iplc`
  files byte for byte. It calls every converted function once: LEN, FIND,
  REPLACE, INSERT, DELETE, LEFT, RIGHT, MID, CONCAT, MOVE, TRUNC, SIZEOF,
  BCD_TO_INT, INT_TO_BCD, an int-to-real, an int-to-bool, an `*_TO_STRING`, a
  `STRING_TO_*`, DT_TO_DATE, DT_TO_TOD, `NOT(x)`, `LT(a, b)`, `ADD_TIME`, SHL and
  ROL. It also calls the variadic ones, so they are shown unchanged: MUX,
  `ADD(a, b, c)`, `AND(a, b, c)`, `SEL`, `SQRT`, and a user-defined function
  with a STRING and a `VAR_IN_OUT` parameter. It first confirms that two
  compiles with the *same* binary are byte-identical, so a difference between
  binaries is not timestamp noise. The `compiler/resources/test/*.st` programs
  go through the same comparison.
- [ ] Re-read the diff for anything that is not mechanical: a changed opcode, a
  changed message, a touched `todo_with_span` or other error path unrelated to
  argument counts.

### PR description

States which sites stayed variadic and why (the table above), the two
differences in "Behaviour-preservation", and the outcome of the bytecode
comparison. It does not name the design in #2011.

## Risks

- **Rebase churn in `compile_call.rs`.** The leaves sit in one file that other
  work edits. Mitigated by keeping bodies untouched and the PR small.
- **A leaf that matched `args` by slice pattern.** Only `FormOf::Not` does
  (`let [term] = args.as_slice()`); it becomes `let [term] = fixed_args::<1>(func)?`.
- **Two errors, one call.** See "Behaviour-preservation", item 2.

## Questions for the reviewer

1. Is `compile_operator_form` rightly kept as a second level of dispatch that
   calls `fixed_args` itself, or should the `FormOf` match move into
   `compile_intrinsic`?
2. Are the two differences listed under "Behaviour-preservation" acceptable for a
   prefactor? Neither is reachable through analysis.
3. The prefactor PR cannot use this plan's branch, which holds a plan that is
   never merged. It needs its own branch from `main`.
