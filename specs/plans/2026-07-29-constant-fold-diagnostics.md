# Plan: Diagnose unevaluable constant expressions (issue #1249)

## Problem

`compiler/analyzer/src/constant_folding.rs` (`fold_integer_binary`,
`fold_real_binary`) collapses two distinct situations into one signal:

- "operands aren't both constant literals" (genuinely not foldable, no
  diagnostic needed -- the expression may still be valid at runtime)
- "operands are both constant literals, but evaluating the operation is
  impossible" (real division/modulo by zero silently folds to `inf`/`NaN`;
  integer overflow and integer division/modulo by zero return `None`, which
  either silently passes the unfolded node through with no diagnostic
  (`xform_fold_constant_expressions`, statement bodies) or gets reported
  under the wrong code -- P4037 "not a constant expression" -- in
  `xform_fold_initializer_expressions`, even though the expression *is*
  constant, it just doesn't evaluate)

## Design

Give the leaf arithmetic functions a three-way outcome instead of
`Option`:

```rust
pub(crate) enum FoldError {
    DivisionByZero,
    Overflow,
}
```

- `fold_integer_binary`/`fold_real_binary` return `Result<T, FoldError>`.
  Both operands being constant is already established by the caller
  (`try_fold_binary`) before these are invoked, so every path through them
  now either succeeds or names a `FoldError` -- there is no more silent
  `None`.
- Integer `Div`/`Mod` by zero -> `FoldError::DivisionByZero`. Real
  `Div`/`Mod` by zero -> `FoldError::DivisionByZero` (checked explicitly;
  `f64` doesn't panic or return `None` on its own).
  `checked_add`/`checked_sub`/`checked_mul`/`checked_div`/`checked_rem`/
  `checked_pow` returning `None` for a non-zero-divisor reason ->
  `FoldError::Overflow`.
- Integer `Pow` with a negative exponent keeps today's behavior exactly:
  `try_fold_binary` special-cases it and returns `Ok(None)` (leave
  unfolded, no diagnostic) *before* calling `fold_integer_binary` --
  out of scope for this issue, not a regression risk.
- `try_fold_binary`/`try_fold_unary` return `Result<Option<ExprKind>,
  FoldError>` (unary negation cannot fail today, so it stays infallible
  internally and is wrapped as `Ok`).
- Add `fold_error_to_diagnostic(FoldError, SourceSpan) -> Diagnostic` in
  `constant_folding.rs` (it already encodes evaluation policy per the
  PR #1220 review discussion, so this belongs there too), mapping to two
  new problem codes.

New problem codes (next available: P4042):

- `P4042 ConstantExpressionDivisionByZero` -- constant division or modulo
  by zero (covers both integer and real).
- `P4043 ConstantExpressionOverflow` -- constant integer arithmetic
  overflows its evaluation range.

## Call sites

- `xform_fold_constant_expressions.rs`: `ConstantFolder::fold_expr` already
  returns `Result<Expr, Diagnostic>` (the `Fold` trait threads this through
  the whole tree already) -- convert `FoldError` to a `Diagnostic` and
  propagate with `?`. No new plumbing needed.
- `xform_fold_initializer_expressions.rs`: `substitute_and_fold` changes
  from `fn(...) -> Expr` to `fn(...) -> Result<Expr, Diagnostic>`,
  propagating child-expression errors with `?` and converting a
  `FoldError` at the point it's raised. `normalize()` matches on the
  `Result` and, on `Err`, pushes the diagnostic and returns an
  uninitialized `Simple` (same recovery shape as the existing "not a
  constant expression" path) instead of treating the whole thing as
  unreachable.

## Tests

- Update the existing `fold_expr_when_div_by_zero_then_no_fold` test in
  `xform_fold_constant_expressions.rs` -- it currently asserts the
  expression is silently left as an unfolded `BinaryOp`; division by zero
  is now a diagnosed error, so rename it and assert `apply()` returns
  `Err` with `Problem::ConstantExpressionDivisionByZero`.
- Add new tests in `xform_fold_constant_expressions.rs`: real division by
  zero, integer overflow (e.g. `DINT` multiplication past `i32`... note
  folding here works in `i128` headroom before the value is later checked
  against the declared type elsewhere, so "overflow" here means the
  `i128` arithmetic itself overflows, e.g. via repeated `Pow`), modulo by
  zero (integer and real).
- Add new tests in `xform_fold_initializer_expressions.rs`: a `VAR`
  initializer expression that divides/overflows, asserting the emitted
  diagnostic is `ConstantExpressionDivisionByZero` /
  `ConstantExpressionOverflow`, not the misleading
  `InitializerNotConstantExpression`.
- New docs: `docs/reference/compiler/problems/P4042.rst`,
  `docs/reference/compiler/problems/P4043.rst`, plus an index.rst toctree
  entry for both.

## Out of scope

- `CONFIGURATION`/`RESOURCE`-scoped constants (already deferred, unrelated
  to this issue).
- Any general "detect non-finite real result" check beyond division and
  modulo by zero (e.g. real `Pow` overflowing to infinity) -- issue #1249
  only reports division by zero for reals; broadening further is a
  separate, unreported concern.
