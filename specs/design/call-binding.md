# Design: Call Binding

status: partially implemented
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

**Status.** [What a call reaches](#what-a-call-reaches) is implemented for
function block types and methods. How arguments bind, the evaluation order and
omitted inputs are not implemented yet; codegen still binds the arguments of
a function block or method call itself.

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

Not implemented yet. The analyzer will record each call's arguments in
normal form: named arguments, one for each parameter the call supplies, in the
callee's parameter declaration order, which is also the order they are
evaluated in. An omitted function block input keeps its value from the
previous call; a function or method may not omit an input.
