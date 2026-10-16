# Plan: Lower Calls with Outputs Inside an Expression

Checked against `main` at `0a1d310` (after #2316). File and line references are
as of that commit.

## Goal

`r := F(x := 3, n => n1) + F(x := 4, n => n2)` compiles from the AST
(`codegen/tests/it/end_to_end_call_outputs.rs`,
`end_to_end_when_function_called_twice_then_output_starts_again`). After this
work it takes the lowered route, and the bytecode is the same as the AST route's.

Every shape where hoisting would change what the program does stays on the
AST route, and so does every shape this backend cannot emit as the AST route
does. `just lowered-count` rises by 1. That statement is the only one in the
end-to-end programs with a call with outputs inside an expression.

## Current behaviour

| Where | What it does with `r := F(a, q => x) + G(b, p => y)` |
|---|---|
| Lowering (`lowering/src/statement.rs:29`, `function.rs` `call_with_outputs`, `assignment_of_call`) | Lowers a call with outputs only when it is the whole value of an assignment, inside the conversions the analyzer recorded (REQ-LOW-lowering-172). Anywhere else `user_call` finds no `call_result` for it and declines, so the statement compiles from the AST. |
| Design (`lowered-program.md`, the paragraphs before REQ-LOW-lowering-172) | "A call of a function with outputs anywhere else, such as inside an expression or a condition, stays on the route that reads the AST." The method section lists the same limit. |
| AST route (`compile_user_call.rs:79-93`, `compile_method.rs:475-487`, `compile_call_output.rs` `store_outputs`) | Compiles the left operand: the inputs, `CALL`, then the store of each output while the result stays on the stack. Then it compiles the right operand the same way, then `ADD`, then the store to `r`. For a method, `METHOD_CALL` leaves the instance beneath the result, and `compile_method_call_expression` drops it once the outputs are stored. |
| Bridge (`codegen/src/lowered/fn_call.rs`) | Recognizes the three parts of REQ-LOW-lowering-172 (`output_call`) and emits them as the AST route emits the whole assignment. The result stays on the stack while the outputs are stored, and neither kind of temporary gets storage. It handles one call. |

The order the AST route runs things in is the program's meaning (the routes
must agree). Above, that order is: `a`, `F`, the store to `x`, `b`, `G`, the
store to `y`, `+`, the store to `r`.

## Decisions for review

### 1. The lowered form: each call hoisted ahead of the assignment

An assignment whose value has calls with outputs lowers to:

1. for each such call, in the order the value evaluates them: the call's
   *part*, which is the assignment of the call to a temporary of its result's
   type followed by the assignments of its outputs, as REQ-LOW-lowering-172's
   first two parts are today;
2. then the assignment itself, whose value reads each call's result from that
   call's temporary.

Every statement carries the assignment's span, as today. A call nested in
another call's arguments gets its own part, placed before the part of the call
that reads it:

```text
r := F(x := 3, n => n1) + F(x := 4, n => n2);

assign t1 (call F (const 3) (ref tn1))
assign n1 (read tn1)
assign t2 (call F (const 4) (ref tn2))
assign n2 (read tn2)
assign r (add (read t1) (read t2))
```

Today's three-part form is the one-call case of this form, so
REQ-LOW-lowering-172 widens rather than gaining a sibling.

**Alternative: a sequencing expression** (`Expr::Seq { stmts, value }`). This
would keep each call where it sits in the expression. But it adds an
expression that contains statements, which every backend would have to
implement, and it needs a value class for the result. Hoisting needs nothing
new in the IR. Not recommended.

### 2. When hoisting keeps the order

Hoisting moves a call, its outputs and its arguments ahead of everything else
in the value. That keeps the order the AST route runs things in exactly when
nothing else in the value runs before the call. So lowering checks one
property on the statements it built. The property is stated on the lowered
form, so lowering and the bridge check the same thing (decision 3):

- **Leading reads.** An expression's *leading reads* are the reads of result
  temporaries it evaluates before any other node finishes:
  - a read of a result temporary is its own leading read;
  - an operation's leading reads are those of its first operand. If that
    operand is just such a read, they continue into the next operand, and so
    on. This covers `Binary`, `Compare` and the value arguments of `Call`. An
    operation of one operand (`Unary`, `Convert`, `ConvertChecked`,
    `Truncate`) has the leading reads of its operand;
  - a `ShortCircuit` has the leading reads of its left operand only;
  - a `Ref` to a whole variable evaluates nothing and is skipped. Any other
    `Ref`, a string argument, a constant, `Null`, `RefTo` or a read of any
    other place ends the run.
- **The rule (new REQ-LOW-lowering-174).** Each result temporary is read
  exactly once, among the leading reads of a later part's call or of the
  assignment's value. The temporaries a part reads are the most recent ones
  not yet read, in the order they were made. Any assignment that breaks the
  rule is not lowered and stays on the AST route.

Why this is enough: the parts run in the order lowering met the calls, which
is the order the value evaluates them. A part's arguments run just before its
call, as they do in the source. The rule leaves nothing else that the source
would have evaluated between two calls, or before the first one.

| Statement | Lowered | Why |
|---|---|---|
| `r := F(x := 3, n => n1) + F(x := 4, n => n2)` | yes | the target statement |
| `r := F(x := 1, n => n1) + 1` | yes | `1` runs after the call in both |
| `r := -F(x := 1, n => n1)` | yes | |
| `r := G(F(x := 1, n => n1))`, `G` with no outputs | yes | |
| `r := F(x := F(x := 1, n => a), n => b)` | yes | nested: the inner part first |
| `r := k.M(x := 1, mn => a) + k.M(x := 2, mn => b)` | yes | a method, as a function |
| `r := a + F(x := 1, n => a)` | no | `a` is read before the call that writes it |
| `r := g + F(x := 1, n => n1)`, `F` writes the global `g` | no | the same: the rule does not look into the callee |
| `r := 1 + F(x := 1, n => n1)` | no | conservative: the constant runs first, and the bridge could not emit it in the AST route's order (decision 3) |
| `r := F(..) * 2 + F(..)` | no | `*` finishes before the second call |
| `r := F(..) / F(..) + F(..)` | no | the division could trap before the third call runs |
| `r := INT_TO_DINT(F(..)) + F(..)`, or a conversion the analyzer recorded around the first call | no | the conversion finishes before the second call |
| `r := c AND_THEN F(..) > 0`, `r := F(..) OR_ELSE F(..)` | no | a call that runs only on a condition is never hoisted |
| `r := a[F(x := 1, n => i)]` | no | conservative: a subscript is not an operand the rule follows |
| `IF`, `ELSIF`, `WHILE`, `REPEAT`, `CASE` and `FOR` conditions, selectors and bounds | no | only an assignment's value hoists in this change |
| `x.3 := F(x := 1, n => x) > 0` | no | `AssignBits` reads its place before the value on the AST route; only a scalar `Assign` hoists |
| `s := CONCAT(..)` with a call with outputs inside | no | `AssignStr`; only a scalar `Assign` hoists |
| `a[F(x := 1, n => i)] := 1` | no | the target's subscripts are lowered outside the hoisting context, and the AST route evaluates them after the value |
| `fb(IN := F(x := 1, n => a))` | no | a function block call's inputs are not a hoisting context (function block call binding is other work) |

An `IF`'s first condition and a `CASE` selector run first and once, so a later
change could hoist from them too. They are not part of this one.

### 3. The bridge's emission

The bridge emits each part as it emits REQ-LOW-lowering-172's first two parts
today: the inputs, `CALL` or `METHOD_CALL` (dropping the instance from beneath
the result once the outputs are stored), then each output's store, read from
the callee's own output variable. The result stays on the stack. Then it emits
the assignment, as an ordinary `Assign`, except that a read of a result
temporary on the stack emits nothing. The leading-reads rule makes that exact:
each such read is the next value the stack already holds, in the order the
expression would have pushed it. Neither kind of temporary gets storage
(new REQ-LOW-codegen-175).

For the target statement this is the AST route's sequence: `3`, `CALL F`,
store `n1`, `4`, `CALL F`, store `n2`, `ADD`, store `r`.

The bridge checks the rule itself in `can_emit`, with the same function in
`ironplc-ir` that lowering uses (no duplicated walk). It adds one limit of its
own: a method's part may have no leading reads, because the AST route pushes
the receiver's instance before the inputs. Anything it does not emit falls
back to the AST route, as today.

**Alternative: emit each call where its result is read.** The bridge would
walk the assignment's value and emit a part at the read of its temporary.
That matches the AST route's bytecode for more shapes (`1 + F(..)`). But the
bridge would then emit statements out of the order the lowered program lists
them (REQ-LOW-codegen-120), which is a decision lowering made, not the
backend. Not recommended.

## Architecture

- **Lowering.** A *hoisting context* replaces `call_result` (`lib.rs:129`).
  The assignment statement opens it around the lowering of its value, on
  `store_at`'s scalar path only. While it is open, `user_call` lowers a call
  with outputs where it meets it: the arguments, a temporary for the result,
  the part appended to the context, and as the value a read of the temporary.
  A call is lowered at most once (keyed by its address, as `call_result` is
  today). After the value is lowered, the statement is the context's parts,
  then the assignment, if the rule holds.
- **IR.** `ironplc-ir` gets the leading-reads walk and the rule as one
  function over the parts' values and the assignment's value. It is pure and
  target-neutral, next to `Expr`. Lowering calls it before returning the
  statements, and the bridge calls it in `can_emit`.
- **Bridge.** `OutputCall` (`fn_call.rs:44`) holds a list of parts ahead of
  the store instead of one. The bridge's emission state holds the result
  temporaries on the stack, and `emit_expr` emits nothing for a read of one.

## Prefactoring

Two prefactors, each behaviour-preserving with every existing test unchanged:

1. **Lower a call with outputs where lowering meets it (lowering).** Replace
   `call_result` with the hoisting context. Today `assignment_of_call` lowers
   the call, then the outputs, then the assignment with `call_result` set.
   Instead, the assignment opens the context and `user_call` makes the part
   when it meets the call. The context opens only where `call_with_outputs`
   finds the call as the whole value, so the same statements lower to the same
   text. Without this, the core change would add a second way to lower a call
   with outputs beside `assignment_of_call`.
2. **Emit a list of call parts ahead of the store (bridge).** `OutputCall`'s
   `Form::Assignment` holds a list of parts; `output_call` still accepts
   exactly one. The store's value is emitted by `emit_expr` with the result
   temporary marked as on the stack, instead of `emit_around(stored, ..)`. Both
   emit the same bytecode for operations of one operand around the read. This
   prepares the general walk without changing what is accepted.

## Design doc reference

`specs/design/lowered-program.md`, §3.8 "Callees, arguments and intrinsics".
The core change rewrites, in the same PR:

- the paragraphs before REQ-LOW-lowering-172: the lowered form and the rule,
  replacing "a call of a function with outputs anywhere else ... stays on the
  route that reads the AST";
- **REQ-LOW-lowering-172**, widened: an assignment whose value has calls of
  functions or methods with outputs lowers to each call's part, in evaluation
  order, then the assignment, which reads each result from its temporary, all
  with the assignment's span;
- **REQ-LOW-lowering-174** (new): the leading-reads rule; an assignment that
  breaks it is not lowered;
- the bridge paragraph after REQ-LOW-lowering-172, and **REQ-LOW-codegen-175**
  (new): while both routes exist, the bytecode backend emits the parts and
  the assignment as the AST route emits the assignment, each result on the
  stack until it is read, with no storage for any temporary;
- the method section's "Not lowered yet" bullet on calls with outputs.

174 and 175 are the next free numbers in the document (the highest is
REQ-LOW-ir-173). The module docs of `lowering/src/function.rs` and
`codegen/src/lowered/fn_call.rs` change with the code.

## File map

| File | Prefactor 1 | Prefactor 2 | Core |
|---|---|---|---|
| `lowering/src/lib.rs` | hoisting context replaces `call_result` | | |
| `lowering/src/hoist.rs` (new; keeps `function.rs` under 1000 lines) | the context, making a part | | apply the rule |
| `lowering/src/function.rs` | `user_call` and `assignment_of_call` use the context | | drop the whole-value restriction |
| `lowering/src/statement.rs` | open the context for an assignment | | |
| `lowering/src/function_tests.rs` | | | the IR of each "yes" row, `unsupported` for the "no" rows |
| `ir/src/lowered/hoisted.rs` (new) | | | leading reads and the rule, with unit tests |
| `codegen/src/lowered/fn_call.rs` | | a list of parts | accept several parts; check the rule; the method limit |
| `codegen/src/lowered/expr.rs` | | skip a read of a result on the stack | |
| `codegen/tests/it/compile_route_lowered_function_output.rs` | | | same code as the AST route, values |
| `specs/design/lowered-program.md` | | | as above |

## Tasks

### Prefactor 1: lower a call with outputs where lowering meets it

- [ ] Hoisting context in `Lowering`, replacing `call_result`.
- [ ] The assignment opens it around its scalar value, only where
      `call_with_outputs` matches; `user_call` makes the part.
- [ ] `assignment_of_call` folds into the assignment path.
- [ ] Every lowering test passes unchanged; `cd compiler && just`.

### Prefactor 2: emit a list of call parts ahead of the store

- [ ] `OutputCall` holds a list of parts; `output_call` accepts one.
- [ ] Mark the result on the stack; `emit_expr` skips its read; the store's
      value goes through `emit_expr`.
- [ ] Every route test passes unchanged, the same-code ones included;
      `cd compiler && just`.

### Core: hoist calls with outputs inside an assignment's value

- [ ] `ironplc-ir`: leading reads and the rule, with unit tests for each kind
      of expression, a short circuit, a skipped `Ref`, a read twice, and reads
      out of order.
- [ ] Lowering: drop the whole-value restriction; check the rule; tests for
      every row of decision 2.
- [ ] Bridge: accept several parts; check the rule; refuse leading reads into
      a method's part; route tests that the bytecode and constants are the
      same as the AST route's for the target statement, a nested call, `F(..) + 1`
      and two method calls, and values for each.
- [ ] Update the design text and requirements; conformance tests for
      REQ-LOW-lowering-172, -174 and REQ-LOW-codegen-175.
- [ ] `cd compiler && just`, `cd specs && just`; `just lowered-count` rises
      by 1.

### Cleanup

- [ ] Close this plan PR unmerged.
