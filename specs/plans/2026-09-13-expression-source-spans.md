# Expressions record their own source span

Fixes [#1662](https://github.com/ironplc/ironplc/issues/1662).

## Goal

A diagnostic whose label is an expression underlines the expression *as
written*, including the tokens that are not part of any child node: the
unary operator in `-g` / `NOT g`, the parentheses in `(a + b)`, the caret
in `p^`, the `REF(` … `)` around a reference, and the argument list of a
function call.

## Architecture

`Located for ExprKind` reconstructs a span from the children of each
variant. That works only for the variants whose syntax is entirely made of
children — `Compare` and `BinaryOp`, where the operator sits between the
two operands. Every other variant has at least one token of its own that
the node does not hold, so the reconstruction is lossy by construction and
no per-variant patch fixes the class:

| Variant | Written | Span today |
|---|---|---|
| `UnaryOp` | `NOT g` | `g` |
| `Expression` | `(a + b)` | `a + b` |
| `Deref` | `p^` | `p` |
| `Ref` | `REF(g)` | `g` |
| `Function` | `MAX(a, b)` | `MAX` |

Issue #1662 sketches two fixes for the unary case: record the operator's
span on `UnaryExpr`, or wrap `unary_expression` in `position!()`. Both stop
at `UnaryOp`. The general fix is to stop deriving the span at all and
record it where the tokens are still in hand — the parser.

`Expr` is already the wrapper the DSL puts around every `ExprKind` for
exactly this kind of out-of-band information (`resolved_type`), and it
appears at every level of an expression tree (`CompareExpr::left`,
`UnaryExpr::term`, `ExprKind::Deref`, …). It gains a `span` field that the
parser fills from the tokens it matched.

`Located for ExprKind` stays as it is, as the fallback for an expression
built by a transform rather than parsed: `Expr::new` seeds the recorded
span from `kind.span()`, so every construction site outside the parser
behaves exactly as it does today.

## Prefactoring

The expression grammar returns `ExprKind`, and each of its ~20 callers
wraps the result in `Expr::new(...)` itself. There is therefore no single
place where a parsed expression becomes an `Expr`, and so nowhere for the
parser to attach a span.

Prefactor first: `expression()`, `unary_expression()`,
`primary_expression()` and `function_expression()` return `Expr`, and the
`Expr::new(...)` wrapping disappears from the call sites. Behaviour is
unchanged — `Expr` has no span of its own yet, so every span still comes
from `Located for ExprKind` — and the existing tests pass unedited.

## File map

Prefactor:

- `compiler/dsl/src/textual.rs` — `Expr::compare` / `Expr::binary` /
  `Expr::unary` taking `Expr` operands; the existing `ExprKind::*`
  constructors delegate to them
- `compiler/parser/src/parser.rs` — expression rules return `Expr`

Fix:

- `compiler/dsl/src/textual.rs` — `Expr::span` field, `Expr::with_span`
- `compiler/parser/src/parser.rs` — record the span in each expression rule
- `compiler/parser/src/tests/literals.rs` — drop the note explaining why a
  leading `-` is excluded from the literal-span cases
- `compiler/parser/src/tests/expression_spans.rs` — new: one `rstest` case
  per expression shape, asserting the span slices back to the source text

## Tasks

- [ ] Prefactor: expression grammar produces `Expr`
- [ ] Add `Expr::span`, seeded by `Expr::new` from `kind.span()`
- [ ] Record spans in `expression()`, `unary_expression()`,
      `primary_expression()`, `function_expression()`
- [ ] Tests: span covers the expression as written, per shape
- [ ] Update the `literals.rs` note that documents the old behaviour
- [ ] `cd compiler && just`
