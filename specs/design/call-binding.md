# Design: Call Binding

status: implemented
date: 2026-10-12

## Overview

A call is a language decision in four parts: what it reaches (which
function, which function block, which method body), how each argument binds
to a parameter, what an omitted input does, and the order the arguments are
evaluated in. The analyzer decides all four, and code generation, and the
lowering that replaces it ([Lowered Program](lowered-program.md)), lower its
answer rather than deciding again. This follows
[ADR-0056](../adrs/0056-analyzer-records-implicit-conversions-in-the-ast.md)
and [ADR-0058](../adrs/0058-the-analyzer-decides-and-lowering-translates.md):
a decision that can make a program invalid is the analyzer's.

A function is found by its name in the function environment, which no stage
after the analyzer can match differently: a user-defined function cannot
share a standard function's name. So [What a call reaches](#what-a-call-reaches)
is about function blocks and methods, and the rest of this design is about
every kind of call.

## What a call reaches

### Function blocks

A function block instance is the type its declaration resolved to
(`VarDecl::type_id`). A standard function block is a property of that type,
not of its spelling: the type environment records which standard block each
of its standard types is (`TypeAttributes::standard_block`), and a type
without that record is a user-defined block, whatever its name. Code
generation asks the type which VM block runs it.

**REQ-CB-analyzer-001** Every standard function block type in the type
environment records which standard block it is, and a counter records the
integer type it counts in.

**REQ-CB-analyzer-002** A user-defined function block named like a standard
one, such as `CTU_INT`, records no standard block.

**REQ-CB-codegen-001** An instance of a user-defined function block named
like a standard one runs the user-defined block's body.

**REQ-CB-codegen-002** An instance of a standard function block runs the VM
block for its recorded identity.

An instance of a block that `EXTENDS` another has the inputs, in-outs and
outputs each block up its chain declares as well as its own, and a call binds
them alike. The type environment lists an instance's fields from the root of
the chain down (`TypeEnvironment::instance_fields`), so normal form writes
the inherited arguments first. Code generation does not yet lay out the
fields a derived block inherits, so it reports such a call as not implemented
(P9999), as it does a read of an inherited field.

**REQ-CB-analyzer-053** A call to a block that `EXTENDS` another binds an argument to an input or output the block inherits as it does one the block declares, and writes the inherited arguments first: with `count` and `seen` declared by the base and `speed` and `twice` by the derived block, `d(twice => r, speed := x, seen => s, count := 3)` binds as `d(count := 3, speed := x, seen => s, twice => r)`, and `d(1, 2)` as `d(count := 1, speed := 2)`.

### Methods

A method call resolves statically
([ADR-0041](../adrs/0041-staged-method-and-interface-dispatch.md), case 1):
the receiver's block's own method of that name, or else the nearest one in its
`EXTENDS` chain. The receiver's block is the instance's declared block, the
enclosing block for `THIS^`, and the block that one `EXTENDS` for `SUPER^`.

The analyzer answers with one walk of the chain,
`callee_resolution::FunctionBlocks::resolve_method`. `MethodOwners` keeps its
answers without borrowing the library, for the recording pass, which consumes
the library as it folds it, and for code generation, which calls a method by
the block that declares it.

**REQ-CB-analyzer-003** A method call on an instance resolves to the method
the instance's block declares, or else to the one the nearest block in its
`EXTENDS` chain declares.

**REQ-CB-analyzer-004** A method the instance's block declares is resolved
before a method of the same name that a block it extends declares.

**REQ-CB-analyzer-005** A call through `THIS^` starts the search at the
enclosing function block, and a call through `SUPER^` at the block that one
`EXTENDS`.

**REQ-CB-analyzer-006** `THIS^` outside a function block, and `SUPER^` in a
block that extends nothing, start the search at no block.

**REQ-CB-analyzer-007** The arguments of a call to an inherited method are
converted to the declared types of that method's parameters.

**REQ-CB-codegen-003** A method a base block declares runs when it is called
on an instance of a derived block, when the base block declares no fields.

**REQ-CB-codegen-004** A method a derived block declares runs in place of a
method of the same name its base declares, and the base's method still runs
on an instance of the base.

**REQ-CB-codegen-005** A call on an instance of a derived block to a method
of a base block that declares fields is reported as not implemented (P9999):
a derived block's layout does not yet hold the fields it inherits.

`rule_method_call_declared` still reports a call through `THIS^` or `SUPER^`
as not implemented, so code generation does not compile one yet.

## How arguments bind

### Normal form

The analyzer records how a call's arguments bind in the call itself, in the
language's own terms: it writes the arguments in **normal form**. A pass,
`xform_bind_call_arguments`, runs before the semantic rules and rewrites each
function, function block and method call whose arguments bind:

- one named input for each `VAR_INPUT` or `VAR_IN_OUT` the call supplies, in
  the order the callee declares them, a positional argument named after the
  input it binds to;
- then one output assignment for each `VAR_OUTPUT` the call reads, in
  declaration order, keeping its negation.

An output assignment also records the value the call stores in its target
(`Output::value`): the output the call leaves, `ExprKind::CallOutput(q)`,
and `NOT` of it for a negated output, written where the output is named. A
call output is not source syntax and appears nowhere else. It is read where
the call leaves its outputs: for a function block, the instance's field, so
it is the value `v := inst.q` after the call would assign; for a function or
a method, the output variable of the callee, which no other call can have
changed before it is read, since a function or method does not recurse.
Lowering reads it from the temporary the call passes for the output
([Lowered Program](lowered-program.md), REQ-LOW-lowering-092). It has the
type the output is declared with, `inst.q`'s for a function block.
The rules check it against the target, and the recording pass converts it
to the target's type, as they do an assigned value's
([Implicit Conversions](implicit-conversions.md)). A negated output that is
not a `BOOL` is P4083, and nothing else is reported for it: the `NOT` is the
analyzer's, not an operation the program wrote.

The callee of a method call is the method its receiver's block declares or
inherits ([Methods](#methods)), so a call to an inherited method is written
in the order the base block's method declares its parameters, and a call
through `THIS^` or `SUPER^` in the order of the method that receiver reaches.

A positional argument of a function binds to its `VAR_INPUT` and
`VAR_IN_OUT` parameters in declaration order, where a function block's or
method's binds to its `VAR_INPUT` parameters alone. An extensible function,
such as `ADD` or `MUX`, takes inputs beyond the ones it declares, named by
counting on from its last declared input: `IN3` after `IN1` and `IN2`.

A parameter is named as its callee declares it, so with the callee's scope it
is the declaration's `DeclRef` ([Variable Binding](variable-binding.md)). A
call whose arguments do not bind keeps them as written, and the problem is
reported: by `rule_function_block_invocation` or `rule_method_call_declared`
for a function block or method call; for a function call, a parameter named
twice or a name that is no parameter by the pass itself, and mixed arguments
or the argument count by `rule_function_call_declared`.
`call_binding::bind` is the one binder: the pass, the rules' diagnostics and
the normal form all come from it.

A pass after the binding pass that builds a call builds it in normal form:
the implicit conversion pass writes `ADD(a, b, c)` as `ADD(IN1 := ADD(IN1 :=
a, IN2 := b), IN2 := c)` when each step computes at its own type.

**REQ-CB-analyzer-020** A function block call whose arguments bind is written
with one named input for each input or `VAR_IN_OUT` it supplies, in the
block's declaration order, followed by its output assignments in declaration
order.

**REQ-CB-analyzer-021** A positional input binds to the `VAR_INPUT` at its
position among the block's `VAR_INPUT` declarations, and a positional call
supplies one for each (P4003).

**REQ-CB-analyzer-022** A named input binds to the `VAR_INPUT` or
`VAR_IN_OUT` of that name, and an output assignment to the `VAR_OUTPUT` of
that name. Any other name is P4002 or P4004.

**REQ-CB-analyzer-023** A call that mixes named and positional inputs is
P4001, and nothing else is reported for it.

**REQ-CB-analyzer-024** A function, function block or method call that names
a parameter twice is P4024.

**REQ-CB-analyzer-025** A call of a standard function block is bound and
checked by the same rules as a call of a user-defined one.

**REQ-CB-analyzer-026** A negated output assignment (`NOT q => v`) whose
output is not a `BOOL` is P4083, and nothing else, of a function block, a
function and a method call alike.

**REQ-CB-analyzer-027** A negated output assignment keeps its negation.

**REQ-CB-analyzer-028** A method call whose arguments bind is written with one
named input for each input or `VAR_IN_OUT` it supplies, in the order the
method it resolves to declares them, inherited or not.

**REQ-CB-analyzer-029** A positional argument of a method call binds to the
`VAR_INPUT` at its position among the method's `VAR_INPUT` declarations, never
to a `VAR_IN_OUT`.

**REQ-CB-analyzer-030** A method call through `THIS^` or `SUPER^` is written in
the order of the method that receiver reaches.

**REQ-CB-analyzer-031** A function call whose arguments bind is written with
one named input for each input or `VAR_IN_OUT` it supplies, in the function's
declaration order.

**REQ-CB-analyzer-032** A positional argument of a function call binds to the
`VAR_INPUT` or `VAR_IN_OUT` at its position among the function's `VAR_INPUT`
and `VAR_IN_OUT` declarations.

**REQ-CB-analyzer-033** A further input of an extensible function is named by
counting on from its last declared input, and binds in number order after the
declared ones; one numbered past the inputs the call supplies is P4023.

**REQ-CB-analyzer-034** A named input of a function call that names no
parameter of the function is P4023.

**REQ-CB-analyzer-035** A call the implicit conversion pass builds is in
normal form.

**REQ-CB-analyzer-036** Each output assignment of a function block call whose
arguments bind records the value the call stores in its target, written
where the output is named: the call output `q`, of the type `inst.q` has, for
`q => v`, and `NOT` of the call output `done` for `NOT done => d`, for a
user-defined and a standard block.

**REQ-CB-analyzer-037** Each output assignment of a function or method call
whose arguments bind records the call output it stores, of the type the
function or method declares the output with: `CallOutput(q)` of an `LINT`
for `q => l` on a function, and `CallOutput(mq)` of a `REAL` for `mq => lr`
on a method.

**REQ-CB-analyzer-038** An output assignment is checked against its target as
the assignment of the value it stores is: a structure output `p` stored in a
target of the identical type, `f(p => q)`, is accepted as `q := f.p` is, and
one stored in a target of another type is P2037.

**REQ-CB-codegen-020** A function block call with positional inputs stores
them in the block's `VAR_INPUT` parameters in declaration order.

**REQ-CB-codegen-021** A positional call of a standard function block stores
its inputs.

**REQ-CB-codegen-022** A negated output assignment stores the inverse of the
output.

**REQ-CB-codegen-023** A method call with positional inputs passes them to the
method's `VAR_INPUT` parameters in declaration order.

**REQ-CB-codegen-024** A method call that binds a `VAR_IN_OUT` is reported as
not implemented (P9999), not passed by value. Lowering passes it by
reference.

**REQ-CB-codegen-025** A function call with named inputs computes what the
positional call with the same inputs in declaration order computes.

**REQ-CB-codegen-026** A positional argument of a call to a user-defined
function that reaches code generation is an internal error (P9998): the
analyzer names every argument of a call it binds.

### Evaluation order

A call evaluates its arguments in the order of its normal form: inputs in the
order the callee declares them, then, after the call, outputs in declaration
order. Functions and methods already evaluate their arguments in parameter
order; a function block call used to evaluate its inputs in the order it
wrote them.

**REQ-CB-codegen-040** A function block call evaluates its inputs in the order
the block declares them, whatever order the call writes them in.

**REQ-CB-codegen-041** A method call evaluates its inputs in the order the
method declares them, whatever order the call writes them in.

**REQ-CB-codegen-042** A function call evaluates its inputs in the order the
function declares them, whatever order the call writes them in.

### Omitted inputs

A function block instance keeps its variables between calls, so an input a
call does not supply keeps the value it had: from the previous call, or its
initial value on the first. The normal form records that by leaving the input
out. A method has no such state: its inputs are fresh on every call, and an
initial value on one is not a default, so a method call supplies every input
and `VAR_IN_OUT` (P4018). A function is the same: its call supplies every
input and `VAR_IN_OUT`, and one that does not has the wrong number of
arguments (P4018).

**REQ-CB-analyzer-050** An input a function block call omits is absent from
its normal form.

**REQ-CB-analyzer-051** An input or `VAR_IN_OUT` a method call omits is P4018,
one for each, and nothing is reported for a call whose arguments do not bind.

**REQ-CB-analyzer-052** A function call that omits an input or `VAR_IN_OUT` is
P4018.

**REQ-CB-codegen-050** An input a function block call omits keeps the value
the previous call gave it.

### Outputs

A call stores each output it assigns in its target after the callee
returns, in the order of its normal form, as the assignment of the value the
analyzer recorded for it (see [Normal form](#normal-form)). A function block
leaves its outputs in its instance's fields, which keep their values between
calls. A function or a method leaves them in its own output variables, in
its region of the variable table, and the caller reads them from there. Like
a local, an output of a function or method starts again from its initial
value on every call (REQ-IV-analyzer-050), so a call that does not assign it
leaves its initial value.

**REQ-CB-codegen-060** A function call stores each output in its target after
the call, converted to the target's type: `r := F(x := 3, q => lr, u => lu)`
with a `REAL` output of 1.5 and a `UDINT` output of 4000000000 stores 1.5 in
an `LREAL` and 4000000000 in an `LINT`, `NOT done => nd` stores the inverse
of a `BOOL` output, and the return value is the function's.

**REQ-CB-codegen-061** A method call stores each output in its target after
the call, in a statement and in an expression: `mr := k.M(x := 5, mq => mlr)`
with a `REAL` output of 2.5 stores 2.5 in an `LREAL`, and `k.M(x := 6, mn =>
a[2])` stores the output in an element of an array.

**REQ-CB-codegen-062** An output of a function or method starts again from its
initial value on every call: two calls of a function that adds its input to
its output store each input alone.
