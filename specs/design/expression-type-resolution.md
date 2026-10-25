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

The pass runs before the `SymbolEnvironment` is built, so each variable's
`SymbolInfo::type_id` holds the same id as its declaration.

A field is declared as a variable is, so the pass also records, in the
`TypeEnvironment`, the id of the declared type of each field of a structure
or function block (`TypeEnvironment::field_type`).

A structure's type lists its fields, and the environment records the id of
each field's declared type beside it. The two are recorded by different
passes, so they come from one declaration only if each takes the same one: a
structure's field ids are recorded once (`TypeEnvironment::record_structure`),
and only on a structure, so they are those of the declaration whose type the
environment keeps, the first of a repeated name (P2007).

**REQ-ETR-analyzer-006** A repeated type name keeps its first declaration's fields: with `TYPE Pair : STRUCT f : INT; END_STRUCT; END_TYPE` followed by a second `Pair` whose `f` is a `STRING` and which adds `g`, `field_type` of `Pair` and `f` is `INT`, and `Pair` has no field `g`. A structure declared after a type of another kind of its name, such as `TYPE Pair : INT`, records no field on that type.

A type's representation never changes once the type is entered, and a
function block's type names the block and lists none of its fields. A block's
type is entered before declared types are resolved, so that `f : FB` resolves
to it, and by the time its variables' types are known it has been copied into
the types that use it: an array of the block declared ahead of it, such as a
top-level `VAR_GLOBAL`, which is placed before every other declaration. A
field list in the type would be in each copy as the copy was taken, and two
copies of one block could disagree. So the environment records a block's
fields -- its `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT` and `VAR` variables, in
order, each with its section and the id of its declared type -- and every
reader looks them up by the block's id (`TypeEnvironment::block_fields`,
`block_field`, `field_type`). A standard block's fields are recorded the same
way when the environment is built.

A block's fields, the names of its properties and the block it `EXTENDS`,
whose fields and properties an instance of the derived block has too, are
recorded together, once (`TypeEnvironment::record_block`): a repeated block
name keeps its first
declaration (P4013), and a library block named like a standard one records
nothing on the standard block. Only a function block the library declares is
recorded as a base: a structure has no fields a block inherits, and the symbol
environment and code generation do not inherit a standard block's, so typing
them here would let a program pass analysis and fail later.

**REQ-ETR-analyzer-002** The type environment records one field for each `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT` and `VAR` variable a function block declares, in the order it declares them, with its section and the id of its declared type, whatever that type: a structure, a string, an array and a reference are recorded as an `INT` is, and a `VAR_TEMP` is not. The block's type lists none of them.

**REQ-ETR-analyzer-005** Two declarations that spell the same array of a function block declare types of the same shape wherever they are in the library: a top-level `VAR_GLOBAL g : ARRAY[1..2] OF Counter`, placed before every block, and `VAR_EXTERNAL g : ARRAY[1..2] OF Counter` in a program are the same type, so the external matches the global.

**REQ-ETR-analyzer-003** A field a function block inherits through `EXTENDS` has the type the block that declares it gives it: `field_type` of the derived block and `count`, an `INT` the base block declares, is `INT`. A base that is not a function block the library declares gives no field: `EXTENDS Pair` on a structure `Pair` does not make `Pair`'s field `f` a field of the block, and `EXTENDS TON` does not make `Q` one.

A selection `v.f` may name a field of `v`'s type or, on a function block, a
property. The environment answers both, so that a rule asking whether `f`
names anything reads one answer. `field_names` gives the names of the fields:
a block's own fields in order, then those of each block it inherits from,
nearest first, and a structure's in the order its type lists them. Only a type
recorded as a structure or a block has any: an interface is entered as an
empty structure, so that a variable of it resolves, and `field_names` gives
nothing for it rather than an empty list. `has_property` says whether a block,
or a block it inherits from, declares a property of a name. Only the name is
recorded: property access is not implemented.

**REQ-ETR-analyzer-007** On a block `Derived EXTENDS Base`, `field_names` lists `Derived`'s fields in declaration order, then `Base`'s. A structure's are in declaration order, as are an alias's of it; `field_names` of an interface or an elementary type is `None`. `has_property` of `Derived` holds for a property `Derived` declares and for one `Base` declares, and not for a field; `has_property` of `Base` does not hold for a property only `Derived` declares.

`expr_type` is left out of `Expr`'s equality: its ids are allocated per
compilation, so an expected expression built by hand cannot know them.

## Resolution

`xform_resolve_expr_types` folds the library bottom-up and sets `expr_type`
on each expression from its operands' types:

| Expression | Type |
|---|---|
| Untyped literal (`5`, `1.5`) | `Literal(ANY_INT)`, `Literal(ANY_REAL)` |
| Typed literal, string, time, boolean | the elementary type |
| Variable | its declaration's `type_id`; an element or field, the element's or field's type; an element of a reference to an array, the referenced array's element type |
| Arithmetic operator | the result of the overload that applies (see [Arithmetic Operator Overloads](arithmetic-operator-overloads.md)), else the concrete operand's type |
| Unary operator, parenthesised expression | the operand's type |
| `AND`, `OR`, `XOR`, `AND_THEN`, `OR_ELSE` | the concrete operand's type |
| Comparison | `BOOL`; the operands compare at the type one widens to (see [Comparison Operand Type](comparison-operand-type.md)) |
| Function call | the overload's result, else the declared return type, else for a generic return type the argument bound to it |
| Enumerated value | its enumeration, when qualified |
| `REF(x)` | `reference_to(x's type)` |
| Dereference | the referenced type (`TypeEnvironment::referenced_type`) |
| `NULL` | `Null` |
| Implicit conversion | the type it converts to, recorded when the node is inserted |

A variable reference is typed by one walk, by type id
(`variable_type::type_id`): the variable's declared type, then
`TypeEnvironment::field_type` for a field, `element_type` for a subscript and
`referenced_type` for a dereference. A subscript of a reference to an array
indexes the array it refers to, as a subscript of its dereference does: a
TwinCAT `REFERENCE TO` is always used without `^`, and a `REF_TO` a subscript
follows without `^`. A rule that asks the type of a reference
(`variable_type::of`) takes the representation of the type the same walk
reaches. Neither reads a field's type from the fields a representation lists,
so a field has the one type its declaration records.

A subscript list subscripts one array and gives one subscript for each of its
dimensions, so an element of an array of arrays is selected one bracket per
array (`rows[1][2]`), and an element of a multi-dimensional array by one list
(`m[1, 2]`). A list with too few or too many subscripts selects no element and
has no type; `rule_array_subscript_count` reports it (P4085). An array type
that records no dimensions has none to count against.

**REQ-ETR-analyzer-008** With `Row : ARRAY[1..3] OF DINT`, `rows : ARRAY[1..2] OF Row` and `m : ARRAY[1..2, 1..3] OF DINT`, `rows[1]` is a `Row`, `rows[1][2]` and `m[1, 2]` are `DINT`, and `rows[1, 2]`, `m[1]` and `m[1][2]` have no type.

**REQ-ETR-analyzer-009** A subscript of a reference to an array selects an element of the array it refers to, as a subscript of its dereference does: with `pa : REF_TO ARRAY[1..3] OF DINT` and `ra : REFERENCE TO ARRAY[1..3] OF DINT`, `pa[2]`, `pa^[2]` and `ra[2]` are `DINT`, and `pa[1, 2]` has no type.

The two rules that check a subscript list, `rule_array_subscript_count`
(P4085) and `rule_array_index_range` (P2027), count and bound it against the
array the walk reaches (`variable_type::subscripted_type_id`), so a list on a
reference to an array is checked against the array it refers to, as the walk
types it.

**REQ-ETR-analyzer-010** A subscript list on a reference to an array is checked against the array it refers to, with and without `^`: with `p : REF_TO ARRAY[1..5] OF INT`, `p[6]` and `p^[6]` report P2027, and with `p : REF_TO ARRAY[1..2, 1..3] OF DINT`, `p[1]` and `p^[1]` report P4085.

**REQ-ETR-analyzer-004** A rule asking the type of a field of a function block instance gets the type the field is declared with, whatever kind of type it is: on a block with a structure output `p`, a `STRING` output `s` and an `ARRAY[1..3] OF INT` variable `a`, `f.p` is the structure, `f.s` a `STRING`, `f.a` the array, `f.a[2]` an `INT`, and `f.p.x` the type of the structure's field `x`.

After the semantic rules, `xform_insert_implicit_conversions` records the
conversions the language makes without the program spelling them: an
`ExprKind::ImplicitConversion` around an operand converted to another type,
and an untyped literal given the type it is used as. See
[Implicit Conversions](implicit-conversions.md).

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

## Inspecting the annotation

`ironplcc echo --types` writes the analyzed program as Structured Text with
the annotation shown as comments, so the types and conversions the analyzer
recorded can be read for a whole program. plc2plc renders it
(`write_to_string_with_types`). It is handed a function that names a
`TypeId`, so it does not depend on the analyzer, and the CLI hands it
`value_type::spelling`.

```
count := ( count (* INT *) + 1 (* CONSTANT INT *) ) (* INT *) ;
flag := ( big (* LINT *) > total (* DINT -> LINT *) ) (* BOOL *) ;
r := Pass ( 0.1 (* CONSTANT REAL -> LREAL *) ) (* LREAL *) ;
b := ( 1 (* CONSTANT ? ANY_INT *) < 2 (* CONSTANT ? ANY_INT *) ) (* BOOL *) ;
```

**REQ-ETR-plc2plc-001** The annotated rendering writes `(* T *)` after every expression, where `T` is the type recorded for it: `count (* INT *)`.

**REQ-ETR-plc2plc-002** An implicit conversion is written as its operand followed by `(* FROM -> TO *)`, and the operand has no comment of its own: `total (* DINT -> LINT *)`.

A literal, an enumerated value and `NULL` have values the compiler knows; a
variable, a call and an operation produce theirs at run time. The comment
tells them apart. A variable declared `CONSTANT` is still loaded at run time,
because constant folding substitutes only literals, so it is not marked. A
`CONSTANT FROM -> TO` comment is a conversion of a value known when compiling,
which the compiler could remove by giving the literal the type `TO`.

**REQ-ETR-plc2plc-003** A literal, an enumerated value and `NULL` are annotated with `CONSTANT` before the type, and no other expression is: `1 (* CONSTANT INT *)` but `count (* INT *)`.

The recording pass gives every untyped literal the type it is compiled at
(ADR-0056), so an expression left at a generic category has no type of its
own: it is one the analyzer did not decide, and it is marked as undecided, as
an expression with no type is.

**REQ-ETR-plc2plc-004** An expression whose type the analyzer did not decide is annotated with `?`: `(* ? *)` with no recorded type, and `(* ? ANY_INT *)` at a generic category.

**REQ-ETR-plc2plc-005** A unary operation and a dereference are parenthesised in the annotated rendering, so the comment on the operand and the comment on the operation are not written side by side: `( NOT flag (* BOOL *) ) (* BOOL *)`.

**REQ-ETR-plc2plc-006** The annotated rendering re-parses to the same library as the plain rendering of the same library.

An output assignment of a function block call stores a value the analyzer recorded but the program does not write: the output the call leaves (see [Call Binding](call-binding.md)). The annotated rendering writes that value's comment after the output.

**REQ-ETR-plc2plc-007** An output assignment is written with the comment of the value it stores after the output's name: `f(q (* REAL -> LREAL *) => out)`, and `f(NOT done (* BOOL *) => flag)` for a negated output.

**REQ-ETR-analyzer-001** `value_type::spelling` writes an elementary type in upper case, any other named type by its declared name, and an anonymous type by its shape: `DINT`, `Color`, `ARRAY[1..2] OF DINT`.

The output is the analyzed program rather than the source: declarations are
in dependency order, named arguments are positional, constant expressions are
folded, and every file is merged into one output. Only the declarations of the
project's own files are written, not those of an activated library.
Assignment targets, `FOR` control variables and `CASE` labels are not
expressions, so they carry no comment.
