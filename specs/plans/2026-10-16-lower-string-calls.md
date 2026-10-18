# Plan: Lower Calls with String Inputs and String Results

## Goal

Lower two kinds of call so they no longer compile from the AST:

- calls of user-defined functions and methods that take a `STRING` or
  `WSTRING` input;
- calls of user-defined functions that return a `STRING` or `WSTRING`.

Behaviour does not change. Each call compiles to the same instructions and
constants as the AST route, and adds no storage.

On main at 62dcab7, 47 top-level statements of the codegen end-to-end programs
fall back because of one of these calls. Logging the fallback path during one
run of the suite gives this split:

| Kind | Statements | Where |
|---|---|---|
| String input, scalar result | 15 | `MY_LEN` and `CHECK_LEN` (5), `first_three` and `overwrite` (3), `same_len` and `wide_len` (3), `FOO` (2), `f.Len2` (2) |
| String result | 32 | `LREAL_TO_FMTSTR` (24), `twice` (2), `decorate` (2), `my_func` and `MY_FUNC` (2), `LEN(doubled(c128))`, `CONCAT(mk(w), "cd")` |

Four more statements fall back because a function returns a structure. They
are out of scope.

Tracking issue: #2321.

## Architecture

### What the lowered program looks like

The design already fixes the shape, in §3.8 "Callees, arguments and
intrinsics".

**A string input** is an `Arg::Str` for a `Param::Str(encoding)` parameter
(REQ-LOW-lowering-072). The callee's parameter is a variable of the
parameter's string type, held by value, as its body already lowers it.

**A string result** is returned through a result parameter
(REQ-LOW-lowering-079):

- The result parameter is a non-null reference to the result's type. It comes
  first among the parameters (a method would put its instance parameter
  before it).
- Inside the function, every access to the result goes through a `Deref`, as
  for a `VAR_IN_OUT`. `twice := CONCAT(text, text)` becomes
  `assign-str v1^ (...)`.
- At the call site, the call becomes a `Call` statement that passes a `Ref` to
  a new temporary of the result's type. An `AssignStr`, or the string
  operation that uses the value, then reads the temporary.

So `n := LEN(doubled(c128));` lowers to:

```
call pou1 (ref v2) (read string[128] v0^.f0)
assign v0^.f1 (truncate 16 (call len_string i32s (read string[300] v2)))
```

The ids here only show the shape. And `nested := twice(twice('x'));` lowers to:

```
call pou1 (ref v3) (literal string[1] "x")
call pou1 (ref v4) (read string[20] v3)
assign-str v0^.f1 (read string[20] v4)
```

The moved calls appear in the order the calls run, an argument's call before
the call it is an argument of. Each moved call has the span of the statement
it was moved out of (REQ-LOW-lowering-063). A `StrExpr` still never calls a
user function (REQ-LOW-lowering-052).

### IR

- **String parameters.** `FunctionSignature::params` gives
  `Param::Str(encoding)` for a parameter of a string type.
  `ProgramBuilder::declare_function` and `declare_method` accept a string
  parameter that the POU owns.
- **Result parameter.** `FunctionSignature` records whether the function
  returns through a result parameter, its first parameter.
  `declare_function` takes the result either as a scalar result variable or as
  that parameter. It refuses a result parameter that is not a non-null
  reference to a string.
- **No value.** A function that returns through a parameter has no value.
  `ProgramBuilder::call` refuses it as a P9998, and `call_stmt` accepts it.
  `FunctionSignature::result` stays the scalar result variable.

### Lowering

**String inputs** (`function.rs`). `callee_pou` accepts a `VAR_INPUT` of a
string type for a function or a method. `call_args` already lowers its
argument through `Lowering::arg`. This covers every shape a call already
lowers in:

- a call that is a value;
- a call with outputs that is the whole value of an assignment
  (REQ-LOW-lowering-172);
- a method call statement.

A `VAR_IN_OUT` of a string type stays unsupported, because the AST route
reports it as P9999 (`compile_reference_arg`).

**The result parameter in the body** (`variables.rs`). In a function whose
result is a string, `named_place` makes the result variable (the analyzer's
`SymbolKind::ResultVariable`) a non-null reference, reached through `^`, as it
does for a `VAR_IN_OUT`. The type of the variable is decided from the
function's declaration, so a body lowered before any call of the function
gets the same variable. A method's result stays a scalar: the AST route
reports a method that returns a string as P9999 (`compile_method.rs`), so its
calls stay declined.

**Moving a call out of an expression** (new `string_result.rs`):

- **The argument for the result parameter.** `call_args` passes a `Ref` to a
  new temporary of the result's type for the result parameter, labelled
  `f()`, as the temporaries of outputs are.
- **The call becomes a statement.** `str_expr` of a call of a function with a
  string result lowers the call to a `Call` statement. It appends the
  statement to the calls moved out of the statement being lowered. The
  expression's value is a `Read` of the temporary.
- **Placing the moved calls.** `Lowering::statement` keeps a list of moved
  calls for each statement. It saves the enclosing statement's list and
  restores it afterwards, because statements nest. Once the statement has
  lowered, its moved calls go before it, after the order check below.
- **Not moved yet:**
  - a call with outputs inside an expression (out of scope);
  - a method call that returns a string (declined above).

**A call with outputs whose result is a string**, as the whole value of an
assignment (`assignment_of_call`), lowers as REQ-LOW-lowering-172 orders it:

1. the `Call` statement, with the result's temporary and an output temporary
   for each output;
2. the assignments of the outputs;
3. the `AssignStr` of the result's temporary.

`str_expr` reads the result's temporary through `call_result`, as `user_call`
does for a scalar result.

### Evaluation order

Moving a call into a statement before the one it is in runs the call earlier
than the source does. Anything the source evaluates before the call is then
evaluated after it. That changes the program when the earlier part can see
what the call does, or the call can see what the earlier part does. A
function can:

- write a global directly: `FOO` in `end_to_end_struct.rs` reads `DATA` with
  no `VAR_EXTERNAL`;
- write a variable passed to its `VAR_IN_OUT`;
- write through a reference.

So any read of a variable before the call can see the call's writes, not only
a read of a global. A part that traps before the call would, once the call
moved, trap after the callee's writes. Those writes stay after a trap
(REQ-LOW-codegen-038).

**The rule** (new `call_order.rs`, run when a statement's moved calls are
placed). Lowering walks the statement in the order it evaluates, and walks
each moved call's arguments the same way. Before any read of a moved call's
temporary, the walk may meet only:

- a constant or a string literal;
- a `Ref` to a place with no subscripts;
- a read of the temporary of a call moved earlier.

Anything else makes the statement unsupported. That covers another call
evaluated before it, a global the callee may write, any other variable read,
and anything that can trap.

The walk follows the order the AST route evaluates in:

- the value of an `Assign` or `AssignStr`, then the subscripts of its target;
- the condition of an `If`;
- the `from` of a `For`;
- the arguments of a `Call` statement, in order;
- the operands of an operation, in order;
- the value of each part of a multi-part statement, part by part.

**Never moved out of.** A temporary read anywhere else makes the statement
unsupported. This covers:

- **an `ELSIF` condition:** the call would run even when an earlier branch is
  taken;
- **a `WHILE` or `REPEAT` condition, the `TO` of a `FOR`, or any nested
  statement list:** a condition or bound is evaluated on each iteration, and
  a nested statement may not run at all;
- **the selector of a `CASE`:** the AST route compiles the selector once per
  label it tests, so the call would run a different number of times on each
  route (REQ-LOW-codegen-124);
- **a bit or partial write (`AssignBits`):** it reads its place before its
  value;
- **the right operand of `AND_THEN` or `OR_ELSE`:** it may not run.

All 47 statements above meet the rule. What still falls back:

- a moved call after a read in the same statement: `n := k + LEN(f(x))`,
  `CONCAT(s, f(x))` and `s = f(x)`;
- a moved call after another operation: `LEN(f(a)) + LEN(f(b))`;
- each position under [Never moved out of](#evaluation-order).

The core change PR for string results reports how many of each remain.

### Bytecode bridge

**String inputs.** The bridge copies a string argument into the parameter's
storage as `compile_string_arg` does:

1. `STR_INIT` of the parameter;
2. the argument, at the offset of a slot that holds it (`str_arg_offset`,
   which mirrors `resolve_string_arg`);
3. `STR_LOAD_VAR` from that slot and `STR_STORE_VAR` into the parameter, cut
   to its capacity (REQ-LOW-codegen-056);
4. a dummy 0 for the slot `CALL` or `METHOD_CALL` pops.

`user_function` and `method_site` accept `ParamPassing::String`, and the
argument loop that prefactor 2 shares emits it. A call with outputs
(`fn_call.rs`) gets it through the same loop.

**String results: emitted where the temporary is read.** The bridge does not
emit a moved `Call` where it stands. It emits the call where its temporary is
read, which is where the AST route compiles the call:

- the arguments;
- `STR_INIT` of the function's result (`return_string_info`);
- `CALL`, which leaves the result in a temporary buffer (ADR-0052).

The temporary has no storage. Its `Read` is that buffer, which the operation
reading it consumes, as it consumes the buffer of the call on the AST route:

- an `AssignStr` stores it;
- `str_arg_offset` copies it into a temporary slot for `LEN` or `CONCAT`;
- the copy of a string argument copies it into the parameter.

A shape like `CONCAT('ab', mk(w))`, where the AST route pushes an operand
before the call, could also be left unsupported instead. **This plan emits
every moved call where its temporary is read**, for three reasons:

- **It is the only emission that matches the AST route in every shape, not
  just that one.** In `twice(twice('x'))` the AST route initializes the outer
  call's parameter before it runs the inner call. In `CONCAT('ab', mk(w))` it
  copies `'ab'` into a temporary slot before the call. Emitting at the read
  reproduces both instruction for instruction, with the same temporary slots
  and so the same data region.
- **It is equivalent to running the call first.** The order rule allows
  nothing before a moved call that could see it.
- **It is uniform.** One emission covers every shape, rather than one for a
  whole assignment and another for an operand.

**Finding the call at the read.** Before checking a lowered statement,
`compile_by_route` collects the moved calls of every statement it lowered to,
nested ones included, into a table keyed by temporary. The table lives on
`CompileContext`, because every emitter of the bridge already takes the
context and the read can sit deep inside an expression. Threading a new
parameter through every emitter would change every signature of the bridge
for one table. The table is used like this:

- **`can_emit`** accepts a moved call when its arguments can be emitted, and
  when each temporary is read exactly once by the statements lowered with
  it. A `Read` of a temporary in the table counts as reachable.
- **Emission.** `sequence` emits nothing for a moved call. `emit_str_expr`
  takes the call out of the table and emits it where the temporary is read.
- **After the statement.** The table must be empty once the statement is
  emitted. A call left in it is an internal error, since the call would never
  run.

**A call with outputs and a string result** is emitted as a group by
`fn_call.rs`, as the AST route compiles it:

1. the arguments;
2. `STR_INIT` of the result and `CALL`;
3. the stores of the outputs;
4. the store of the buffer the call left on the stack.

Such a call never enters the table.

**The function's body.** `result^` is found where the AST route keeps the
result. `by_reference` (`lowered/place.rs`) maps a non-null reference of the
current body to its name. The AST route registers the result under the
function's name in `string_vars`, at `return_string_info.data_offset`.
`string_storage` finds `o^` for an output the same way. No change is expected
there; a test checks the body still compiles to the same code.

**Storage and pool.** Every instruction is the AST route's, so the temporary
buffers, the pool size and the data region are the AST route's too
(`compile_temp_bufs.rs`, `assert_routes_agree`).

## Prefactoring

Each prefactor is behaviour preserving and its own PR.

1. **One function copies a string argument into its parameter** (codegen).
   - `compile_string_arg` (`compile_user_call.rs`) splits into:
     - the copy: `STR_INIT` of the parameter, a source that yields the
       offset of a slot holding the argument, the load and store, and the
       dummy 0;
     - the AST route's source, `resolve_string_arg`.
   - The AST route's function and method calls use the copy.
   - The bridge will reuse it with `str_arg_offset` as the source, rather than
     repeating the sequence.
   - The bytecode is unchanged.
2. **The bridge passes the arguments of a function and of a method in one
   place** (codegen).
   - `passes` (functions: `Value` and `Reference`) and `MethodSite::takes`
     (methods: `Value` only) become one check of the arguments against the
     callee's `ParamPassing`. A method still refuses a `VAR_IN_OUT`, as
     `compile_method_call` does.
   - `emit_user_call` and `emit_method_call` push their inputs through one
     loop.
   - The string arm is then added once, in core 1, rather than in each of
     these four places.
   - Strings stay refused, so nothing new lowers. The lowered-route count is
     unchanged.

No lowering prefactor is needed:

- the string-input change to `callee_pou` is a few lines;
- moving calls out of statements is new behaviour, in a new module.

## Design Doc Reference

`specs/design/lowered-program.md`:

- §3.6 "String expressions": REQ-LOW-lowering-052.
- §3.8 "Callees, arguments and intrinsics": REQ-LOW-lowering-071, 072, 079 and
  092; REQ-LOW-lowering-172; the passages on what is not lowered for
  functions and methods; and the "While both routes exist" paragraphs.
- §3.10 "Meaning of operations": REQ-LOW-codegen-038 and 056.
- §7 "Backend Contract": REQ-LOW-codegen-124.
- "Delivery Constraints".

Each core PR updates the design (see the tasks):

- **New requirements:**
  - one for moving a call out of an expression;
  - one for the order rule and the positions never moved out of;
  - one for the IR's result parameter.
- **Numbering:** the IDs are the next free ones when each PR is written (174
  to 176 today).
- **Updated passages:** the ones near REQ-LOW-lowering-172, and the methods
  list that says these calls stay on the AST route.
- **Bytecode emission:** a "While both routes exist" paragraph for each.
- **Testing:** the design's requirements are cited in test doc comments, as
  lowering's tests already cite them.

## File Map

| File | Change |
|---|---|
| `compiler/codegen/src/compile_user_call.rs` | The copy of a string argument, split from `compile_string_arg` (P1) |
| `compiler/codegen/src/compile_method.rs` | Uses the shared copy (P1) |
| `compiler/codegen/src/lowered/call_args.rs` | New: check and push a user function's or method's arguments (P2); the string arm (C1); skip the result parameter's `Ref` (C2) |
| `compiler/codegen/src/lowered/mod.rs` | `user_function` accepts strings (C1) and string results (C2); `can_emit` of a moved call; the table filled in `compile_by_route` (C2) |
| `compiler/codegen/src/lowered/method.rs` | `takes` and the input loop use `call_args.rs` (P2); string inputs (C1) |
| `compiler/codegen/src/lowered/expr.rs` | `emit_user_call` uses `call_args.rs` (P2) |
| `compiler/codegen/src/lowered/string.rs` | A `Read` of a moved call's temporary emits the call (C2) |
| `compiler/codegen/src/lowered/string_result.rs` | New: the table of moved calls (C2) |
| `compiler/codegen/src/lowered/stmt.rs` | A moved call emits nothing where it stands (C2) |
| `compiler/codegen/src/lowered/fn_call.rs` | A call with outputs and a string result (C2) |
| `compiler/codegen/src/compile.rs` | The table on `CompileContext` (C2) |
| `compiler/ir/src/lowered/variables.rs` | `Param::Str` (C1); the result parameter in `FunctionSignature` (C2) |
| `compiler/ir/src/lowered/program.rs` | `declare_*` accept a string parameter (C1) and a result parameter (C2) |
| `compiler/ir/src/lowered/expr.rs` | `check_user_call`: no value through a result parameter (C2) |
| `compiler/ir/src/lowered/tests/calls.rs` | IR tests (C1, C2) |
| `compiler/lowering/src/function.rs` | String inputs (C1); the result parameter's argument, and outputs with a string result (C2) |
| `compiler/lowering/src/variables.rs` | The result variable through `^` (C2) |
| `compiler/lowering/src/string.rs` | `str_expr` of a call with a string result (C2) |
| `compiler/lowering/src/string_result.rs` | New: moving a call out of an expression (C2) |
| `compiler/lowering/src/call_order.rs` | New: the order rule (C2) |
| `compiler/lowering/src/statement.rs`, `lib.rs` | The moved calls of each statement (C2) |
| `compiler/lowering/src/function_tests.rs`, `method_tests.rs` | String inputs (C1) |
| `compiler/lowering/src/string_result_tests.rs` | New: string results and the order rule (C2) |
| `compiler/codegen/tests/it/compile_route_lowered_string_call.rs` | New: route tests (C1, C2) |
| `compiler/codegen/tests/it/main.rs` | Registers it |
| `specs/design/lowered-program.md` | §3.6 and §3.8 (C1, C2) |

## Tasks

### Prefactor 1: one function copies a string argument into its parameter

- [ ] Split the copy out of `compile_string_arg`, taking its source as a
      closure that returns the slot's offset
- [ ] `compile_user_function_call` and `compile_method_call` use it through
      `compile_string_arg`
- [ ] Every existing codegen test passes unchanged, the bytecode tests
      included
- [ ] `cd compiler && just`

### Prefactor 2: the bridge passes the arguments of a function and of a method in one place

- [ ] `lowered/call_args.rs`:
  - one check of a call's arguments against the callee's `ParamPassing`
    (`Value` and `Reference`, and `Value` only for a method);
  - one loop that pushes them.
- [ ] `passes`, `MethodSite::takes`, `emit_user_call` and `emit_method_call`
      use it
- [ ] `cd compiler && just`; the lowered-route count is unchanged

### Core 1: string inputs

- [ ] IR: `FunctionSignature::params` gives `Param::Str(encoding)` for a
      string parameter, and `declare_function` and `declare_method` accept
      one the POU owns. Tests:
  - a function with a string and a `WSTRING` input takes `Str` of each
    encoding;
  - a `Call` passing an `Arg::Value` for one is refused.
- [ ] Lowering: `callee_pou` accepts a `VAR_INPUT` of a string type for
      functions and methods; a `VAR_IN_OUT` of a string type stays
      unsupported. Update the module comment of `function.rs`.
- [ ] Bridge: `call_args.rs` passes `Arg::Str` for `ParamPassing::String` with
      the shared copy and `str_arg_offset`; `user_function` and `method_site`
      accept it
- [ ] Design: in §3.8,
  - drop "a parameter other than an output … that is a string" from what
    stays on the AST route;
  - narrow the methods list to an in-out or result that is a string;
  - say how the bytecode backend copies a string argument.
- [ ] Lowering tests, against the IR text:
  - a string input;
  - a `WSTRING` input;
  - a string input of a call with outputs;
  - a method with a string input, as a value and as a statement;
  - a `VAR_IN_OUT` of a string type is unsupported.
- [ ] Route tests (`compile_route_lowered_string_call.rs`), every statement
      required to lower, the same code as the AST route, and values checked
      against the AST route:
  - a literal, a variable and an element of a string array as the argument;
  - an argument longer than the parameter's capacity, cut;
  - `STRING` and `WSTRING` parameters of one function;
  - a method with a string input;
  - a call with outputs and a string input;
  - the callee's write to its parameter does not reach the caller.
- [ ] `cd compiler && just`; `just lowered-count` rises by about 15

### Core 2: string results

- [ ] IR:
  - `FunctionSignature` records a result parameter;
  - `declare_function` takes a scalar result variable or a result parameter,
    and refuses a result parameter that is not a non-null reference to a
    string;
  - `ProgramBuilder::call` refuses a function that returns through a
    parameter;
  - a test for each.
- [ ] Lowering, the body: the result variable of a function whose result is a
      string is a non-null reference reached through `^`
- [ ] Lowering, the call:
  - `call_args` passes a `Ref` to a new temporary for the result parameter;
  - `str_expr` moves a call with a string result into a `Call` statement and
    reads its temporary;
  - `statement` collects each statement's moved calls and places them before
    it.
- [ ] Lowering, a call with outputs and a string result as the whole value of
      an assignment: the call, its outputs, then the `AssignStr` of the result
- [ ] Lowering, `call_order.rs`: the order rule and the positions never moved
      out of; a statement that breaks either is unsupported
- [ ] Bridge:
  - the table of moved calls on `CompileContext`, filled in
    `compile_by_route`;
  - `can_emit` of a moved call and of a read of its temporary;
  - `sequence` skips a moved call;
  - `emit_str_expr` emits the call at the read;
  - an internal error for a call left in the table.
- [ ] Bridge: `user_function` accepts a string result, and `call_args.rs`
      skips the result parameter's `Ref`, for which the call emits `STR_INIT`
      of the result before `CALL`
- [ ] Bridge, `fn_call.rs`: a `Call` of a function with a string result, its
      outputs and the `AssignStr` of its result, emitted as one group
- [ ] Design:
  - §3.6 and §3.8 describe moving a call out, with the new requirements for
    the move, the order rule and the IR's result parameter;
  - the bytecode emission at the read gets its own "While both routes exist"
    paragraph;
  - drop "or a result, that is a string" near REQ-LOW-lowering-172;
  - the methods list keeps a string result, now with its reason: the AST
    route reports it as P9999.
- [ ] Lowering tests, against the IR text:
  - a string result assigned whole;
  - a string result as an operand of `LEN` and of `CONCAT`;
  - a string result as the argument of a call (`twice(twice('x'))`);
  - two calls of one function in one statement;
  - a string result in an `IF` condition and in a `FOR`'s `from`;
  - a body that writes and reads its result through `result^`;
  - a call with outputs and a string result;
  - each unsupported shape:
    - after a read (`k + LEN(f(x))`);
    - after a call (`g(1) + LEN(f(x))`);
    - in an `ELSIF`, `WHILE` or `REPEAT` condition;
    - in a `FOR`'s `TO`;
    - in a `CASE` selector;
    - in a bit write;
    - on the right of `AND_THEN` and `OR_ELSE`;
  - a method with a string result stays declined.
- [ ] Route tests, every statement required to lower, the same code as the
      AST route, and values checked against the AST route:
  - `s := f(s)`;
  - `twice(twice('x'))`;
  - `CONCAT(f(a), f(b))`;
  - `CONCAT('ab', mk(w))` with a `WSTRING` result;
  - `LEN(doubled(c))` with a result longer than the default capacity;
  - a string result in an `IF` condition;
  - a call in a `FOR` of 200 iterations, with the same `num_temp_bufs` and
    `max_temp_buf_bytes` as the AST route;
  - a call with outputs and a string result;
  - the body of a function that returns a string, compiled to the same code.
- [ ] Route tests: an unsupported shape (an `ELSIF` condition) compiles by
      the AST route under `PreferLowered`, and is a P9999 under
      `RequireLowered`
- [ ] `cd compiler && just`; `just lowered-count` rises by about 32; every
      statement of `LREAL_TO_FMTSTR`'s body still takes the lowered route.
      The PR lists the shapes that still fall back.

### Cleanup

- [ ] Open an issue for the shapes that still fall back (a moved call after a
      read or another operation), with their counts, if any test or library
      has one
- [ ] Close this plan PR unmerged, and the tracking issue when both core PRs
      land

## Out of Scope

- **A function result or input that is a structure or an array.** It needs
  `Copy` (#2316), and follows once that merges.
- **Function block calls and standard function blocks**, which other work
  owns.
- **A call of a method the receiver's block inherits.**
- **A call with outputs inside an expression.**
- **`EN` and `ENO`.**
- **A method that returns a string**, which is P9999 on the AST route.
- **A `VAR_IN_OUT` of a string type**, which is P9999 on the AST route.
- **`lowered_floor`** in `compiler/justfile`, which stays as it is.
