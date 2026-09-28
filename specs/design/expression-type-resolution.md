# Design: Expression Type Resolution

status: implemented
date: 2026-09-28

## Overview

Every expression in the AST carries the type of its value, by identity:
`Expr::expr_type`. The analyzer's rules and codegen read an expression's type
from it and from nowhere else, so an expression cannot have two types that
disagree.

### Building On

- **[ADR-0013](../adrs/0013-expression-type-annotation-via-wrapper-struct.md)**:
  the `Expr` wrapper carries the annotation.
- **[ADR-0055](../adrs/0055-concrete-type-ids-numbered-by-debug-tag.md)**:
  types are identified by a numeric `TypeId`, and an elementary type's id is
  its debug type tag.
- **[ADR-0001](../adrs/0001-bytecode-integer-arithmetic-type-strategy.md)**:
  the promote-operate-truncate model codegen implements from the type.

## The annotation

```rust
// compiler/dsl/src/textual.rs
pub enum ExprType {
    Concrete(TypeId),         // a value of exactly this type
    Literal(GenericTypeName), // an untyped literal, typed by where it is used
    Null,                     // NULL, of whichever reference type it is used as
}

pub struct Expr {
    pub kind: ExprKind,
    pub expr_type: Option<ExprType>, // None until resolved, or when unresolvable
    pub span: SourceSpan,
}
```

A declaration carries the id of the type it declares in
`VarDecl::type_id`, filled in by `xform_resolve_decl_types`:

- a named type takes its name's id;
- a type spelled out in place (an inline array, enumeration or subrange) is
  entered in the `TypeEnvironment` as an anonymous type with no name, one
  per declaration;
- a reference type is the one type `TypeEnvironment::reference_to(target)`
  interns, so `REF_TO INT` is the same type wherever it is spelled.

`expr_type` is left out of `Expr`'s equality: its ids are allocated per
compilation, so an expected expression built by hand cannot know them.

## Resolution

`xform_resolve_expr_types` folds the library bottom-up and sets `expr_type`
on each expression from its operands' types:

| Expression | Type |
|---|---|
| Untyped literal (`5`, `1.5`) | `Literal(ANY_INT)`, `Literal(ANY_REAL)` |
| Typed literal, string, time, boolean | the elementary type |
| Variable | its declaration's `type_id`; an element or field, the element's or field's type |
| Arithmetic operator | the result of the overload that applies (see [Arithmetic Operator Overloads](arithmetic-operator-overloads.md)), else the concrete operand's type |
| Unary operator, parenthesised expression | the operand's type |
| `AND`, `OR`, `XOR`, `AND_THEN`, `OR_ELSE` | the concrete operand's type |
| Comparison | `BOOL` |
| Function call | the overload's result, else the declared return type, else for a generic return type the argument bound to it |
| Enumerated value | its enumeration, when qualified |
| `REF(x)` | `reference_to(x's type)` |
| Dereference | the referenced type (`TypeEnvironment::referenced_type`) |
| `NULL` | `Null` |

## Relations that compare by name

The compatibility relation (`type_compat::are_types_compatible`) and the
arithmetic overloads compare elementary types and generic categories by
name. They take their operand's name from `value_type::operand_type_name`,
derived from the `ExprType` each time it is asked for, so it cannot disagree
with it:

- an untyped literal is its generic category;
- an elementary type, or an alias of one, is the elementary name;
- a string is `STRING` or `WSTRING`;
- a reference is known by the type it references;
- a subrange is its own name, or its base type's when anonymous;
- any other type is its own name, and has none when anonymous.

`value_type::check` decides whether a value is accepted where a type is
required. A whole array, structure, enumeration or function block instance
is accepted only for its own type, or an array or structure of identical
shape; a subrange compares as its base type.

## Codegen

`CompileContext::types` holds every type's representation by id.
`type_info::expr_type_info` projects an expression's `expr_type` onto the
operation width and signedness codegen needs:

- an enumeration operates as a `DINT`;
- a subrange operates as its base type;
- a reference, and `NULL`, operate as a 64-bit address;
- an untyped literal defaults to `DINT` or `REAL`.

`type_info::decl_type_info` does the same for a declaration from its
`type_id`. `resolve_type_name` remains only for types written as names
with no declaration behind them: function return types, parameter types
from a signature, array element type names, and conversion function names.
