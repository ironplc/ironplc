# The Analyzer Records Implicit Conversions in the AST

status: accepted
date: 2026-10-01

## Context and Problem Statement

IEC 61131-3 converts a value without the program spelling it: an operand of
one type compared with an operand of another, and an untyped literal that takes
its type from where it is used (ADR-0028). Which conversion applies is a
language rule, but the analyzer did not record its answer. It resolved each
expression's type (`Expr::expr_type`), and codegen decided the rest. For a
comparison, codegen chose the operand type from the concrete left operand, the
concrete right one or the enclosing operation type, and then converted an
operand whose width differed from it when that operand was a variable.

That leaves three gaps:

* **Nobody but codegen can see the decision.** The language server cannot
  show how an expression will be interpreted, which is often hard to work out
  from the source.
* **Every backend has to repeat it.** A second backend would have to apply the
  same fallbacks, and nothing checks that it does.
* **The decision is spread across codegen.** The same "compile at its own
  type, then convert" logic lived in three places, each with a slightly
  different test for when to convert.

Where should the decision live?

## Decision Drivers

* **One answer per expression**, recorded once, that every consumer reads.
* **The language server can show it** without running codegen.
* **Backends lower; they do not decide** what the language means.
* **The semantic rules are unaffected:** what a program is allowed to write
  does not change.

## Considered Options

* Insert `ExprKind::ImplicitConversion` nodes in an analyzer pass
* Record the operand type on the comparison node
* Keep the decision in codegen

## Decision Outcome

Chosen option: "Insert `ExprKind::ImplicitConversion` nodes in an analyzer
pass", because it records the decision on the operand it applies to, in the
tree every consumer already walks.

* **`ExprKind::ImplicitConversion(Box<Expr>)`**: the inner expression keeps
  its own type, and the enclosing `Expr`'s `expr_type` is the type it is
  converted to. The parser never produces one, and it is written (`Display`,
  plc2plc) as the expression it converts.
* **A lowering pass, `xform_insert_implicit_conversions`,** runs in
  `stages::analyze` after the semantic rules. For each comparison it settles
  the operand type, gives an untyped literal operand that type, and wraps an
  operand it converts in a conversion. A backend compiles the library
  `analyze` returns; `resolve_types` alone does not lower it.
* **Codegen compiles what the tree says.** A conversion compiles its inner
  expression at its own type and converts. A comparison compiles at its left
  operand's type, which the pass made the operand type.

The pass starts by recording exactly what codegen did for a comparison, so
introducing it changes no generated code; correcting what it records is then a
change to one analyzer pass (#1920). Arithmetic operands, assignments and
function arguments are still converted by codegen; each moves to the pass in
its own change. See `specs/design/implicit-conversions.md`.

### Consequences

* Good, because the language server can show where a value is converted and
  to what, from the analyzed tree.
* Good, because a backend no longer chooses an operand type for a comparison,
  so two backends cannot choose differently.
* Good, because a questionable conversion becomes visible: an `LINT`
  compared with a `DINT` on its left shows as a conversion to `DINT` (#1920).
* Good, because the pass runs after the semantic rules, so a rule checks the
  operands the program wrote. Running it before them was tried and lost
  diagnostics: `rule_constant_range` checks a literal against the other
  operand's type, and `DINT#300 < s` on a `SINT` stopped reporting P2026 once
  `s` was converted to `DINT`.
* Bad, because a caller that compiles the output of `resolve_types` rather
  than `analyze` gets no conversions, and a comparison with an untyped
  literal on its left compiles at the literal's default type. Every caller in
  the repository that compiles uses `analyze`.
* Bad, because the analyzed tree is no longer only what the program wrote. A
  consumer that renders it must render a conversion as its operand.

### Confirmation

The pass has unit tests asserting the conversions it records for each kind of
operand pair (`xform_insert_implicit_conversions/tests.rs`), tied to the
`REQ-IC-analyzer-*` requirements in `specs/design/implicit-conversions.md`,
including that the semantic rules still see the operands as written. The
codegen tests pass unchanged.

## Pros and Cons of the Options

### Insert `ExprKind::ImplicitConversion` nodes in an analyzer pass (chosen)

* Good, because the conversion sits on the operand it converts, where a
  language server hover or inlay hint would show it.
* Good, because the same node serves every later kind of implicit
  conversion: arithmetic operands, assignments, arguments.
* Bad, because adding an `ExprKind` variant touches every exhaustive match.

### Record the operand type on the comparison node

* Good, because it is a smaller change: one field on `CompareExpr`.
* Bad, because each backend would still decide which operand to convert, and
  how, from the recorded type.
* Bad, because the function forms (`GT(a, b)`) are `Function` nodes and would
  need the field somewhere else.

### Keep the decision in codegen

* Good, because nothing changes.
* Bad, because the language server cannot see the decision, and every backend
  repeats it.

## More Information

### Arithmetic operands (postscript)

The pass now records the conversions of arithmetic operands too, for the
operator expression and for the function forms `ADD`, `SUB`, `MUL`, `DIV` and
`MOD` (see `specs/design/implicit-conversions.md`). Codegen compiles the
recorded nodes and no longer converts an arithmetic operand itself, or asks
the analyzer's overload resolver a second time for a numeric fold. A function
form of three or more inputs is recorded as the nested calls it folds to,
because each step computes at its own result type and the accumulated value is
converted between steps, and an accumulator is not an operand a node can wrap.
Assignments and function arguments are still converted by codegen, and so is
the conversion of an arithmetic result to the type of its context; each moves
to the pass in its own change. The decision above is unchanged.

### Assignments (postscript)

The pass also records the conversion of an assigned value to the type its
target is stored as, where codegen converts it: a variable or an arithmetic
result of another operation width. The conversions a variable or an arithmetic
result makes to the type of its context stay in codegen until arguments, the
remaining context, record them too, since the same code serves both. The
decision above is unchanged.

### Arguments (postscript)

The pass also records the conversion of an argument to a user-defined
function to the type its parameter is passed as, and codegen no longer
converts an argument itself. The recording makes two existing choices visible:
a parameter of a non-elementary type is passed as a `DINT`, and an untyped
real literal is a `REAL` converted to an `LREAL` parameter. Correcting either
is a change to the pass. Function block and method arguments are not recorded
yet. The decision above is unchanged.

