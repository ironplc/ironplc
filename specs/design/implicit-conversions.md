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

### Arithmetic

An arithmetic operation computes at the type of its result (ADR-0001): `INT + REAL` converts the `INT` to `REAL` and adds as `REAL`. The pass covers the operator expression (`+`, `-`, `*`, `/`, `MOD`, `**`) and the function forms `ADD`, `SUB`, `MUL`, `DIV` and `MOD`. A pair with a typed overload on the time and date types (`t1 + t2`, `dt + t`) is left as it is, and so is an operation whose result is not an elementary numeric type.

**REQ-IC-analyzer-020** An operand whose operation width differs from the result's is wrapped in an `ImplicitConversion` to the result type: in `i + r` on an `INT` and a `REAL` the `INT` is converted to `REAL`.

**REQ-IC-analyzer-021** An untyped literal operand is given the result type: the `1` of `l + 1` is an `LINT`.

**REQ-IC-analyzer-022** An operand of the result's operation width is not converted, whatever its signedness: in `i + s` on an `INT` and a `SINT` neither is wrapped.

**REQ-IC-analyzer-023** A pair with a typed overload is left as it is: the two `TIME` operands of `t1 + t2` are not converted.

**REQ-IC-analyzer-024** The inputs of a call to the function form of an operator are converted as the operands of the operator expression are.

**REQ-IC-analyzer-025** A function form of three or more inputs whose every step is the numeric overload is written as the calls it folds to, each step computing at its own result type: `ADD(d, e, l)` on two `DINT`s and an `LINT` is `ADD(ADD(d, e), l)`, with the `DINT` result of the inner call converted to `LINT`.

**REQ-IC-analyzer-026** An operand that is itself arithmetic is converted by its result type: in `(i + j) * r` the `INT` result of `i + j` is converted to `REAL`.

The conversion of an arithmetic result to the type of its context is recorded where the context is: see Assignments below. An argument's is not recorded yet.

### Assignments

An assignment stores its value at the type of its target. The pass records the conversion of the value to the type the target is stored as: its own elementary type, or its base type for a subrange. It records the conversions the code generator makes, and only those: a value converts to its context when it is a variable, an operation that computes at its own result type, or a parenthesized one of those, and its operation width differs from the target's. An operation computes at its own type when it is arithmetic (operator or function form) or an operation on one value (see Codegen). Any other value is compiled at the target's width rather than converted to it, so there is nothing to record: a literal takes the target's type (ADR-0028), and `MAX` of two `DINT`s assigned to an `LINT` selects at the target's width.

**REQ-IC-analyzer-030** A variable assigned to a target of another operation width is wrapped in an `ImplicitConversion` to the target's type: in `l := d` on an `LINT` and a `DINT` the `DINT` is converted to `LINT`.

**REQ-IC-analyzer-031** An arithmetic value is converted from its result type: in `l := d + e` on an `LINT` and two `DINT`s the `DINT` sum is converted to `LINT`.

**REQ-IC-analyzer-032** A value of the target's operation width is not converted, whatever its signedness: in `i := s` on an `INT` and a `SINT` the `SINT` is not wrapped.

**REQ-IC-analyzer-037** An operation on one value is converted from its own type: in `l := -d` on an `LINT` and a `DINT` the negation is converted to `LINT`, and so are `NOT`, `ABS` and the other numeric functions of one input, `MOVE`, and a shift or rotate.

**REQ-IC-analyzer-034** An array element or structure field target converts the value to the element's or field's type.

**REQ-IC-analyzer-035** A function's result assignment converts the value to the function's return type, looking the result variable up in the function's scope.

**REQ-IC-analyzer-036** A subrange target converts the value to the subrange's base type.

The target of a dereference (`r^ := d`), a function block field (`timer.PT := t`), and a directly represented variable (`%QW0 := w`) are not recorded yet, and neither is a function block output stored by a call (`fb(OUT => x)`), which codegen stores at the field's operation type without a conversion.

### Arguments

A call to a user-defined function passes each input by value at the operation width of its parameter. The pass records the conversion of an argument of another width to the type its parameter is passed as, and codegen compiles the argument at the parameter's width without choosing a conversion. It records what codegen did before it, including two choices a later change may correct: a parameter whose type is not elementary (an alias, a subrange, an enumeration) is passed as a `DINT`, the default slot type, and an untyped literal is operated at its default type (`DINT` or `REAL`, ADR-0028) and then converted.

**REQ-IC-analyzer-040** An argument whose operation width differs from its parameter's is wrapped in an `ImplicitConversion` to the parameter's type: the `DINT` of `f(d)` with an `LINT` parameter is converted to `LINT`, whatever kind of expression the argument is.

**REQ-IC-analyzer-041** An argument of its parameter's operation width is not converted: the `SINT` of `f(s)` with an `INT` parameter is not wrapped.

**REQ-IC-analyzer-042** An untyped literal argument takes its parameter's type when its default type has the parameter's width, and is otherwise its default type converted to the parameter's: the `1` of `f(1)` with an `INT` parameter is an `INT`, with an `LINT` one a `DINT` converted to `LINT`.

**REQ-IC-analyzer-043** A parameter whose type is not elementary is passed as a `DINT`: an argument of an alias of `LINT` to a parameter of that alias is converted to `DINT`.

**REQ-IC-analyzer-044** A named argument is converted as a positional one is.

**REQ-IC-analyzer-045** The arguments of a standard function, and of a `VAR_IN_OUT` parameter, are not converted.

**REQ-IC-codegen-001** An argument narrower than its parameter is widened by its own signedness: a `UDINT` above `i32::MAX` passed to an `LINT` parameter keeps its value.

The arguments of a function block call and of a method call are not recorded yet: codegen compiles them at the field's or parameter's operation type, and a variable or an arithmetic result converts itself to it, as it does for any context that records nothing.

### Literals

An untyped literal has a generic type (`ANY_INT`) until a context gives it one (ADR-0028). Codegen gives it one top-down: a statement passes the type it stores at into the expression, and that type flows through a negation, parentheses and an arithmetic operation whose own type is generic until it reaches the literal. The pass records the type each literal reaches. An arithmetic operation of literals alone is folded to one literal before the pass runs. A literal operand of an arithmetic operation of a concrete type takes that type instead (`d + -1`), and a comparison operand or a function argument is typed by those constructs above.

**REQ-IC-analyzer-050** An untyped literal assigned to a target takes the type the target is stored at: the `1` of `l := 1` on an `LINT` is an `LINT`.

**REQ-IC-analyzer-051** The type of the context flows through a negation and parentheses: the `1` of `l := -(1)` is an `LINT`.

**REQ-IC-analyzer-053** The bounds and step of a `FOR` loop take the type of its control variable.

**REQ-IC-analyzer-054** A literal input of a function block call takes the type of the field it is stored in for a user-defined block, and `DINT`, the default slot type, for a standard one, which is how codegen stores it.

**REQ-IC-analyzer-055** A literal assigned through a dereference takes `DINT`, the default slot type codegen stores it at.

**REQ-IC-analyzer-056** A literal assigned to a subrange target takes the subrange's base type.

Every construct a literal can sit in either passes the type of its context on, computes at a type of its own, or computes at a fixed type, and the literal takes the type it reaches:

**REQ-IC-analyzer-057** A standard function that computes at the type of its context (`MAX`, `MIN`, `LIMIT`, the inputs of `MUX` and `SEL`) passes that type to its literal inputs: the `5` of `l := MAX(d, 5)` on an `LINT` is an `LINT`.

**REQ-IC-analyzer-058** The selector of `MUX` and of `SEL` takes `DINT`, whatever the type of the inputs it selects between.

**REQ-IC-analyzer-059** The count of a shift or rotate takes `LINT` when the shifted value is operated at 64 bits and `DINT` otherwise.

**REQ-IC-analyzer-060** A position or length input of a string function (`LEFT`, `RIGHT`, `MID`, `INSERT`, `DELETE`, `REPLACE`) takes `DINT`.

**REQ-IC-analyzer-061** The input of a type conversion function takes the conversion's source type: the `5` of `INT_TO_REAL(5)` is an `INT`.

**REQ-IC-analyzer-062** A comparison of two untyped literals compares at the left one's default type: both literals of `1 < 2` are `DINT`s.

**REQ-IC-analyzer-063** A literal argument of a method call takes the type of the parameter it is passed to, or `DINT` for a parameter not declared with a simple type, which is how codegen passes it.

**REQ-IC-analyzer-064** A literal subscript of an array access whose subscripts are not all literals takes `DINT`.

**REQ-IC-analyzer-065** The literals of a `CASE` selector take the selector's type, and those of an `IF`, `WHILE` or `REPEAT` condition the type the condition is tested at.

**REQ-IC-analyzer-067** An operation on one value of a numeric type passes its own type, not its context's, to its literal inputs: the count `1` of `lw := SHL(w, 1)` on an `LWORD` and a `DWORD` is a `DINT`, since the `DWORD` is shifted at 32 bits.

**REQ-IC-analyzer-066** A typed numeric literal keeps its own type and, in a context of another numeric type, is wrapped in an `ImplicitConversion` to it: the `UDINT#4000000000` of `l := UDINT#4000000000` on an `LINT` is converted to `LINT`.

**REQ-IC-codegen-002** An integer or real literal compiles at the type the analyzer recorded for it, not at a type its context passes down: `l := 5000000000` stores 5000000000, and `l := UDINT#4000000000` stores 4000000000.

A literal codegen builds itself has no recorded type: a member initializer of a function block instance takes the field's type in a user-defined block and is stored at the default slot type in a standard one. A time, date, string, boolean or bit-string literal names its own type and is compiled for the storage its context gives it.

A literal a standard function does not give a type to (`TRUNC`, a typed time function such as `MUL_TIME`) takes its default type, as does a literal argument of a user-defined function whose parameter the argument pass did not give it, before the argument's conversion. The member initializers of a function block instance, which codegen compiles as expressions it builds itself, are not typed by the pass.

## Codegen

An `ImplicitConversion` compiles its operand at the operand's own type and
converts it to the type the node records. A comparison compiles at the type of
its left operand, else of its right one when the left one has no type codegen
can place (a direct address the analyzer does not type yet). Codegen does not
choose a comparison's operand type, and does not decide which operand to
convert.

An operation on one value -- a negation, `NOT`, a numeric function of one
input (`ABS`, `SQRT`, ...), `MOVE`, or a shift or rotate, whose count only
says how far -- has its operand's type, and computes at that type when it is
numeric, as an arithmetic operation computes at its result's. Its result is
converted to the type of its context. A function of several inputs of one type
(`MAX`, `MIN`, `LIMIT`, `SEL`, `MUX`, `EXPT`, `ATAN2`) computes at the type of
its context instead, because the analyzer types its result by its first input,
which need not be the widest
([#2127](https://github.com/ironplc/ironplc/issues/2127)).

**REQ-IC-codegen-003** An operation on one value assigned to a wider target computes at its operand's type: `lw := SHL(dw, 1)` with `dw = 16#80000000` stores 0, `lw := NOT dw` with `dw = 0` stores `16#FFFFFFFF`, and `l := -d` with `d` the least `DINT` stores that `DINT`.

## Out of scope

- A value in a context that records nothing -- a function block or method
  argument, a condition, a subscript -- is still converted by codegen.
- Codegen still converts a variable, an arithmetic result or an operation on
  one value to the type of its context where nothing recorded it, so its own
  conversion is removed only once every context records it. A literal's type
  is no longer passed down.
