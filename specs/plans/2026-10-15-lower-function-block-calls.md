# Plan: Lower Function Block Calls

## Goal

Lower every function block call statement, so that it no longer compiles
from the AST. Function block calls are most of what still does: in the
codegen end-to-end programs, about 1,184 top-level statements are function
block calls, of about 1,400 that compile from the AST. About 557 call
user-defined blocks and about 626 call standard blocks.

Behaviour does not change. A call compiles to the same instructions on both
routes, apart from the one difference named under
[Known difference](#known-difference).

Tracking issue: #2289.

## Architecture

### The analyzer has already decided everything a call needs

Call binding is done (pull requests 2209, 2216, 2217, 2219, 2222, 2277 and
2283):

- **Normal form.** Each call is written with one named input for each input
  it supplies, in the order the block declares them, then its outputs in
  declaration order. That order is the evaluation order.
- **Recorded outputs.** Each output assignment records the value it stores:
  `ExprKind::CallOutput(q)`, wrapped in the conversion to the target's type
  where they differ, and in `NOT` for a negated output.
- **Block identity.** The instance's block, user-defined or standard, is known
  by its type.

So lowering only translates. A call needs no new analyzer work.

### Shape of a lowered call

The design already fixes the shape (REQ-LOW-lowering-069): one source
statement lowers to a list of statements.

```
f(x := a + 1, y := b, q => r, NOT done => nd);
```

lowers to the following, with places written by name where the printer
writes field numbers (`v0^.f2`):

```
assign this^.f.x (add i32s (read i32s this^.a) (const i32s 1))
assign this^.f.y (read i32s this^.b)
fb_call this^.f
assign this^.r (read i32s this^.f.q)
assign this^.nd (bool_not (read bool this^.f.done))
```

- **Inputs** are ordinary stores to the instance's fields, in the order of the
  normal form. An input the call omits has no store, so the field keeps its
  value (REQ-CB-codegen-050).
- **`FbCall`** runs the block's body on the instance. It is the one new IR
  statement, `StmtKind::FbCall { instance: Place, block: PouId }`, as the
  design's section 3.7 lists it. The builder takes `block` from the instance
  type's `TypeKind::FunctionBlock { body }`. A place of any other type is a
  P9998.
- **Outputs** are ordinary stores of the recorded value. While lowering an
  output's value, a `CallOutput(q)` is the read of field `q` of the call's
  instance.
- **Spans.** Every statement lowered from the call has the call's span
  (REQ-LOW-lowering-063), so the line map does not change.

A call whose input or output lowering does not support yet is unsupported as a
whole and stays on the AST route, as any other statement does. Examples are a
string or aggregate field the bridge does not reach, a field a block inherits
through `EXTENDS`, and `EN`/`ENO`.

### Standard function blocks run on the VM for now

A standard block's instance already has a lowered type: its fields are the
ones the analyzer gives it, and its `body` is a POU named after the block. In
this plan that POU has no body in the lowered program. The bytecode backend
emits the same `FB_CALL` it emits today, with the VM's type id for the block,
so the VM's built-in block runs as before.

Generating the standard blocks in lowering, as ADR-0060 decides, is
out of scope here: #2288. Until it lands, only the bytecode backend can run a
standard block. The ADR and the design stay as they are; this is the order of
the work, not a change of decision.

### The bytecode backend

- **Finding the instance.** The bridge finds the instance as `compile_fb_call`
  does, by name through `ctx.fb_instances`, using `storage_name` on the
  instance place.
  - An instance the AST route cannot call is refused by `can_emit` and falls
    back to the same error. Examples are one declared inside a function block
    (#1553), one reached through `VAR_EXTERNAL` (#1858), and an array of
    instances (#2211).
  - So no program that fails on the AST route compiles on the lowered one.
- **Emitting the call.** An `FbCall` emits `FB_LOAD_INSTANCE`, `FB_CALL` and
  `POP`. It records a call-graph edge for a user-defined block, as
  `compile_fb_call` does (`compile_stmt.rs:186`).
- **One instance load per call.** On its own, each input store would emit its
  own `FB_LOAD_INSTANCE` and `POP` (`Place::assign`, `compile_place.rs:375`).
  When a run of `Assign`s to fields of one instance ends in an `FbCall` of that
  instance, the bridge emits the run as `compile_fb_call` does: one
  `FB_LOAD_INSTANCE`, then each value and `FB_STORE_PARAM`, then `FB_CALL` and
  `POP`. A call then compiles to the same instructions on both routes, which
  the tests check. This is local to the bytecode backend: the lowered program
  keeps the design's shape.
- **Outputs** are emitted like any other store.

### Known difference

A narrow input field is the one place the routes can differ:
- **Lowered route.** It narrows every store to a narrow type
  (REQ-LOW-lowering-046), including a store to a function block's input field.
- **AST route.** It stores a function block field at 32 or 64 bits and never
  narrows it (`compile_place.rs:154`, and `compile_fb_call`'s inputs). So
  `f(x := a + b)`, with `x` an `INT` and `a + b` equal to 40000, stores 40000
  in the field today.

The two agree whenever the value fits the field. The lowered route already
narrows `f.x := a + b` outside a call, so a call's inputs then behave as that
assignment already does.

If an end-to-end test shows the difference, the core PR lists that program as
a known difference, as the design's "The routes check each other" allows. It
does not change either route.

### What stays on the AST route

Everything the AST route's statement compilers handle stays in place. A
compound statement that falls back, such as an `IF` holding a method call and
a function block call, compiles the call from the AST. So `compile_fb_call`
can only be deleted along with the rest of the AST route, not in this plan.

## Prefactoring

Each prefactor is behaviour preserving and its own PR.

1. **One source statement lowers to a list of statements.**
   - `Lowering::statement` returns `Made<Vec<Stmt>>`, and
     `StatementLowering::Lowered` holds a `Vec<Stmt>`.
   - `Lowering::statements` flattens the lists.
   - The bridge checks and emits every statement of the list.
   - `test_support` prints a list one statement per line.
   - Every statement still lowers to a list of one, so nothing else changes.
2. **One function stores a value at a target.**
   - `Lowering::assignment` (`lowering/src/statement.rs:37`) keeps only the
     `S=`/`R=` refusal.
   - The rest moves to a `store` of a source value to a target `Variable`,
     which picks a bit store, a string store or a scalar store with its
     narrowing.
   - An output assignment reuses it.
3. **The instructions of a function block call are shared.**
   - The sequence `compile_fb_call` emits moves to a small module,
     `compile_fb_call.rs`: open the call on an instance by name (and its
     `FB_LOAD_INSTANCE`), store an input, close the call (`FB_CALL`, the
     call-graph edge, `POP`).
   - `compile_fb_call` uses it, and so will the bridge.
   - The bytecode is unchanged.

## Design Doc Reference

- `specs/design/lowered-program.md`: section 3.7 (`FbCall`,
  REQ-LOW-lowering-069), "Standard function blocks", and "Delivery
  Constraints".
- `specs/design/call-binding.md`: normal form and recorded outputs.
- `specs/adrs/0060-lowering-generates-the-standard-function-blocks.md`, which
  stands; its work is #2288.

No design document or ADR changes.

## File Map

| File | Change |
|---|---|
| `compiler/ir/src/lowered/stmt.rs` | `StmtKind::FbCall` and its constructor |
| `compiler/ir/src/lowered/program.rs` | `ProgramBuilder::fb_call(instance, span)` |
| `compiler/ir/src/lowered/print.rs` | Print `fb_call <instance>` |
| `compiler/ir/src/lowered/tests.rs` | `FbCall` built, refused and printed |
| `compiler/lowering/src/lib.rs` | `StatementLowering::Lowered(Vec<Stmt>)` (prefactor 1) |
| `compiler/lowering/src/statement.rs` | List of statements; `store` split out (prefactors 1, 2); the `FbCall` arm (core 1) |
| `compiler/lowering/src/control.rs` | Flatten statement lists (prefactor 1) |
| `compiler/lowering/src/fb_call.rs` | New: inputs, `FbCall`, outputs |
| `compiler/lowering/src/fb_call_tests.rs` | New: lowering tests |
| `compiler/lowering/src/expr.rs` | `CallOutput` read from the current call's instance |
| `compiler/lowering/src/test_support.rs` | Print a list of statements |
| `compiler/codegen/src/compile_fb_call.rs` | New: the shared call sequence (prefactor 3) |
| `compiler/codegen/src/compile_stmt.rs` | `compile_fb_call` uses it |
| `compiler/codegen/src/lowered/mod.rs` | Emit a list; `can_emit` for `FbCall` |
| `compiler/codegen/src/lowered/stmt.rs` | Emit `FbCall`, and the one-load run of inputs |
| `compiler/codegen/tests/it/compile_route_lowered_fb_call.rs` | New: route tests |
| `compiler/codegen/tests/it/compile_route.rs` | A new example of an unsupported statement (core 2) |

## Tasks

### Prefactor 1: a list of statements per source statement

- [ ] `Lowering::statement` returns `Made<Vec<Stmt>>`, `statements` flattens,
      and `StatementLowering::Lowered` holds a `Vec<Stmt>`
- [ ] `compile_by_route` checks and emits each statement of the list, after
      recording the source statement's position once, as today
- [ ] `test_support` prints a list one statement per line; existing tests
      unchanged
- [ ] `cd compiler && just`; the lowered-route count is unchanged

### Prefactor 2: one function stores a value at a target

- [ ] `store(frame, target, value, span)` holds the bit, string and scalar
      stores; `assignment` keeps the `S=`/`R=` refusal and calls it
- [ ] `cd compiler && just`; the lowered-route count is unchanged

### Prefactor 3: the instructions of a function block call are shared

- [ ] `compile_fb_call.rs`: open, store an input, close
- [ ] `compile_fb_call` uses it; every existing codegen test passes
      unchanged, the bytecode tests included
- [ ] `cd compiler && just`

### Core 1: `FbCall`, and calls of user-defined function blocks

- [ ] IR: `StmtKind::FbCall { instance, block }`, built by
      `ProgramBuilder::fb_call` from a place of a function block type (P9998
      otherwise), printed as `fb_call <instance>`, with the IR tests
- [ ] Lowering (`fb_call.rs`):
  - the instance place;
  - each named input stored to its field;
  - `FbCall`;
  - each output's recorded value stored to its target, with `CallOutput`
    reading the instance's field;
  - a call of a standard block is unsupported for now.
- [ ] Error handling in lowering:
  - a positional input, or an output with no recorded value, is a P9998, as
    in `compile_fb_call` and `store_outputs`;
  - a `CallOutput` outside an output's value is unsupported, not a P9998,
    while function and method calls with outputs are not lowered.
- [ ] Bridge:
  - `can_emit` reaches the instance by name in `ctx.fb_instances`;
  - `FbCall` emits `FB_LOAD_INSTANCE`, `FB_CALL`, `POP` and the call-graph
    edge;
  - a run of input stores ending in the call is emitted with one instance load.
- [ ] Lowering tests:
  - inputs in declaration order whatever the source order;
  - an omitted input has no store;
  - an output converted to a wider target;
  - a negated output;
  - a call with no arguments;
  - a call inside `IF` and `FOR`;
  - an instance in a function block's body;
  - a standard block is unsupported.
- [ ] Codegen route tests, every statement required to lower:
  - same code as the AST route for each shape above;
  - values after one scan for a counting block;
  - evaluation order of inputs with side effects;
  - the call-depth limit still counts a user-defined block.
- [ ] `cd compiler && just`; report the lowered-route count and how many
      function block calls still fall back, by reason

### Core 2: calls of standard function blocks

- [ ] Lowering: a call of a standard block lowers like any other; its POU has
      no body. The doc comment says the bytecode backend runs the VM's block
      until #2288.
- [ ] The bridge needs no new emission: `ctx.fb_instances` gives the VM's type
      id, as for `compile_fb_call`. Check the inputs and outputs of `TON`,
      `TOF`, `TP`, `CTU`, `CTD`, `CTUD` at each width, `R_TRIG`, `F_TRIG`,
      `SR` and `RS`.
- [ ] Tests that use a standard block call as their example of an unsupported
      statement get another example:
  - `compile_route.rs` (`SOURCE`);
  - `lowering/src/tests.rs` (`lower_statement_when_not_supported_yet_then_unsupported`).
- [ ] Lowering tests: a timer and a counter call lower to stores, `FbCall`
      and output stores.
- [ ] Codegen route tests: same code as the AST route for a timer, a counter
      at two widths, an edge detector and a bistable; values after one scan.
- [ ] `cd compiler && just`; report the lowered-route count

### Cleanup

- [ ] Open an issue for each shape of function block call that still falls
      back after core 2, with its count
- [ ] Close this plan PR unmerged, and close #2289 when both core PRs land

## Out of Scope

- **Generating the standard function blocks in lowering** (ADR-0060): #2288.
- **Method calls and method bodies**: the next plan. A method body is not
  lowered today because codegen asks lowering for no method body.
- **A function block's `VAR_IN_OUT`**: P9999 on both routes today.
- **Inputs and outputs a block inherits through `EXTENDS`**: lowering does not
  reach an inherited field yet.
- **The AST route's bugs with function block calls**, such as #1553, #1858
  and #2211: both routes keep failing the same way.
- **Deleting `compile_fb_call`**: it goes with the AST route (see
  [What stays on the AST route](#what-stays-on-the-ast-route)).
- **`lowered_floor`** in `compiler/justfile`: left as it is.
