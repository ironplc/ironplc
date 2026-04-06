# Constant Folding for Binary Expressions

## Goal

Add a new analyzer transform pass that folds binary expressions where both
operands are compile-time integer or real constants into a single constant
node. This avoids emitting redundant load + arithmetic instructions for
expressions like `2 + 3`. Also migrate the existing unary-negation constant
folding from codegen into this pass for consistency.

## Architecture

Add a new `xform_fold_constant_expressions` module in the analyzer crate.
This pass runs **after** `xform_resolve_expr_types` (so `resolved_type` is
populated) and **before** `xform_resolve_type_aliases`. It uses the standard
`Fold<Diagnostic>` trait, overriding `fold_expr` to:

1. Recurse into children first (bottom-up folding).
2. Check if the result is a `BinaryOp` where both operands are `Const(IntegerLiteral)` or `Const(RealLiteral)`.
3. Evaluate the operation at compile time and replace the node with `Const(...)`.
4. Similarly fold `UnaryOp(Neg)` of a constant literal.

No new problem codes are needed — `ConstantOverflow` (P2026) already exists and
will be reused for overflow errors during folding.

Codegen's existing unary-negation constant folding in `compile_expr.rs` will be
removed since the AST pass now handles it.

## Design doc reference

N/A — self-contained optimization.

## File map

| File | Action |
|------|--------|
| `compiler/analyzer/src/xform_fold_constant_expressions.rs` | **Create** — new fold pass |
| `compiler/analyzer/src/lib.rs` | **Modify** — add `mod` declaration |
| `compiler/analyzer/src/stages.rs` | **Modify** — wire pass into pipeline |
| `compiler/codegen/src/compile_expr.rs` | **Modify** — remove unary-neg constant folding |

## Tasks

- [x] Create `xform_fold_constant_expressions.rs` with `Fold<Diagnostic>` impl
- [x] Handle binary ops: Add, Sub, Mul, Div, Mod for integer literals
- [x] Handle binary ops for real literals
- [x] Handle unary negation folding (migrated from codegen)
- [x] Register module in `lib.rs`
- [x] Wire into `stages.rs` after `xform_resolve_expr_types`
- [x] Remove unary-neg constant folding from `compile_expr.rs`
- [x] Add unit tests (BDD-style naming)
- [x] Run full CI pipeline
