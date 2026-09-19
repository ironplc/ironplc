# Arithmetic Operator Overloads

## Problem

IEC 61131-3 defines the arithmetic operators `+`, `-`, `*` and `/` as the
functions `ADD`, `SUB`, `MUL` and `DIV` (Table 55), and defines those functions
twice: once generically over `ANY_NUM` (Table 24), and once as a set of
*overloads* on the time and date types (Table 30), each with a typed name such
as `ADD_TIME` or `SUB_DATE_DATE`. An operator therefore accepts exactly the
union of what its function's overloads accept.

IronPLC has both definitions but connects neither to the operators, and does
not connect the overloaded names to the typed ones
([#1621](https://github.com/ironplc/ironplc/issues/1621)):

- The operator-form table (`operator_function_form.rs`) declares `ADD`,
  `SUB`, `MUL` and `DIV` over `ANY_NUM` only, so `ADD(t1, t2)` on `TIME` is
  rejected with P4026 although Table 30 defines it.
- The typed functions `ADD_TIME`, `SUB_DATE_DATE`, `MUL_TIME` and the rest are
  registered and have codegen that knows the storage units (`DATE` and `DT` in
  seconds, `TIME` and `TOD` in milliseconds) and promotes to floating point for
  `MUL_TIME` by a `REAL`. The operators know none of this.
- The operator spellings are not operand-checked at all. The expression type
  resolver copies the left operand's type onto the result, and codegen
  compiles the whole expression at the width of the variable it is assigned
  to. `t1 + t2` and `dt + t` compile correctly by accident; `t1 * 1.5` fails
  in codegen with P9999; `s1 + s2` on `STRING` and `x * x` on `BOOL` compile
  to nonsense; and `d1 - d2` on `DATE` is rejected with P4035 because its
  result is typed `DATE` rather than `TIME`.
- Two numeric operands of different widths compile to wrong values, because
  codegen loads each operand at the assignment's width with no conversion.
  Measured on this tree: `x := i + r` with `i : INT := 3` and
  `r : REAL := 1.5` stores 1.5 into `x : REAL`, and `x := u + l` with
  `u : UDINT := 4000000000` and `l : LINT := 1` stores -294967295 into
  `x : LINT`.

Only `MOD` is operand-checked today, by `rule_operator_operand_type_check`
against its single row.

### Building On

- **[Keyword Function Forms](keyword-function-forms.md)** — the operator-form
  table that states each function form of an operator as one row, from which
  the analyzer derives the signature and codegen the dispatch. This design
  gives the arithmetic rows their Table 30 overloads.
- **[Expression Type Resolution](expression-type-resolution.md)** — every
  `Expr` carries a `resolved_type`; this design changes how an arithmetic
  binary expression's type is chosen, and makes codegen use it.
- **[ADR-0001](../adrs/0001-bytecode-integer-arithmetic-type-strategy.md)** —
  two operation widths, operands promoted on load, results narrowed on store.
  The numeric codegen rule below is that model applied to an expression whose
  operands differ in width.
- **[ADR-0021](../adrs/0021-time-32bit-ltime-64bit.md)** — `TIME` is 32-bit
  and `LTIME` 64-bit, both in milliseconds, so the two widths of a temporal
  family convert without a unit change.
- **[ADR-0028](../adrs/0028-literal-type-inference-across-numeric-families.md)** — a bare
  integer literal is inferred as `REAL` or `LREAL` where one is expected.
- **[ADR-0029](../adrs/0029-implicit-integer-widening.md)** and
  **[ADR-0031](../adrs/0031-expanded-implicit-type-widening.md)** — the
  implicit widening rules (integer-to-integer, lossless integer-to-real,
  bit-string-to-bit-string). The numeric overload applies these between its
  two operands.
- **[ADR-0052](../adrs/0052-bit-string-arithmetic-behind-its-own-flag.md)** —
  arithmetic on bit strings is a vendor extension and gets its own flag.

## What the standard defines

The generic definitions (Table 24). All inputs of a generic function have the
same actual type, and the result has that type.

| Function | Inputs | Result |
|---|---|---|
| ADD, SUB, MUL, DIV | ANY_NUM | the input type |
| MOD | ANY_INT | the input type |

The overloads (Table 30). Each is one fixed signature, not a generic category.
The third edition renames `MULTIME` and `DIVTIME` to `MUL_TIME` and `DIV_TIME`
and adds an `L`-prefixed row for each long-width type; IronPLC uses the
third-edition names. The long-width rows have the same shapes over `LTIME`,
`LDATE`, `LTIME_OF_DAY` and `LDATE_AND_TIME` and are not repeated here.

| Overloaded | Typed name | IN1 | IN2 | Result |
|---|---|---|---|---|
| ADD | ADD_TIME | TIME | TIME | TIME |
| ADD | ADD_TOD_TIME | TIME_OF_DAY | TIME | TIME_OF_DAY |
| ADD | ADD_DT_TIME | DATE_AND_TIME | TIME | DATE_AND_TIME |
| SUB | SUB_TIME | TIME | TIME | TIME |
| SUB | SUB_DATE_DATE | DATE | DATE | TIME |
| SUB | SUB_TOD_TIME | TIME_OF_DAY | TIME | TIME_OF_DAY |
| SUB | SUB_TOD_TOD | TIME_OF_DAY | TIME_OF_DAY | TIME |
| SUB | SUB_DT_TIME | DATE_AND_TIME | TIME | DATE_AND_TIME |
| SUB | SUB_DT_DT | DATE_AND_TIME | DATE_AND_TIME | TIME |
| MUL | MUL_TIME | TIME | ANY_NUM | TIME |
| DIV | DIV_TIME | TIME | ANY_NUM | TIME |

`ANY_MAGNITUDE` (`ANY_NUM` plus `TIME`) is not the category of any arithmetic
function. The standard uses it for the comparison and selection functions,
where one category is right. It is wrong for arithmetic: it would admit
`TIME * TIME`, which no row defines.

Nothing in the standard defines arithmetic on `ANY_BIT`, `ANY_STRING`, or
between two different numeric types. Arithmetic on bit strings is a vendor
extension (CODESYS, TwinCAT and RuSTy treat a bit string as the unsigned
integer of its width), which is why it sits behind a flag below.

## Architecture

One resolver answers, for an arithmetic operator and two operand types, which
overload applies and what its result type is. The type resolver and the
operator rule ask it in the analyzer; codegen asks its typed step to pick the
routine. The tree is not rewritten: `a + b` stays a `BinaryOp` and `ADD(a, b)`
stays a `Function`, as the user wrote them.

```
Parser ──> a + b            (BinaryOp)      ADD(a, b)        (Function)
                 │                                │
                 v                                v
xform_resolve_expr_types ── resolver ──> result type on the Expr
                 │                                │
                 v                                v
xform_fold_constant_expressions (unchanged: folds literal BinaryOps)
                 │                                │
                 v                                v
rule_operator_operand_type_check ── resolver ──> P4049 when none,
                                                 for both spellings
                 │                                │
                 v                                v
Codegen: BinaryOp ── typed step ──> typed routine (units, width)
                                    or the numeric opcode at the
                                    expression's resolved width
         Function ── typed step ──> the same, folded from the left
```

The numeric overload compiles as it does today, through the one emit function
that both spellings already share. A typed overload compiles as the typed
routine, because the typed routine is the code that knows the units, and both
spellings reach it through the same dispatch.

### The overload table

The `ADD`, `SUB`, `MUL` and `DIV` rows of the operator-form table gain one
column: the typed names of their Table 30 overloads. The row keeps its generic
operand category for the numeric overload. The typed names are only names; the
signature of each typed function stays where it is registered today, in
`get_time_functions`, so a typed overload is still defined once. A test pins
that every name in the column is a registered two-input signature, since the
column cannot be checked by the compiler.

```rust
form("ADD", FormOf::Arithmetic(Operator::Add), Arity::Extensible, "ANY_NUM",
     FormResult::Operand, &["ADD_TIME", "ADD_TOD_TIME", "ADD_DT_TIME"]),
form("SUB", ..., &["SUB_TIME", "SUB_DATE_DATE", "SUB_TOD_TIME", "SUB_TOD_TOD",
                   "SUB_DT_TIME", "SUB_DT_DT"]),
form("MUL", ..., &["MUL_TIME"]),
form("DIV", ..., &["DIV_TIME"]),
form("MOD", ..., &[]),
```

The column names the short-width form of each overload. Each short form has a
long form (`ADD_LTIME`, `SUB_LDATE_LDATE`, `MUL_LTIME`, and so on), registered
alongside the short one in `get_time_functions` with the same shape over the
long-width types. The resolver derives the long name from the short one, so
the column lists each overload once. The long forms are registered whether or
not `--allow-long-time-types` is on; without the flag no program can declare
an operand of a long type, so the rows are unreachable rather than harmful.
Without them the resolver would have no row for `lt1 + lt2`, which compiles
correctly today.

### Resolution

The resolver lives in its own module beside the table,
`intermediates/arithmetic_overload.rs`, not in `xform_resolve_expr_types.rs`,
which is already past the module size limit.

`resolve_arithmetic_overload(op, left, right, options)` returns
`Option<Overload>`, where an overload is `Unchecked { result }`,
`Numeric { result }` or `Typed { name, result }`, and `None` means the
operator is not defined for the pair. It is a pure function of its arguments,
so calling it from more than one pass needs no annotation on the tree.

1. If either operand type is one the compatibility predicate cannot judge (a
   subrange, an enumeration, a structure, a user type), return `Unchecked`
   with the left operand's type. This is today's behaviour, and the operator
   rule already skips such operands. It is a separate answer from `Numeric`
   so that nothing downstream mistakes "not judged" for "judged numeric".
2. Try the numeric overload against the row's category. Both operands must be
   in the category, and one must be acceptable where the other is expected
   under the existing `are_types_compatible` predicate (implicit widening per
   ADR-0029 and ADR-0031, literal inference per ADR-0028). The result is the
   operand the other widens to: `INT + DINT` is `DINT`, `INT + REAL` is
   `REAL`, `DINT + 1` is `DINT`, `REAL + 1` is `REAL`. `DINT + REAL` has no
   result, because `DINT` does not widen losslessly to `REAL`.
3. Otherwise try the typed step, `typed_overload(op, left, right)`. It takes
   no options, because no typed row depends on a flag, which is what lets
   codegen call it. Each operand is judged where the corresponding parameter
   is expected: a temporal parameter matches an operand of the same temporal
   family at either width (`TIME` or `LTIME` where `TIME` is written), and
   the `ANY_NUM` parameter of `MUL_TIME` and `DIV_TIME` matches by the
   ordinary category. The families of a row are pairwise distinct from the
   other rows of the same function, so at most one row matches. The row's
   form is then the long one if either temporal operand is long-width, and
   the short one otherwise; the result is that form's return type. So
   `t1 + t2` is `ADD_TIME`, `lt1 + lt2` is `ADD_LTIME`, `t + lt` is
   `ADD_LTIME` with result `LTIME`, and `lt + LTIME#1s` is `ADD_LTIME`
   although the literal is typed `TIME`.
4. Otherwise return `None`.

Step 3 matches by family rather than by exact type for the reason
`are_types_compatible` does: every duration and date literal resolves to the
short name of its family whatever prefix it was written with, so an exact
match would reject `lt + LTIME#1s`, which compiles correctly today. The width
rule is what ADR-0021 makes safe: both widths of a family share a unit, so
promoting the short operand is a sign extension with no conversion.

With `--allow-bit-string-arithmetic` (ADR-0052), step 2 additionally judges a
`BYTE`, `WORD`, `DWORD` or `LWORD` operand (not `BOOL`) as the unsigned
integer of its width. Two bit-string operands give the wider bit-string type;
a bit-string operand and an integer operand give what the widening picks with
the bit string standing for its unsigned integer, so `b + 1` on `BYTE` is
`BYTE`, `b + i` with `i : INT` is `INT`, and `b + r` with `r : REAL` is
`REAL`. This is the only non-standard behaviour in this design. The predicate
cannot express it, since `BYTE` is not in `ANY_NUM` under any flag, so it is
a rule of the resolver and not of the predicate.

An extensible call with more than two inputs (`ADD(a, b, c)`) folds from the
left: the resolver runs on the first two inputs, then on that result and the
third, and so on. `ADD(t1, t2, t3)` is therefore two `ADD_TIME` steps, which
is what `t1 + t2 + t3` is, so the function form accepts exactly what the
operator accepts (the invariant of Keyword Function Forms). Codegen folds the
same way, asking the typed step at each step with the accumulated result type.

### Type resolution

`xform_resolve_expr_types` asks the resolver for the type of an arithmetic
`BinaryOp` and of a call to an overloaded name. The result type of a
resolved overload replaces today's "left operand" rule. When the resolver
returns `Unchecked` or `None`, the pass keeps today's rule so later passes
still see a type and the operator rule can report the operands.

### Rules

`rule_operator_operand_type_check` checks every arithmetic operator, not only
`MOD`, and also every call to one of the four overloaded names, by asking the
resolver; `checked_form` goes away. It reports P4049 when the answer is
`None`. Both spellings of an operator get the same diagnostic, so `t * r` and
`MUL(t, r)` are reported the same way. P4049 changes its context from an
`expected` category to the two operand types, because an operator with
overloads has no single expected type, and its message changes from "the
operand type" to "the operand types":

```
error[P4049]: Operator is not defined for the operand types
              (operator=*, left=TIME, right=REAL)
```

`rule_function_call_type_check` no longer checks the inputs of a call to one
of the four overloaded names, since the operator rule has reported them. It
still checks their arity (P4018) and every other function's inputs (P4026),
including a call to a typed name such as `ADD_TIME(a, b)`, which is not
resolved: it is bound to its own registered signature as today.

### Codegen

Codegen changes in two places.

**Typed dispatch.** The `BinaryOp` arm of `compile_expr` and the operator-form
fold in `compile_call.rs` ask the typed step. When it answers, they compile
the two operands through the typed routine named by the answer, the routine
that `ADD_TIME(a, b)` already reaches by name. The typed routines take their
two operand expressions rather than a `Function`, so both spellings share
them, and take their operation width from the typed name (32-bit for the
short form, 64-bit for the long one) instead of hard-coding 32-bit, so the
long forms compile. A short-width operand of a long form is loaded at 32
bits and sign-extended, as ADR-0001 loads any narrower integer.

**Numeric width.** A numeric binary expression compiles at the width of its
resolved type, not at the width of the variable it is assigned to. An operand
whose natural width differs is compiled at its own width and followed by the
conversion opcode, as `compile_user_function_call` already does for an
argument; the result is converted to the enclosing width after the opcode.
This is ADR-0001's promote-operate-truncate applied to the expression rather
than to the statement: `INT + REAL` loads the `INT` at 32 bits, converts it
to `REAL`, and adds as `REAL`; `UDINT + LINT` zero-extends the `UDINT` before
the 64-bit add. It also changes an expression whose operands are narrower than
its target: `l := d1 * d2` with `l : LINT` and `DINT` operands multiplies at
32 bits and widens the product, which is what the standard's "the result has
the input type" means, rather than multiplying at 64 bits as today. An
expression whose operands and target share a width compiles to the same
bytecode as before.

### Considered: lowering to typed calls

An earlier draft rewrote every operator that resolved to a typed overload
into a call of the typed name, in a transform after constant folding, so that
codegen would only ever see calls it already compiles. That is one more pass,
a call node in the analyzed tree that the user did not write, a fabricated
span on its name, and a rule whose correctness depends on the pass having
run. Codegen already depends on the analyzer's table (`compile_function_call`
looks up `operator_function_form`), so asking the typed step from codegen is
the same dependency with no new node, and the tree stays what the parser
produced for every consumer of it. The rewrite was dropped.

### Relationship to Keyword Function Forms

[Keyword Function Forms](keyword-function-forms.md) says the operator rule
checks only `MOD` and cites #1621 for the rest, and REQ-KF-analyzer-005 says
an argument outside a function form's category is P4026. The change that
implements this design updates that paragraph and REQ-KF-analyzer-001 to say
that `ADD`, `SUB`, `MUL` and `DIV` accept the numeric overload or one of the
typed overloads listed here, and narrows REQ-KF-analyzer-005 to the forms
without overloads, since the four arithmetic forms report P4049 through the
operator rule. The module documentation of
`rule_operator_operand_type_check.rs` cites #1621 in the same way and is
updated with it.

### Behaviour that changes

Programs that analyze cleanly today and are reported after this design:

| Program | Today | After | Why |
|---|---|---|---|
| `b + 1` on `BYTE`, strict dialect | clean, correct | P4049 | not defined by the standard; the flag is on in the `Rusty`, `CODESYS` and `TwinCAT` dialects |
| `s1 + s2` on `STRING` | clean, nonsense | P4049 | not defined |
| `x * x` on `BOOL` | clean, nonsense | P4049 | not defined |
| `t1 * t2` on `TIME` | clean, nonsense | P4049 | no Table 30 row |
| `r + d` on `REAL` and `DINT` | clean, wrong value | P4049 | `DINT` does not widen losslessly to `REAL` |

Programs whose value changes:

| Program | Today | After |
|---|---|---|
| `x : REAL := i + r`, `i : INT := 3`, `r : REAL := 1.5` | 1.5 | 4.5 |
| `x : LINT := u + l`, `u : UDINT := 4000000000`, `l : LINT := 1` | -294967295 | 4000000001 |
| `l : LINT := d1 * d2` on `DINT` operands | 64-bit product | 32-bit product, widened |

Programs that keep working: `t1 + t2`, `t + lt`, `lt + LTIME#1s`, `dt + t`,
`tod - tod`, `ADD_TIME(t1, t2)`, and `b + 1` under any of the three dialects.

## Requirements

### Resolution

**REQ-AO-analyzer-001** Two operands of the same elementary numeric type resolve to the numeric overload with that result type.

**REQ-AO-analyzer-002** Two numeric operands where one implicitly widens to the other resolve to the numeric overload with the wider type as the result type.

**REQ-AO-analyzer-003** A bare integer literal operand resolves with the other operand's numeric type, including `REAL` and `LREAL`.

**REQ-AO-analyzer-004** Two numeric operands where neither widens to the other, such as `DINT` and `REAL`, do not resolve.

**REQ-AO-analyzer-005** Each Table 30 pair of operand types resolves the overloaded function to its typed name with the typed function's return type.

**REQ-AO-analyzer-006** Each pair of long-width operand types resolves to the long form of the typed name and not to the short one.

**REQ-AO-analyzer-007** A pair mixing the two widths of one temporal family resolves to the long form with its result type, so `t + lt` and `lt + LTIME#1s` are `ADD_LTIME` with result `LTIME`.

**REQ-AO-analyzer-008** `TIME` multiplied or divided by an `ANY_NUM` operand resolves to `MUL_TIME` or `DIV_TIME`; an `ANY_NUM` multiplied by `TIME` does not resolve.

**REQ-AO-analyzer-009** Operands of `ANY_STRING`, `BOOL`, or two different temporal families with no Table 30 row do not resolve.

**REQ-AO-analyzer-010** With `--allow-bit-string-arithmetic`, a `BYTE`, `WORD`, `DWORD` or `LWORD` operand resolves to the numeric overload as the unsigned integer of its width, and the result is the wider bit string for two bit-string operands or the widened type for a bit-string and an integer operand.

**REQ-AO-analyzer-011** Without `--allow-bit-string-arithmetic`, a bit-string operand does not resolve.

**REQ-AO-analyzer-012** An operand whose type the compatibility predicate cannot judge resolves as unchecked with the left operand's type, distinct from the numeric overload.

**REQ-AO-analyzer-013** An extensible call with more than two inputs resolves by folding from the left, so `ADD(t1, t2, t3)` resolves and `ADD(t1, t2, r)` does not.

**REQ-AO-analyzer-014** Every typed name listed in the operator-form table, in both widths, is a registered function signature with two inputs.

### Type resolution

**REQ-AO-analyzer-020** The resolved type of an arithmetic binary expression is the result type of its resolved overload.

**REQ-AO-analyzer-021** The resolved type of a call to an overloaded name is the result type of its resolved overload, so `SUB(d1, d2)` on `DATE` is `TIME`.

**REQ-AO-analyzer-022** An arithmetic binary expression or call that resolves as unchecked or does not resolve keeps the left operand's type.

**REQ-AO-analyzer-023** The analyzed tree contains no node the parser did not produce for an arithmetic expression: a resolved operator stays a binary expression and a resolved call stays a call.

**REQ-AO-analyzer-024** Constant folding of literal arithmetic is unaffected.

### Diagnostics

**REQ-AO-analyzer-030** An arithmetic operator whose operands do not resolve is reported as P4049 naming the operator and both operand types.

**REQ-AO-analyzer-031** Every arithmetic operator (`+`, `-`, `*`, `/`, `MOD`) is checked, so `r MOD 2.0`, `t1 * 1.5`, `s1 + s2` and `x * x` on `BOOL` are each reported as P4049.

**REQ-AO-analyzer-032** A call to an overloaded name whose inputs do not resolve is reported as P4049 naming the function and the two operand types of the failing step, and not as P4026.

**REQ-AO-analyzer-033** An operator expression that resolves is reported as nothing, so `t1 + t2`, `tod + t`, `d1 - d2`, `t + lt` and `INT + DINT` are clean.

**REQ-AO-analyzer-034** A function call on a Table 30 pair, such as `ADD(t1, t2)`, is clean.

**REQ-AO-analyzer-035** A call to a typed name is checked against its own signature as today, so `ADD_TIME(t1, t2)` is clean and `ADD_TIME(t1, r)` is P4026.

### Codegen

**REQ-AO-codegen-001** An operator expression on a Table 30 pair compiles to the same bytecode as the call to its typed name on the same operands.

**REQ-AO-codegen-002** `dt + t` on `DATE_AND_TIME` and `TIME` computes the value `ADD_DT_TIME` computes, with the millisecond-to-second conversion.

**REQ-AO-codegen-003** `t * r` on `TIME` and `REAL` computes the value `MUL_TIME` computes, with floating-point promotion.

**REQ-AO-codegen-004** `d1 - d2` on `DATE` computes a `TIME` in milliseconds.

**REQ-AO-codegen-005** The long forms of the typed functions compute at 64-bit width, and a short-width operand of a long form is sign-extended.

**REQ-AO-codegen-006** A numeric operator expression whose operands and assignment target share an operation width compiles to the same bytecode as before this design.

**REQ-AO-codegen-007** A numeric operator expression with operands of different widths computes at the resolved type's width with the narrower operand converted first, so `INT + REAL` gives 4.5 for 3 and 1.5 and `UDINT + LINT` gives 4000000001 for 4000000000 and 1.

**REQ-AO-codegen-008** A numeric operator expression assigned to a wider variable computes at its resolved width and converts the result, so `DINT * DINT` assigned to `LINT` wraps at 32 bits.

**REQ-AO-codegen-009** An extensible call on a Table 30 pair compiles to the typed routine folded from the left, so `ADD(t1, t2, t3)` computes what `t1 + t2 + t3` computes.

## Out of scope

- `**` (`EXPT`) has no row in the operator-form table and is not checked.
  It stays as it is.
- Unary negation of a `TIME` is not a Table 30 row and is not checked.
- Comparisons, and `AND`, `OR`, `XOR` and `NOT`, already resolve against
  `ANY_ELEMENTARY` and `ANY_BIT` and are dispatched by the operator.
- The standard's same-type rule for the numeric overload is relaxed to the
  project's implicit widening, as it already is for function arguments. A
  strict mode is not proposed.
- A call to a typed name on operands of the other width, `ADD_TIME(lt1, lt2)`,
  is accepted by the call rule today because the predicate treats the widths
  of a family as interchangeable, and computes at the typed name's width. That
  stays as it is; the resolver does not apply to typed names.
- `MULTIME` and `DIVTIME`, the second-edition spellings, are not registered
  today and this design does not add them.

## Testing

- The resolver is unit-tested as a pure function over pairs of type names,
  one case per requirement above, with the bit-string cases run under both
  flag settings.
- The spec conformance module for this design extends the pattern of
  `spec_conformance_keyword_function_forms.rs`: one program per Table 30 row
  in operator and function form, at both widths, asserted clean, and one
  program per rejected pair, asserted P4049.
- The codegen conformance tests assert bytecode equality between the operator
  spelling and the typed call, and end-to-end tests run each Table 30 row on
  the VM and compare against the value of the typed function. The mixed-width
  numeric cases in *Behaviour that changes* are end-to-end tests with the
  values in that table.
- `plc2plc` does not run the analyzer, so its round-trip tests need no
  change. The LSP and MCP tools do not read the analyzed tree today, and
  REQ-AO-analyzer-023 keeps the tree as the parser produced it, so they need
  none either.

## Documentation

- `docs/reference/compiler/problems/P4049.rst` is rewritten to describe all
  arithmetic operators, both spellings, and the two-operand message; the
  message in `problem-codes.csv` changes with it.
- `docs/reference/compiler/problems/P4026.rst` no longer lists the function
  forms of the arithmetic operators as a case.
- The `ADD`, `SUB`, `MUL` and `DIV` reference pages gain their Table 30
  overloads, each linking to the typed function's page.
- The eleven long forms (`ADD_LTIME`, `SUB_LDATE_LDATE`, `MUL_LTIME`, and so
  on) each get a reference page under `docs/reference/standard-library/functions/`,
  as every short form has.
- `--allow-bit-string-arithmetic` is added to `docs/reference/compiler/ironplcc.rst`,
  to the dialect table in `docs/explanation/enabling-dialects-and-features.rst`,
  and to `docs/explanation/type-conversions.rst` as the bit-string arithmetic
  case.
