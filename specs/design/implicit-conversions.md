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

The pass covers the comparisons `=`, `<>`, `<`, `<=`, `>` and `>=`. It records
the rule codegen applied before the analyzer did, without changing it: a
comparison compares at the type of its concrete left operand, else of its
concrete right one, and only a variable of a numeric or bit-string type is
converted to that type. Any other operand of another type is compiled at the
operand type, so a wider one is computed narrow
([#1920](https://github.com/ironplc/ironplc/issues/1920)).

**REQ-IC-analyzer-001** A variable of a numeric or bit-string type other than the operand type is wrapped in an `ImplicitConversion` to the operand type, and an operand of the operand type is left as it is: in `l > d` on an `LINT` and a `DINT` the `DINT` is converted to `LINT`.

**REQ-IC-analyzer-002** The operand type of a comparison is the type of its concrete left operand, else of its concrete right one: in `d < l` on a `DINT` and an `LINT` the `LINT` is converted to `DINT`.

**REQ-IC-analyzer-003** An untyped literal operand is given the operand type: the `1` of `l > 1` and of `1 < l` is an `LINT`.

**REQ-IC-analyzer-004** An operand whose type is the operand type under another name, such as an alias of it, is not converted.

**REQ-IC-analyzer-005** An operand of another type that is not a numeric or bit-string variable, such as a call or a `TIME` variable, is not converted.

**REQ-IC-analyzer-006** The inputs of a call to `EQ`, `NE`, `LT`, `LE`, `GT` or `GE` are not converted.

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

- The function forms `EQ` to `GE` compile at the type of the enclosing
  expression, which is not a conversion the pass can record.
- Arithmetic operands, assignments and function arguments are still
  converted by codegen.
