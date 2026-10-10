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
semantic rules, so a rule checks the operands the program wrote.
`rule_constant_range` runs after the pass instead, on the library it returns:
it checks each literal against the type the pass records for it, and reads an
operand's type through the conversion that wraps it, so it checks the operands
as written too. A backend compiles the library `analyze` returns;
the library `resolve_types` returns has no conversions yet.

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

**REQ-IC-analyzer-009** A rule checks the operands as written, reading an operand through the conversion that wraps it: `DINT#300 < s` on a `SINT` reports P2026 although `s` is converted to `DINT`.

### Arithmetic

An arithmetic operation computes at the type of its result (ADR-0001): `INT + REAL` converts the `INT` to `REAL` and adds as `REAL`. The pass covers the operator expression (`+`, `-`, `*`, `/`, `MOD`, `**`) and the function forms `ADD`, `SUB`, `MUL`, `DIV` and `MOD`. A pair with a typed overload on the time and date types (`t1 + t2`, `dt + t`) computes through its routine, whose operands are converted as Time and date operations below says, and an operation whose result is not an elementary numeric type is left as it is.

**REQ-IC-analyzer-020** An operand whose operation width differs from the result's is wrapped in an `ImplicitConversion` to the result type: in `i + r` on an `INT` and a `REAL` the `INT` is converted to `REAL`.

**REQ-IC-analyzer-021** An untyped literal operand is given the result type: the `1` of `l + 1` is an `LINT`.

**REQ-IC-analyzer-022** An operand of the result's operation width is not converted, whatever its signedness: in `i + s` on an `INT` and a `SINT` neither is wrapped.

**REQ-IC-analyzer-023** A pair with a typed overload whose operands have the width its routine computes at is left as it is: the two `TIME` operands of `t1 + t2` are not converted.

**REQ-IC-analyzer-024** The inputs of a call to the function form of an operator are converted as the operands of the operator expression are.

**REQ-IC-analyzer-025** A function form of three or more inputs whose every step is the numeric overload is written as the calls it folds to, each step computing at its own result type: `ADD(d, e, l)` on two `DINT`s and an `LINT` is `ADD(ADD(d, e), l)`, with the `DINT` result of the inner call converted to `LINT`.

**REQ-IC-analyzer-026** An operand that is itself arithmetic is converted by its result type: in `(i + j) * r` the `INT` result of `i + j` is converted to `REAL`.

The conversion of an arithmetic result to the type of its context is recorded where the context is: see Assignments below. An argument's is not recorded yet.

### Time and date operations

A typed time or date function (IEC 61131-3 Table 30: `ADD_TIME`, `SUB_DT_DT`, `MUL_TIME`, ...), and an operator expression or function form that resolves to one, computes at the width of its form: 32 bits for a short form and 64 for a long one (`ADD_LTIME`, `SUB_LDATE_LDATE`). The pass converts each operand to that width, in a call to the typed function itself as in the operator and the function form. An operand of a temporal parameter is converted to the parameter's type. The number `MUL` and `DIV` scale a duration by is converted to the type the routine scales at: a 64-bit integer for a long form, and `LREAL` for a real of a long form or a 64-bit integer of a short one, which cannot be narrowed to the duration's 32 bits. The code generator used to widen these operands itself.

**REQ-IC-analyzer-096** A temporal operand of a long form whose width differs from its parameter's is converted to the parameter's type, in the operator expression, the function form and a call to the typed function: the `TIME` of `lt + t`, `t + lt`, `ADD(t, lt)`, `ADD_LTIME(lt, t)` and `ldt + t` is converted to `LTIME`, and the `DATE` of `lda - da` and `SUB_LDATE_LDATE(lda, da)` to `LDATE`.

**REQ-IC-analyzer-097** The number a duration is scaled by is converted to the type its routine scales at: the `DINT` of `lt * d` and the `UDINT` of `MUL_LTIME(lt, ud)` to `LINT`, the `REAL` of `lt * r` to `LREAL`, the literal of `lt / 2` is an `LINT`, and the `LINT` of `t * l` and of `MUL_TIME(t, l)` is converted to `LREAL`. A `SINT` or `REAL` a `TIME` is scaled by is not converted.

**REQ-IC-analyzer-098** A function form of three or more inputs whose every step is a typed overload is written as the calls it folds to, each step's operands converted to its routine's width: `ADD(t, t, lt)` is `ADD(ADD(t, t), lt)`, with the `TIME` result of the inner call converted to `LTIME`.

### Standard functions

A function of several inputs of one type -- `MIN`, `MAX`, `LIMIT`, `SEL`, `MUX`, `EXPT` and `ATAN2` -- computes at the type of its result, as an arithmetic operation does. The result is the type every input of the function's type widens to, by the relation a comparison chooses its operand type by (see [Comparison Operand Type](comparison-operand-type.md)), so it is never narrower than an input: `MAX(i, l)` on an `INT` and an `LINT` is an `LINT`, and assigning it to an `INT` is a type mismatch. `EXPT` is typed by its base: its result has the type of `IN1`, and the exponent is converted to it. `SEL`'s selector `G` is a `BOOL` and `MUX`'s `K` an integer, whatever the type of the inputs they select between.

A set of inputs none of whose types every other one widens to, such as a `DINT` and a `UDINT`, has the type of its first concrete input, as a comparison of such a pair compares at its concrete left operand's. It is not reported yet ([#1931](https://github.com/ironplc/ironplc/issues/1931), [#2127](https://github.com/ironplc/ironplc/issues/2127)).

The pass records, for each call, the conversion of every input of the function's type to the result type, and the conversion of the result to the type of its context where that context is recorded (Assignments and Arguments below, and a comparison or arithmetic operand above).

**REQ-IC-analyzer-070** The result of `MIN`, `MAX`, `LIMIT`, `SEL`, `MUX` and `ATAN2` has the type every input of the function's type widens to, whichever input that is: `MAX(i, l)` and `MAX(l, i)` on an `INT` and an `LINT` are `LINT`s, and the `G` of `SEL` and the `K` of `MUX` do not take part.

**REQ-IC-analyzer-071** A set of inputs none of whose types every other one widens to has the type of its first concrete input: `MAX(d, u)` on a `DINT` and a `UDINT` is a `DINT`, and `MAX(5, d)` on a `DINT` is a `DINT`.

**REQ-IC-analyzer-072** The result of `EXPT` has the type of its first input: `EXPT(r, l)` on a `REAL` and an `LINT` is a `REAL`.

**REQ-IC-analyzer-073** An input of the function's type whose operation width differs from the result's is wrapped in an `ImplicitConversion` to the result type: the `INT` of `MAX(i, l)` is converted to `LINT`, and the `LINT` exponent of `EXPT(r, l)` to `REAL`.

**REQ-IC-analyzer-074** An input of the result's operation width is not converted, whatever its signedness: in `MAX(s, i)` on a `SINT` and an `INT` neither is wrapped.

**REQ-IC-analyzer-075** The `K` of `MUX` is converted to `DINT` when its operation width differs: the `LINT` `K` of `MUX(k, d, e)` is converted to `DINT`. The `G` of `SEL` is left the `BOOL` it is.

**REQ-IC-analyzer-076** A call whose result is an untyped literal's category takes the type of its context, as an untyped literal does (ADR-0028), and its inputs are converted to that type: `l := MAX(1, 2)` on an `LINT` is an `LINT`, and in `lr := EXPT(2, r)` on an `LREAL` and a `REAL` the `REAL` is converted to `LREAL`.

**REQ-IC-analyzer-077** The result is converted to the type of a context of another operation width: in `l := MAX(d, e)` on an `LINT` and two `DINT`s the `DINT` result is converted to `LINT`.

**REQ-IC-analyzer-078** A result assigned to a narrower target is a type mismatch: `i := MAX(i, l)` on an `INT` and an `LINT` reports P4027.

`TRUNC`, `BCD_TO_INT` and `SIZEOF` return an integer, and nothing about their inputs says which. Such a call is an integer of whichever type its context stores it at, as an untyped integer literal is (ADR-0028), and the pass records that type on it. Unlike a literal it stays an integer: in a context that is not an integer type it takes `DINT`, the type an untyped integer literal defaults to, and is converted to the context's type. The code generator used to compile `TRUNC` at its context's type, so `r := TRUNC(x)` with a `REAL` target did not truncate. `INT_TO_BCD` encodes its input digit by digit in the bit string as wide as it, and computes at that type as an operation on one value does.

**REQ-IC-analyzer-088** A call to `TRUNC`, `BCD_TO_INT` or `SIZEOF` in an integer context takes the context's type: the `TRUNC(lr)` of `l := TRUNC(lr)` on an `LINT` is an `LINT`, and of `i := TRUNC(r)` on an `INT` an `INT`.

**REQ-IC-analyzer-089** A call to `TRUNC`, `BCD_TO_INT` or `SIZEOF` in a context that is not an integer type takes `DINT` and is converted to the context's type, wherever the context is: in `x := TRUNC(r)` on a `REAL`, in `TRUNC(r) < r`, in `TRUNC(r) * r` and as an argument to a `REAL` parameter, the `DINT` is converted to `REAL`.

**REQ-IC-analyzer-090** `INT_TO_BCD` has the type of the bit string as wide as its input, and its result is converted to a wider target: `INT_TO_BCD(i)` on an `INT` is a `WORD`, `INT_TO_BCD(l)` on an `LINT` an `LWORD`, `INT_TO_BCD(42)` a `DWORD`, and `lw := INT_TO_BCD(i)` converts the `WORD` to `LWORD`.

**REQ-IC-analyzer-091** `TRUNC` assigned to a bit string, and `INT_TO_BCD` assigned to an integer or to a narrower bit string, report P4027: `w := TRUNC(r)`, `j := INT_TO_BCD(i)` and `w := INT_TO_BCD(l)` on a `WORD`, an `INT` and an `LINT`.

### Bitwise operators

`AND`, `OR` and `XOR` are boolean on `BOOL`s and bitwise on bit strings. Each computes at the type every operand widens to, by the relation a comparison chooses its operand type by, as a function of several inputs of one type does: `w OR lw` on a `WORD` and an `LWORD` is an `LWORD`. Their function forms are functions of several inputs of one type (see Standard functions). The pass converts an operand of another width to the operation's type, and records the conversion of the result to the type of its context, as it does an arithmetic result's. The analyzer used to type the operator by its left operand, so `w OR lw` truncated the `LWORD` to 32 bits, and a `DWORD` result stored in an `LWORD` was sign-extended.

**REQ-IC-analyzer-081** `AND`, `OR` and `XOR`, in the operator and the function form, have the type every operand widens to, whichever side it is on: `w OR lw` and `lw OR w` on a `WORD` and an `LWORD` are `LWORD`s, `OR(b, lw, w)` is an `LWORD`, and `g AND h` on `BOOL`s is a `BOOL`.

**REQ-IC-analyzer-082** An operand of `AND`, `OR` or `XOR` whose operation width differs from the operation's is wrapped in an `ImplicitConversion` to the operation's type, in the operator and the function form: the `WORD` of `w OR lw` is converted to `LWORD`, and in `w AND d` on a `WORD` and a `DWORD` neither is wrapped.

**REQ-IC-analyzer-083** The result of `AND`, `OR` or `XOR` is converted to the type of a target of another operation width: in `lw := d OR e` on an `LWORD` and two `DWORD`s the `DWORD` result is converted to `LWORD`.

**REQ-IC-analyzer-084** `AND`, `OR` or `XOR` assigned to a target narrower than its widest operand is a type mismatch: `w := w OR lw` on a `WORD` and an `LWORD` reports P4035, and `w := AND(w, lw)` reports P4027.

### Assignments

An assignment stores its value at the type of its target. The pass records the conversion of the value to the type the target is stored as: its own elementary type, or its base type for a subrange. A value converts to its context when it has a type of its own -- a variable, an operation that computes at its own result type, a call to a user-defined function or a method, a dereference, or a parenthesized one of those -- and its operation width differs from the target's. An operation computes at its own type when it is arithmetic (operator or function form), an operation on one value (see Codegen), or a function of several inputs of one type (see Standard functions). A call returns the type its function or method declares, and a dereference reads the referenced variable at its own type. Any other value is compiled at the target's width rather than converted to it, so there is nothing to record: a literal takes the target's type (ADR-0028), and `TRUNC` of a `REAL` assigned to an `LINT` truncates to the target's width.

**REQ-IC-analyzer-030** A variable assigned to a target of another operation width is wrapped in an `ImplicitConversion` to the target's type: in `l := d` on an `LINT` and a `DINT` the `DINT` is converted to `LINT`.

**REQ-IC-analyzer-031** An arithmetic value is converted from its result type: in `l := d + e` on an `LINT` and two `DINT`s the `DINT` sum is converted to `LINT`.

**REQ-IC-analyzer-032** A value of the target's operation width is not converted, whatever its signedness: in `i := s` on an `INT` and a `SINT` the `SINT` is not wrapped.

**REQ-IC-analyzer-037** An operation on one value is converted from its own type: in `l := -d` on an `LINT` and a `DINT` the negation is converted to `LINT`, and so are `NOT`, `ABS` and the other numeric functions of one input, `MOVE`, and a shift or rotate.

**REQ-IC-analyzer-085** The result of a call to a user-defined function or a method, and a dereference, are converted to the type of a target of another operation width: in `l := big(u)` with `big` returning a `UDINT` the result is converted to `LINT`, in `lr := half(r)` with `half` returning a `REAL` to `LREAL`, and so are `l := k.Big()` and `l := p^` on a `REF_TO UDINT`. The code generator used to store the result unconverted, which sign-extended an unsigned result and read a `REAL`'s bits as an `LREAL` ([#2126](https://github.com/ironplc/ironplc/issues/2126)).

**REQ-IC-analyzer-034** An array element or structure field target converts the value to the element's or field's type.

**REQ-IC-analyzer-035** A function's result assignment converts the value to the function's return type, looking the result variable up in the function's scope.

**REQ-IC-analyzer-036** A subrange target converts the value to the subrange's base type.

A value assigned to a function block field is converted to the field's declared type, and one assigned through a dereference to the type the reference refers to. The type of a user-defined function block lists only the fields declared with a simple type, so the pass reads a field's declared type from the block's declaration: a subrange field is operated at its base type, like a subrange variable.

**REQ-IC-analyzer-038** A value assigned to a function block field is converted to the field's declared type: in `b.x := d` on an `LINT` field and a `DINT` the `DINT` is converted to `LINT`, and so is a `DINT` assigned to a field of a subrange of `LINT`.

**REQ-IC-analyzer-039** A value assigned through a dereference is converted to the type the reference refers to: in `r^ := d` on a `REF_TO LINT` and a `DINT` the `DINT` is converted to `LINT`.

A time or date is converted as a number is, between the short and the long type of its family. The pass used to record only the conversions of numbers and bit strings, so the code generator read a short time or date at its long target's 64 bits, which sign-extended the unsigned seconds of a date after 2038.

**REQ-IC-analyzer-099** A time or date stored in a target of the other width of its family is converted to the target's type, as an assigned value, a function block input, a method argument and a `FOR` bound are: in `ld := d`, `lt := t`, `ltod := tod` and `ldt := dt` the short value is converted to the long type, and `b(x := d)` on an `LDATE` input converts the `DATE`.

A reference refers only to a variable of the type it names (P2032), so the conversion stores a value of that variable's own type. A value the referenced type cannot hold without narrowing is rejected, as it is when assigned to the variable itself.

**REQ-IC-analyzer-080** An assignment through a dereference is checked as an assignment to the variable the reference refers to: `p^ := d` on a `REF_TO SINT` and a `DINT` reports P4035, and `p^ := f(d)` with `f` returning a `DINT` reports P4027, as `s := d` and `s := f(d)` on a `SINT` do.

A directly represented variable (`%QW0 := w`) is not recorded yet, and neither is a function block output stored by a call (`fb(OUT => x)`), which codegen stores at the field's operation type without a conversion ([#2125](https://github.com/ironplc/ironplc/issues/2125)).

### References

A reference holds the index of the variable it refers to. The analyzer knows its value by `REF_TO` and the name of the type it references (`REF_TO INT`), never as a value of that type: the name the type relations, the conversion pass and the code generator read for it. It used to be known by the referenced type's own name, so a `REF_TO REAL` passed for a `REAL` everywhere an operand's name was asked for: the code generator read the index it holds as float bits, and a `REF_TO INT` assigned to an `INT` was accepted and stored the index. A `REF_TO` parameter records the type it references, and its argument is checked as a reference to that type.

**REQ-IC-analyzer-086** A reference is known by `REF_TO` and the type it references: a `REF_TO DINT` passed to a `REF_TO INT` parameter reports P4026, and a `REF_TO INT` passed to a parameter referencing an alias of `INT` does not.

**REQ-IC-analyzer-087** A reference variable assigned to a variable that is not a reference reports P2032, and so does one assigned to a reference to another type unless type punning is allowed (`--allow-ref-type-punning`): `i := ri` and `ri := rd` on a `REF_TO INT` and a `REF_TO DINT` report P2032, and `ri2 := ri` does not.

### Subranges and fields

A value of a subrange type is operated at its base type: the name the type relations, the conversion pass and the code generator read for it is its base type's. It used to be known by the subrange's own name, which no relation judged, so a comparison fell back to the left operand's type and narrowed a wider right operand to the subrange, an arithmetic operation took the left operand's subrange type, and codegen computed both at the type of their context. A subrange parameter takes a value of its base type, as a subrange variable does when assigned.

A field of a structure has its declared type, a subrange field its base type, whether the structure is a variable, an element of an array variable or an element of an array that is itself a field. A field reached through an element of an array field (`h.items[1].a`), and a subrange field, used to have no type.

**REQ-IC-analyzer-092** A subrange operand is operated at its base type, in a comparison, an arithmetic operation and a function of several inputs of one type: in `s < l` on a subrange of `INT` and an `LINT` the subrange is converted to `LINT` where the `LINT` used to be narrowed to the subrange, `s + i` on an `INT` is an `INT`, in `s + b` on a subrange of `LINT` the subrange is converted to `LINT`, and in `l := MAX(u, u)` on a subrange of `UINT` the `UINT` result is converted to `LINT`.

**REQ-IC-analyzer-093** An operation on a subrange assigned to a target narrower than its result is a type mismatch: `d := s + b` on a `DINT`, a subrange of `INT` and a subrange of `LINT` reports P4035, as `d := i + l` does.

**REQ-IC-analyzer-094** An argument of a subrange parameter's base type is accepted, and a wider one is not: `f(i)` and `f(s + 1)` with a parameter of a subrange of `INT` report nothing, and `f(d)` on a `DINT` reports P4026.

**REQ-IC-analyzer-095** A field of a structure has its declared type wherever the structure is, and a subrange field its base type: `h.items[1].a` on an array field of structures with a `DINT` field `a` is a `DINT`, converted to `LINT` in `l := h.items[1].a`, and the subrange field `it.r` of a subrange of `INT` is an `INT`.

### Loops

A `FOR` loop stores its initial value in its control variable and compares and steps it at the control variable's type.

**REQ-IC-analyzer-079** The initial value, final value and step of a `FOR` loop are converted to the type of its control variable as an assigned value is: in `FOR l := d TO e` on an `LINT` and two `DINT`s both bounds are converted to `LINT`.

### Subscripts and shift counts

An array subscript is compiled at `DINT`, and the count of a shift or rotate at `LINT` when the shifted value is operated at 64 bits and at `DINT` otherwise, whatever their own types: the literals of both already take those types (see Literals). The pass converts a subscript or count of another width to that type as an assigned value is converted. A 64-bit subscript, or a 64-bit count of a 32-bit value, is narrowed: the code generator narrowed it before the conversion was recorded, and recording it makes the narrowing visible. Whether a subscript should index at 64 bits, and whether a count beyond the value's width should shift every bit out, are separate decisions.

**REQ-IC-analyzer-100** A subscript whose width differs from `DINT`'s is converted to `DINT`, in a value and in an assignment target: the `LINT` of `a[l]` and the `LINT` sum of `a[d + l]` are converted to `DINT`, and the `SINT` of `a[s]` is not converted.

**REQ-IC-analyzer-101** A shift or rotate count whose width differs from the count's type is converted to it: the `LINT` count of `SHL(dw, l)` on a `DWORD` is converted to `DINT`, the `DINT` count of `SHL(lw, d)` and `ROR(lw, d)` on an `LWORD` to `LINT`, and a `SINT` count of a `DWORD` and a `ULINT` count of an `LWORD` are not converted.

### Arguments

A call to a user-defined function passes each input by value at the operation width of its parameter. The pass records the conversion of an argument of another width to the type its parameter is passed as, and codegen compiles the argument at the parameter's width without choosing a conversion. A parameter is passed at its declared type, whatever kind of declaration declares it: a subrange at its base type and an alias at the type it names. An untyped literal takes its parameter's type, as it takes an assignment target's, so the parameter receives the value the program wrote. Codegen used to operate it at its default type (`DINT` or `REAL`, ADR-0028) and convert it, which rounded `f(0.1)` with an `LREAL` parameter to a `REAL` and failed on `f(5000000000)` with an `LINT` one.

**REQ-IC-analyzer-040** An argument whose operation width differs from its parameter's is wrapped in an `ImplicitConversion` to the parameter's type: the `DINT` of `f(d)` with an `LINT` parameter is converted to `LINT`, whatever kind of expression the argument is.

**REQ-IC-analyzer-041** An argument of its parameter's operation width is not converted: the `SINT` of `f(s)` with an `INT` parameter is not wrapped.

**REQ-IC-analyzer-042** An untyped literal argument takes its parameter's type: the `1` of `f(1)` with an `INT` parameter is an `INT`, the `5000000000` of `f(5000000000)` with an `LINT` one an `LINT`, and the `0.1` of `f(0.1)` with an `LREAL` one an `LREAL`.

**REQ-IC-analyzer-043** A parameter of an alias or a subrange type is passed at the type it is operated as: the type the alias names, or the subrange's base type. An argument of the parameter's own type is not converted: in `f(p)` with `p` and the parameter both of an alias of `LREAL`, `p` is not wrapped.

**REQ-IC-analyzer-044** A named argument is converted as a positional one is.

**REQ-IC-analyzer-045** The arguments of a standard function, and of a `VAR_IN_OUT` parameter, are not converted.

**REQ-IC-codegen-001** An argument narrower than its parameter is widened by its own signedness: a `UDINT` above `i32::MAX` passed to an `LINT` parameter keeps its value.

An input of a function block call is stored in a field of the block, and an argument of a method call in a parameter of the method. Both are converted as an assigned value is, to the declared type of the field or parameter (see Assignments).

**REQ-IC-analyzer-046** An input of a function block call is converted to the declared type of its field: in `b(x := d)` on an `LINT` field and a `DINT` the `DINT` is converted to `LINT`, and in `c(PV := d)` on a `CTU_LINT` it is converted to `LINT`, the declared type of the standard block's input.

**REQ-IC-analyzer-048** A positional input of a function block call is stored in the input the block declares in its place, as IEC 61131-3 binds a non-formal call: in `b(d, i)` on a block whose inputs are an `LINT` and a `REAL`, the `DINT` is converted to `LINT` and the `INT` to `REAL`.

**REQ-IC-analyzer-047** An argument of a method call is converted to the declared type of its parameter, whether it is positional or named, and in a call statement or a call expression: in `k.Set(u)` on an `LINT` parameter and a `UDINT` the `UDINT` is converted to `LINT`.

Codegen drops a positional input of a function block call ([#1855](https://github.com/ironplc/ironplc/issues/1855)), so its recorded conversion has no effect until codegen binds it. Codegen stores the inputs of a standard function block in 32 bits whatever their declared type ([#2054](https://github.com/ironplc/ironplc/issues/2054)).

### Literals

An untyped literal has a generic type (`ANY_INT`) until a context gives it one (ADR-0028). Codegen gives it one top-down: a statement passes the type it stores at into the expression, and that type flows through a negation, parentheses, an arithmetic operation and a function of several inputs of one type whose own type is generic until it reaches the literal. The pass records the type each literal reaches. An arithmetic operation of literals alone is folded to one literal before the pass runs. A literal operand of an arithmetic operation of a concrete type takes that type instead (`d + -1`), and a comparison operand or a function argument is typed by those constructs above.

**REQ-IC-analyzer-050** An untyped literal assigned to a target takes the type the target is stored at: the `1` of `l := 1` on an `LINT` is an `LINT`.

**REQ-IC-analyzer-051** The type of the context flows through a negation and parentheses: the `1` of `l := -(1)` is an `LINT`.

**REQ-IC-analyzer-053** The bounds and step of a `FOR` loop take the type of its control variable.

**REQ-IC-analyzer-054** A literal input of a function block call takes the declared type of the field it is stored in, for a user-defined block and a standard one: the `5` of `c(PV := 5)` on a `CTU` is an `INT`.

**REQ-IC-analyzer-055** A literal assigned through a dereference takes the type the reference refers to: the `5` of `r^ := 5` on a `REF_TO LINT` is an `LINT`.

**REQ-IC-analyzer-056** A literal assigned to a subrange target takes the subrange's base type.

Every construct a literal can sit in either passes the type of its context on, computes at a type of its own, or computes at a fixed type, and the literal takes the type it reaches:

**REQ-IC-analyzer-057** A function of several inputs of one type (`MIN`, `MAX`, `LIMIT`, `SEL`, `MUX`, `EXPT`, `ATAN2`) gives its literal inputs of that type its own type, not its context's: the `5` of `l := MAX(d, 5)` on an `LINT` and a `DINT` is a `DINT`, and the `DINT` result is converted to `LINT`.

**REQ-IC-analyzer-058** A literal selector of `MUX` takes `DINT`, the type `MUX` reads it as, whatever the type of the inputs it selects between, and the selector of `SEL` is the `BOOL` it is.

**REQ-IC-analyzer-059** The count of a shift or rotate takes `LINT` when the shifted value is operated at 64 bits and `DINT` otherwise.

**REQ-IC-analyzer-060** A position or length input of a string function (`LEFT`, `RIGHT`, `MID`, `INSERT`, `DELETE`, `REPLACE`) takes `DINT`.

**REQ-IC-analyzer-061** The input of a type conversion function takes the conversion's source type: the `5` of `INT_TO_REAL(5)` is an `INT`.

**REQ-IC-analyzer-062** A comparison of two untyped literals compares at the left one's default type: both literals of `1 < 2` are `DINT`s.

**REQ-IC-analyzer-063** A literal argument of a method call takes the declared type of the parameter it is passed to.

**REQ-IC-analyzer-064** A literal subscript of an array access whose subscripts are not all literals takes `DINT`.

**REQ-IC-analyzer-065** The literals of a `CASE` selector take the selector's type, and those of an `IF`, `WHILE` or `REPEAT` condition the type the condition is tested at.

**REQ-IC-analyzer-067** An operation on one value of a numeric type passes its own type, not its context's, to its literal inputs: the count `1` of `lw := SHL(w, 1)` on an `LWORD` and a `DWORD` is a `DINT`, since the `DWORD` is shifted at 32 bits.

**REQ-IC-analyzer-068** The type the pass gives an untyped literal is recorded as inferred (`ExprType::Inferred`), and a prefixed literal's stays stated (`ExprType::Concrete`), so a rule can tell a type the program wrote from one the pass chose: the `1` of `l := 1` is an inferred `LINT`, the `LINT#1` of `l := LINT#1` a stated one.

**REQ-IC-analyzer-066** A typed numeric literal keeps its own type and, in a context of another numeric type, is wrapped in an `ImplicitConversion` to it: the `UDINT#4000000000` of `l := UDINT#4000000000` on an `LINT` is converted to `LINT`.

**REQ-IC-codegen-002** An integer or real literal compiles at the type the analyzer recorded for it, not at a type its context passes down: `l := 5000000000` stores 5000000000, and `l := UDINT#4000000000` stores 4000000000.

A literal codegen builds itself has no recorded type: a member initializer of a function block instance takes the field's type in a user-defined block and is stored as a `DINT` in a standard one. A time, date, string, boolean or bit-string literal names its own type and is compiled for the storage its context gives it.

A literal a standard function does not give a type to (`TRUNC`, a typed time function such as `MUL_TIME`) takes its default type, as does a literal argument of a user-defined function whose parameter the argument pass did not give it, before the argument's conversion. The member initializers of a function block instance, which codegen compiles as expressions it builds itself, are not typed by the pass.

## Codegen

An `ImplicitConversion` compiles its operand at the operand's own type and
converts it to the type the node records. A comparison compiles at the type of
its left operand, else of its right one when the left one has no type codegen
can place. A pair of which neither operand has one is reported (P9999) rather
than compiled at the type of its context. Codegen does not choose a
comparison's operand type, and does not decide which operand to convert.

An operation on one value -- a negation, `NOT`, a numeric function of one
input (`ABS`, `SQRT`, ...), `MOVE`, or a shift or rotate, whose count only
says how far -- has its operand's type, and computes at that type when it is
numeric, as an arithmetic operation computes at its result's. Its result is
converted to the type of its context.

A function of several inputs of one type (`MAX`, `MIN`, `LIMIT`, `SEL`, `MUX`,
`EXPT`, `ATAN2`) computes at the type the analyzer recorded for the call, and
selects its builtin by that type's width and signedness. Each input compiles at
the type recorded for it, which for an input of the function's type is the
call's width (see Standard functions), and the result is converted to the type
of its context. A call without a recorded type is reported, not compiled at its
context's.

**REQ-IC-codegen-003** An operation on one value assigned to a wider target computes at its operand's type: `lw := SHL(dw, 1)` with `dw = 16#80000000` stores 0, `lw := NOT dw` with `dw = 0` stores `16#FFFFFFFF`, and `l := -d` with `d` the least `DINT` stores that `DINT`.

A variable is read at its own type and converted to its context's where that differs. A variable of a reference type holds the index of the variable it refers to, and its own type is the reference, which the analyzer knows by `REF_TO` and the type it references (see References), so it is read as that 64-bit index whatever it refers to. A read through it is a dereference, which the analyzer spells as one even for a bare `REFERENCE TO` read.

**REQ-IC-codegen-010** A variable of a reference type is read as the 64-bit index it holds, whatever type it refers to: with `q := REF(r)` on a `REAL` `r` that is not the first variable, `q^` reads `r`, and so does `q2^` after `q2 := q`; a `REF_TO LREAL` reads its target the same way.

**REQ-IC-codegen-004** A function of several inputs of one type selects its builtin at the type the analyzer recorded for the call, whatever the type of its context: `MAX` of the `UDINT`s 3000000000 and 1 passed to a `DINT` input of a function block selects 3000000000, and `l := MAX(i, l2)` with `l2 = 5000000000` stores 5000000000.

A condition of an `IF`, `ELSIF`, `WHILE` or `REPEAT` compiles at its own type,
the `BOOL` the analyzer gave it, as any other expression does. A comparison in
it reads its operand type from its operands, so the condition passes no
operand type down.

**REQ-IC-codegen-007** A condition compiles at its `BOOL` type whatever its comparisons compare: `IF NOT (a > b)` with the `DWORD`s `a = 5` and `b = 3` skips its body, and so does the same condition on `LWORD`s.

A field of a user-defined function block and a parameter of a method are
operated at their declared type, whatever kind of declaration declares them: a
subrange at its base type. A value stored through a dereference is compiled at
the type the analyzer gave it, which is the referenced type, and stored as it
is: a result of a type narrower than 32 bits is not truncated to its width, so
`p^ := a + a` on `SINT`s of 100 stores 200
([#2116](https://github.com/ironplc/ironplc/issues/2116)).

**REQ-IC-codegen-005** A value stored in a function block field, a method parameter or through a dereference keeps the value of its declared type: `b(x := 4000000000)` on a field of a subrange of `LINT` stores 4000000000, and `q^ := 1.5` on a `REF_TO REAL` stores 1.5.

**REQ-IC-codegen-014** A time or date stored in a variable or input of its long type keeps its value: `ld := d` with `d = D#2100-01-01` stores `LDATE#2100-01-01` where it stored a date thousands of millennia away, and so do `ldt := dt` after 2038, `keep(x := d)` on an `LDATE` input, `lt := t` with `t = T#-5s` and `ltod := tod`.

**REQ-IC-codegen-009** The result of a call to a user-defined function or a method, and a dereference, stored in a wider target keep their value: `l := big(u)` with `big` returning the `UDINT` 4000000000 stores 4000000000, as do `l := k.Big()` and `l := p^`, and `lr := half(3.0)` with `half` returning a `REAL` stores 1.5.

An arithmetic operation, in the operator and the function form, computes at
its own result type: a typed overload's routine, or the numeric type at whose
width the analyzer placed every operand, a subrange's being its base type. An
operation with neither, or with an operand of another width, is an internal
error rather than compiled at the type of its context, which is how a subrange
operand and a field reached through an array of structures used to compile.

**REQ-IC-codegen-012** An operation on a subrange or on a field reached through an array of structures computes at its own type, as on a variable of the base or field type: `gt := s > l` with `s = 10` on a subrange of `INT` and `l = 4294967297` stores `FALSE`, `l := s * s` with `s = 100000` on a subrange of `DINT` wraps at 32 bits as `DINT * DINT` does, `l := MAX(u, v)` with `u = 4000000000` on a subrange of `UDINT` stores 4000000000, and `l := h.items[1].a * h.items[2].a` on `DINT` fields of 100000 wraps at 32 bits.

**REQ-IC-codegen-011** `TRUNC`, `BCD_TO_INT` and `SIZEOF` compute at the integer type the analyzer recorded for the call: `x := TRUNC(r)` with `r = 2.75` and a `REAL` target stores 2.0, `l := TRUNC(lr)` with `lr = 5000000000.5` stores 5000000000, and `SIZEOF` and `BCD_TO_INT` assigned to an `LINT` store their value at 64 bits.

A parameter of a user-defined function is passed at its declared type the same way, whatever kind of declaration declares it.

**REQ-IC-codegen-006** A function's parameter of an alias or a subrange type receives the value of the type it is operated as: `pass(p)` with `p = 2.5` and a parameter of an alias of `LREAL` passes 2.5, and `pass(b)` with `b = 5000000000` and a parameter of an alias or a subrange of `LINT` passes 5000000000.

`AND`, `OR` and `XOR` compute at the type the analyzer recorded for the operation, to which it converted an operand of another width, and their result is converted to the type of its context.

A typed time or date function compiles each operand at the type the analyzer recorded for it, which is the width the routine computes at. An operand of another width is an internal error (P9998), not widened as the code generator used to widen a short operand of a long form and the number a duration is scaled by. The analyzer writes a typed fold as the calls it folds to, so no step widens the previous step's result on the stack.

An array subscript and a shift or rotate count compile at the type the analyzer recorded for them, which is `DINT`, or `LINT` for the count of a value operated at 64 bits. One of another width is an internal error, not narrowed or widened as the code generator used to.

**REQ-IC-codegen-015** A subscript or shift count whose conversion the analyzer did not record is an internal error: with the recorded conversions removed, `a[l]`, `a[d + l]`, `SHL(dw, l)` and `SHL(lw, d)` report P9998, and `a[d]` and `SHL(dw, d)` compile.

**REQ-IC-codegen-013** A typed time or date operation whose operand conversion the analyzer did not record is an internal error: with the recorded conversions removed, `lt + t`, `SUB_LDATE_LDATE(lda, da)`, `lt * d`, `MUL_LTIME(lt, r)` and `t * l` report P9998, and `lt + lt` compiles.

**REQ-IC-codegen-008** `AND`, `OR` and `XOR` keep every bit of their widest operand and widen their result by its own type: with `lw = 16#100000000` and `w = 1`, `w OR lw` and `lw OR w` give `16#100000001`, and with the `DWORD`s `d1 = 16#80000000` and `d2 = 1`, `lw := d1 OR d2` stores `16#80000001`.

## Out of scope

- A value in a context that records nothing -- a condition, a subscript, an
  input of a standard function -- is still converted by codegen.
- Codegen still converts a variable, an arithmetic result or an operation on
  one value to the type of its context where nothing recorded it, so its own
  conversion is removed only once every context records it. A literal's type
  is no longer passed down.
