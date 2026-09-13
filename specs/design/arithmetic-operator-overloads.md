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
  resolver copies the left operand's type onto the result, and codegen picks
  the opcode from that type. `t1 + t2` compiles correctly by accident;
  `t1 * 1.5` and `r + d` (`REAL` plus `DINT`) compile to wrong values;
  `s1 + s2` on `STRING` and `x * x` on `BOOL` compile to nonsense; and
  `d1 - d2` on `DATE` is rejected with P4035 because its result is typed `DATE`
  rather than `TIME`.

Only `MOD` is operand-checked today, by `rule_operator_operand_type_check`
against its single row.

### Building On

- **[Keyword Function Forms](keyword-function-forms.md)** — the operator-form
  table that states each function form of an operator as one row, from which
  the analyzer derives the signature and codegen the dispatch. This design
  gives the arithmetic rows their Table 30 overloads.
- **[Expression Type Resolution](expression-type-resolution.md)** — every
  `Expr` carries a `resolved_type`; this design changes how an arithmetic
  binary expression's type is chosen.
- **[ADR-0028](../adrs/0028-literal-type-inference-across-numeric-families.md)** — a bare
  integer literal is inferred as `REAL` or `LREAL` where one is expected.
- **[ADR-0029](../adrs/0029-implicit-integer-widening.md)** and
  **[ADR-0031](../adrs/0031-expanded-implicit-type-widening.md)** — the
  implicit widening rules (integer-to-integer, lossless integer-to-real,
  bit-string-to-bit-string) and the `--allow-cross-family-widening` flag for
  bit-string-to-integer. The numeric overload applies these between its two
  operands.

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
third-edition names.

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
extension (CODESYS and TwinCAT treat a bit string as the unsigned integer of
its width), which is why it sits behind a flag below.

## Architecture

One resolver answers, for an arithmetic operator and two operand types, which
overload applies and what its result type is. Three places ask it, and codegen
never sees an operator it cannot compile:

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
xform_lower_arithmetic_overloads ── resolver ──> typed overload?
                 │                                │
        numeric: unchanged              numeric: unchanged
        typed:   ADD_TIME(a, b)         typed:   ADD_TIME(a, b)
        none:    unchanged              none:    unchanged
                 │                                │
                 v                                v
rule_operator_operand_type_check ── resolver ──> P4049 when none
rule_function_call_type_check (unchanged)   ──> P4026 when none
                 │                                │
                 v                                v
Codegen (unchanged): BinaryOp ──> opcode by resolved type
                     Function ──> operator-form fold, or typed routine
```

The numeric overload is left as an operator. Codegen already compiles `a + b`
and `ADD(a, b)` through one emit function, so lowering them would change
nothing and would make diagnostics name a function the user never wrote. Only
a call that resolves to a typed overload is rewritten, because the typed
routine is the code that knows the units.

### The overload table

The `ADD`, `SUB`, `MUL` and `DIV` rows of the operator-form table gain one
column: the typed names of their Table 30 overloads. The row keeps its generic
operand category for the numeric overload. The typed names are only names; the
signature of each typed function stays where it is registered today, in
`get_time_functions`, so a typed overload is still defined once.

```rust
form("ADD", FormOf::Arithmetic(Operator::Add), Arity::Extensible, "ANY_NUM",
     FormResult::Operand, &["ADD_TIME", "ADD_TOD_TIME", "ADD_DT_TIME"]),
form("SUB", ..., &["SUB_TIME", "SUB_DATE_DATE", "SUB_TOD_TIME", "SUB_TOD_TOD",
                   "SUB_DT_TIME", "SUB_DT_DT"]),
form("MUL", ..., &["MUL_TIME"]),
form("DIV", ..., &["DIV_TIME"]),
form("MOD", ..., &[]),
```

The long-width rows (`ADD_LTIME`, `SUB_LDATE_LDATE`, `MUL_LTIME`, and so on)
are registered alongside the short ones with the same shapes over `LTIME`,
`LDATE`, `LTIME_OF_DAY` and `LDATE_AND_TIME`, and listed in the same column.
Without them the resolver would have no row for `lt1 + lt2`, which compiles
correctly today.

### Resolution

`resolve_arithmetic_overload(op, left, right, options)` returns
`Option<Overload>`, where an overload is either `Numeric { result }` or
`Typed { name, result }`. It is a pure function of its arguments, so calling
it from three passes needs no annotation on the tree.

1. If either operand type is one the compatibility predicate cannot judge (a
   subrange, an enumeration, a structure, a user type), return `Numeric` with
   the left operand's type. This is today's behaviour, and the operator rule
   already skips such operands.
2. Try the numeric overload against the row's category. Both operands must be
   in the category, and one must be acceptable where the other is expected
   under the existing `are_types_compatible` predicate (implicit widening per
   ADR-0029 and ADR-0031, literal inference per ADR-0028). The result is the
   operand the other widens to: `INT + DINT` is `DINT`, `INT + REAL` is
   `REAL`, `DINT + 1` is `DINT`, `REAL + 1` is `REAL`. `DINT + REAL` has no
   result, because `DINT` does not widen losslessly to `REAL`.
3. Otherwise try each typed name in the row, in table order. A typed overload
   matches when each operand is acceptable where the corresponding parameter
   is expected, judged by exact elementary type for the temporal parameters
   (so `LTIME` matches the `ADD_LTIME` row and not the `ADD_TIME` row) and by
   the ordinary predicate for the `ANY_NUM` parameter of `MUL_TIME` and
   `DIV_TIME`. The result is the typed function's return type. The rows of a
   function are pairwise disjoint on at least one parameter, so at most one
   matches.
4. Otherwise return `None`.

With `--allow-cross-family-widening`, step 2 additionally admits `BYTE`,
`WORD`, `DWORD` and `LWORD` (not `BOOL`) as the unsigned integer of the same
width. The result keeps the operand's own type, so `b + 1` on `BYTE` is
`BYTE`, as it is today. This is the only non-standard behaviour in this
design and the existing flag is its natural home: the flag already governs
the bit-string-to-integer boundary and CODESYS and TwinCAT, which the flag
exists to follow, accept this arithmetic.

An extensible call with more than two inputs (`ADD(a, b, c)`) resolves only
against the numeric overload, folded from the left. The typed overloads are
binary, as they are in the standard; `t1 + t2 + t3` is two nested binary
expressions and resolves twice.

### Type resolution

`xform_resolve_expr_types` asks the resolver for the type of an arithmetic
`BinaryOp` and of a call to an overloaded name. The result type of a
resolved overload replaces today's "left operand" rule. When the resolver
returns `None`, the pass keeps today's rule so later passes still see a type
and the operator rule can report the operands.

### Lowering

`xform_lower_arithmetic_overloads` is a `Fold` that runs last among the
transforms, after constant folding. For an arithmetic `BinaryOp` or a call to
an overloaded name whose resolver answer is `Typed`, it produces
`ExprKind::Function` naming the typed function, with the two operands as
positional inputs, the expression's joined span as the name's span, and the
overload's result as `resolved_type`. Everything else passes through
unchanged.

The typed call it produces is the call the user could have written, so the
call rules and codegen already handle it. Codegen's dispatch on the typed
name reaches the routine with the unit conversion, and P4035 on
`diff := d1 - d2` goes away because the value is now typed `TIME`.

### Rules

`rule_operator_operand_type_check` checks every arithmetic operator, not only
`MOD`, by asking the resolver; `checked_form` goes away. It reports P4049
when the answer is `None`. Since the lowering already replaced every operator
with a typed overload, the rule only ever sees the numeric case and the
failures. P4049 changes its context from an `expected` category to the two
operand types, because an operator with overloads has no single expected
type:

```
error[P4049]: Operator is not defined for the operand types
              (operator=*, left=TIME, right=REAL)
```

`rule_function_call_type_check` is unchanged. A call that resolved to a typed
overload was lowered to that typed name and passes; a call that did not is
still bound to the row's `ANY_NUM` signature and reports P4026 per input, as
today.

### Codegen

The only codegen change is that the typed time routines take their operation
width from the typed name (32-bit for the short types, 64-bit for the long
ones) instead of hard-coding 32-bit, so the long-width rows compile. The
operator path and the operator-form fold are untouched.

### Relationship to Keyword Function Forms

[Keyword Function Forms](keyword-function-forms.md) says the operator rule
checks only `MOD` and cites #1621 for the rest. The change that implements
this design updates that paragraph and its REQ-KF-analyzer-001 to say that
`ADD`, `SUB`, `MUL` and `DIV` accept the numeric overload or one of the typed
overloads listed here.

## Requirements

### Resolution

**REQ-AO-analyzer-001** Two operands of the same elementary numeric type resolve to the numeric overload with that result type.

**REQ-AO-analyzer-002** Two numeric operands where one implicitly widens to the other resolve to the numeric overload with the wider type as the result type.

**REQ-AO-analyzer-003** A bare integer literal operand resolves with the other operand's numeric type, including `REAL` and `LREAL`.

**REQ-AO-analyzer-004** Two numeric operands where neither widens to the other, such as `DINT` and `REAL`, do not resolve.

**REQ-AO-analyzer-005** Each Table 30 pair of operand types resolves the overloaded function to its typed name with the typed function's return type.

**REQ-AO-analyzer-006** Each long-width pair of operand types resolves to the long-width typed name and not to the short-width one.

**REQ-AO-analyzer-007** `TIME` multiplied or divided by an `ANY_NUM` operand resolves to `MUL_TIME` or `DIV_TIME`; an `ANY_NUM` multiplied by `TIME` does not resolve.

**REQ-AO-analyzer-008** Operands of `ANY_BIT`, `ANY_STRING`, `BOOL`, or two different temporal types with no Table 30 row do not resolve.

**REQ-AO-analyzer-009** With `--allow-cross-family-widening`, a `BYTE`, `WORD`, `DWORD` or `LWORD` operand resolves to the numeric overload as an unsigned integer of its width and the result keeps the bit-string type.

**REQ-AO-analyzer-010** Without `--allow-cross-family-widening`, a bit-string operand does not resolve.

**REQ-AO-analyzer-011** An operand whose type the compatibility predicate cannot judge resolves to the numeric overload with the left operand's type.

**REQ-AO-analyzer-012** An extensible call with more than two inputs resolves only against the numeric overload.

### Type resolution and lowering

**REQ-AO-analyzer-020** The resolved type of an arithmetic binary expression is the result type of its resolved overload.

**REQ-AO-analyzer-021** The resolved type of a call to an overloaded name is the result type of its resolved overload, so `SUB(d1, d2)` on `DATE` is `TIME`.

**REQ-AO-analyzer-022** A binary expression that resolves to a typed overload is lowered to a call of the typed name with the operands as positional inputs, in that order.

**REQ-AO-analyzer-023** A call to an overloaded name that resolves to a typed overload is lowered to a call of the typed name.

**REQ-AO-analyzer-024** A binary expression or call that resolves to the numeric overload is not rewritten.

**REQ-AO-analyzer-025** A lowered call carries the joined span of the original expression on its name.

**REQ-AO-analyzer-026** Constant folding of literal arithmetic is unaffected by lowering.

### Diagnostics

**REQ-AO-analyzer-030** An arithmetic operator whose operands do not resolve is reported as P4049 naming the operator and both operand types.

**REQ-AO-analyzer-031** Every arithmetic operator (`+`, `-`, `*`, `/`, `MOD`) is checked, so `r MOD 2.0`, `t1 * 1.5`, `s1 + s2` and `x * x` on `BOOL` are each reported as P4049.

**REQ-AO-analyzer-032** A call to an overloaded name whose arguments do not resolve is reported as P4026.

**REQ-AO-analyzer-033** An operator expression that resolves is reported as nothing, so `t1 + t2`, `tod + t`, `d1 - d2` and `INT + DINT` are clean.

**REQ-AO-analyzer-034** A function call on a Table 30 pair, such as `ADD(t1, t2)`, is clean.

### Codegen

**REQ-AO-codegen-001** An operator expression on a Table 30 pair compiles to the same bytecode as the call to its typed name on the same operands.

**REQ-AO-codegen-002** `dt + t` on `DATE_AND_TIME` and `TIME` computes the value `ADD_DT_TIME` computes, with the millisecond-to-second conversion.

**REQ-AO-codegen-003** `t * r` on `TIME` and `REAL` computes the value `MUL_TIME` computes, with floating-point promotion.

**REQ-AO-codegen-004** `d1 - d2` on `DATE` computes a `TIME` in milliseconds.

**REQ-AO-codegen-005** The long-width typed functions compile at 64-bit width.

**REQ-AO-codegen-006** A numeric operator expression compiles to the same bytecode as before this design.

## Out of scope

- `**` (`EXPT`) has no row in the operator-form table and is not checked.
  It stays as it is.
- Unary negation of a `TIME` is not a Table 30 row and is not checked.
- Comparisons, and `AND`, `OR`, `XOR` and `NOT`, already resolve against
  `ANY_ELEMENTARY` and `ANY_BIT` and are dispatched by the operator; they are
  not lowered.
- The standard's same-type rule for the numeric overload is relaxed to the
  project's implicit widening, as it already is for function arguments. A
  strict mode is not proposed.

## Testing

- The resolver is unit-tested as a pure function over pairs of type names,
  one case per requirement above.
- The spec conformance module for this design extends the pattern of
  `spec_conformance_keyword_function_forms.rs`: one program per Table 30 row
  in operator and function form, asserted clean, and one program per
  rejected pair, asserted P4049 or P4026.
- The codegen conformance tests assert bytecode equality between the operator
  spelling and the typed call, and end-to-end tests run each Table 30 row on
  the VM and compare against the value of the typed function.
- `plc2plc` does not run the analyzer, so its round-trip tests need no
  change. The LSP and MCP tools consume the analyzed tree; a test asserts a
  lowered call does not appear as a symbol reference.

## Documentation

- `docs/reference/compiler/problems/P4049.rst` is rewritten to describe all
  arithmetic operators and the two-operand message.
- The `ADD`, `SUB`, `MUL` and `DIV` reference pages gain their Table 30
  overloads, each linking to the typed function's page.
- `docs/explanation/type-conversions.rst` gains the bit-string arithmetic
  case under the cross-family widening flag.
