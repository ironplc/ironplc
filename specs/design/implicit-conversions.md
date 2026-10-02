# Design: Implicit Conversions

status: implemented
date: 2026-10-01

## Overview

IEC 61131-3 converts a value without the program spelling it: an operand of
one type compared with an operand of another, or an untyped literal used where
a type is expected (ADR-0028). The analyzer records each such conversion in the
AST (ADR-0056), so that the language server can show how an expression will be
interpreted, and every backend compiles the same conversions rather than
deciding them.

## The node

```rust
// compiler/dsl/src/textual.rs
pub enum ExprKind {
    // ...
    ImplicitConversion(Box<Expr>),
}
```

The inner expression keeps its own type. The type of the enclosing `Expr`
(`expr_type`) is the type the value is converted to. The parser never produces
the node; it is written (`Display`, plc2plc) as the expression it converts.

## The pass

`xform_insert_implicit_conversions` runs in `stages::analyze` after the
semantic rules, so a rule checks the operands the program wrote. A backend
compiles the library `analyze` returns; the library `resolve_types` returns has
no conversions yet.

The pass covers the comparisons `=`, `<>`, `<`, `<=`, `>` and `>=`, and their
function forms `EQ` to `GE`. A comparison compares at the type one operand
widens to, whichever side it is on (see
[Comparison Operand Type](comparison-operand-type.md)); a pair neither of
which widens to the other compares at the type of its concrete left operand,
else of its concrete right one
([#1931](https://github.com/ironplc/ironplc/issues/1931)). Every scalar operand
of another type is converted to the operand type, so a narrower operand is
widened by its own signedness rather than the wider one truncated.

**REQ-IC-analyzer-001** A variable of a numeric or bit-string type other than the operand type is wrapped in an `ImplicitConversion` to the operand type, and an operand of the operand type is left as it is: in `l > d` on an `LINT` and a `DINT` the `DINT` is converted to `LINT`.

**REQ-IC-analyzer-010** The operand type of a comparison is the type one operand widens to, whichever side it is on: in `d < l` on a `DINT` and an `LINT` the `DINT` is converted to `LINT`.

**REQ-IC-analyzer-011** A comparison of two types neither of which widens to the other converts the right operand to the type of the concrete left operand: in `d < u` on a `DINT` and a `UDINT` the `UDINT` is converted to `DINT`.

**REQ-IC-analyzer-012** Any scalar operand of another type is converted, not only a variable: the `ABS(d)` of `ABS(d) < l` and the `TIME` variable of `t < lt` are converted to `LINT` and `LTIME`.

**REQ-IC-analyzer-013** The inputs of a call to `EQ`, `NE`, `LT`, `LE`, `GT` or `GE` are converted as the operands of the operator expression are.

**REQ-IC-analyzer-003** An untyped literal operand is given the operand type: the `1` of `l > 1` and of `1 < l` is an `LINT`.

**REQ-IC-analyzer-004** An operand whose type is the operand type under another name, such as an alias of it, is not converted.

**REQ-IC-analyzer-007** A comparison of strings is left as it is.

**REQ-IC-analyzer-008** An implicit conversion is written where its operand was written: it has the operand's span, and it renders as the operand.

**REQ-IC-analyzer-009** The semantic rules check the operands as written: `DINT#300 < s` on a `SINT` reports P2026 although `s` is converted to `DINT`.

## Codegen

An `ImplicitConversion` compiles its operand at the operand's own type and
converts it to the type the node records. A comparison compiles at the type of
its left operand, else of its right one when the left one has no type codegen
can place (a direct address the analyzer does not type yet). Codegen does not
choose a comparison's operand type, and does not decide which operand to
convert.

## Out of scope

- Arithmetic operands, assignments and function arguments are still
  converted by codegen.
