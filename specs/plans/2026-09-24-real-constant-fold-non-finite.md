# Report non-finite real constant folds (issue #1708)

## Problem

`fold_real_binary` in `compiler/analyzer/src/constant_folding.rs` returns the
raw `f64` result of `+`, `-`, `*`, `/` and `**`. When the result is not finite
the folded literal silently becomes `inf` (e.g. `1.0E300 * 1.0E300`,
`1.0E300 ** 2.0`, `1.0E308 + 1.0E308`, `1.0E300 / 1.0E-300`, `0.0 ** -1.0`) or
`NaN` (e.g. `(-8.0) ** 0.5`), and the program compiles.

## Decision

Reuse P4040 (`ConstantExpressionOverflow`) rather than a new problem code: the
consequence is the same as integer overflow -- the compiler cannot produce the
constant and refuses to compile. Every real operator is checked (one check on
the result, not per operator), so any non-finite result is reported.

- An infinite result is labelled "Arithmetic overflow" (existing label).
- A NaN result gets its own `FoldError::NotANumber` variant so the label can
  say the operation has no real result, still reported as P4040.

## Steps

1. Prefactor: make every arm of `fold_real_binary` compute an `f64` and return
   through a single exit, so the check drops in at one place.
2. Add the finiteness check and `FoldError::NotANumber`.
3. Update the P4040 problem-code message and `P4040.rst` to cover real
   expressions (no longer "integer" only), with a real example.
4. Tests: unit tests on `fold_real_binary`, pass tests in
   `xform_fold_constant_expressions` and `xform_fold_initializer_expressions`.

## Out of scope (report to user / follow-up issues)

- A real *literal* outside the `f64` range (`1.0E400`) parses to `inf` in
  `RealLiteral::try_parse`; that is a literal-range problem, not a fold.
- A finite `LREAL`-range result assigned to `REAL` (`1.0E30 * 1.0E30`) is a
  narrowing concern, the same as a plain `REAL` literal `1.0E300` today.
