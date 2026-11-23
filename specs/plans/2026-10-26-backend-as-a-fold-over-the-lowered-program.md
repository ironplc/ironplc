# Plan: Make the bytecode backend a fold over the lowered program

This is a proposal, written to be agreed before code. It answers one
question: now that code generation consumes only the lowered program, can
`ironplc-codegen` be made smaller in a way that makes a WebAssembly backend
simpler to write and unable to disagree with the bytecode backend? Or has the
lowering work already reached the minimum?

The short answer is in two parts.

- **The IR is close to its minimum.** Its node kinds are each something every
  backend must implement, and the one node that is sugar (`For`) comes with
  its expansion. Three small additions close the places where two backends
  can still decide trap behaviour differently (see
  [Finding 5](#5-what-the-ir-lacks)). No redesign is needed.
- **The bytecode backend is not at its minimum.** About a third of its
  non-test lines (roughly 4,500 of 13,000 that are not instruction encoding)
  exist for one of three reasons, none of which is "the bytecode VM is a
  stack machine": it recognizes patterns in the lowered program instead of
  folding over it, it carries structure from the deleted AST route, and it
  accommodates three known limits of the VM whose fixes are already decided
  (issues 2120, 2288 and 2118). Every one of those lines is a line a
  WebAssembly backend would either have to copy or would do differently, and
  "differently" is where two backends disagree.

The proposal is to finish what the lowered program design started: make each
backend a *fold* over the IR, one match per node, with no knowledge of how
nodes are arranged and no decision that lowering already made. The bytecode
backend reaches that shape in a sequence of prefactors (behaviour-preserving
for programs, though the container's bytes change) and three core changes
that correct the VM. The WebAssembly backend is then written to the same
shape from the start, and the end-to-end suite runs both from a crate that
belongs to neither.

The file names below assume
[pull request 2507](https://github.com/ironplc/ironplc/pull/2507) has
landed: `codegen/src/body/` for the emission of lowered bodies and
`codegen/src/layout/declarations.rs` for the layout. All line counts are
`wc -l` of non-test source on that branch, blank lines and comments
included; they show proportions, not targets.

## Goal

A second backend implements the lowered program, not the bytecode backend's
reading of it. Concretely:

1. No backend recognizes a sequence or shape of lowered nodes to find out
   what a program means. Only a target optimization may recognize a shape,
   and only when the end-to-end suite proves it changes no result.
2. No backend computes anything that decides which inputs trap. That is in
   the IR.
3. Every variable of the lowered program, the temporaries lowering makes
   included, has storage in every backend, so `Arg::Ref` is an address on
   every target.
4. The bytecode backend's meaning is the IR's meaning: the three recorded
   departures (fields copied around a body, standard function blocks run
   natively, one size for every temporary string buffer) are closed, and
   until they are, the agreement suite lists them as expected differences
   rather than letting them pass as agreement.
5. The end-to-end suite and its harness are owned by neither backend, and a
   test a backend cannot run yet is an expected failure that the suite
   reports when it starts passing.

## Findings

### 1. Where the lines are

Non-test lines of `compiler/codegen/src`, grouped by what they do:

| Area | Files | Lines | Target-specific by nature? |
|---|---|---|---|
| Emission of lowered bodies | `body/*.rs` | 4,551 | Partly: the opcode choice is; the walk, the P9999 predicates and the pattern recognition are not |
| Layout | `layout/*.rs` | 2,807 | Mostly: slots, data region, descriptors, debug entries |
| Addressing a place | `compile_place.rs`, `compile_array.rs`, `compile_array_struct.rs`, `compile_struct.rs`, `compile_struct_init.rs`, `compile_string_place.rs`, `compile_subscript.rs`, `compile_partial_access.rs`, `access.rs` | 2,400 | The opcodes are; the three-stage translation and the shape cross product are not |
| Context, drivers, calls, type vocabulary | `compile.rs`, `compile_setup.rs`, `compile_fn.rs`, `compile_method.rs`, `compile_stmt.rs`, `compile_fb_call.rs`, `compile_user_call.rs`, `compile_string.rs`, `compile_reference.rs`, `compile_call.rs`, `compile_expr.rs`, `compile_arith.rs`, `compile_enum.rs`, `held_instances.rs` | 2,827 | Partly |
| Instruction encoding and its checks | `emit.rs`, `optimize/*.rs`, `stack_balance.rs`, `call_graph.rs`, `source_lookup.rs` | 4,634 | Yes |
| Capability checks | `execution.rs` | 380 | Yes |
| Conformance tests, test support | `spec_conformance*.rs`, `test_support.rs`, `lib.rs` | 2,143 | (tests) |

About 5,000 lines are instruction encoding and the checks a stack machine
needs, which no proposal can remove. The question concerns the other
13,000.

The backend raises a diagnostic at 133 sites: 67 `internal_error` (P9998),
36 `not_implemented` plus 9 `todo_with_span` (P9999), 14 `not_supported`
(P9997) and 7 user-facing `Problem`s. The design measured 145 P9999 and
P9998 sites before lowering existed ([Lowered Program](../design/lowered-program.md),
"Problem"); it is now 112, and about half of those are the pairs described
in [Finding 4](#4-where-the-backend-recognizes-patterns-instead-of-folding).

### 2. The IR is close to its minimum

`ironplc_ir::lowered` has 13 expression kinds, 13 statement kinds, 3
projections, 4 argument modes and one `Intrinsic` variant per operation and
operand type. Each is something every target must do: a backend cannot do
without `Read`, `Assign`, a conditional, a loop, a call, or a reference. The
one node that is sugar, `For`, carries its expansion (REQ-LOW-ir-068), so a
backend with no use for its bounds implements only `Loop`. `AssignBits`,
`Copy`, `ShortCircuit` and `InPlaceCall` each exist to state an evaluation
order or a place-once rule that a desugaring would lose
([Lowered Program](../design/lowered-program.md), §3.7 and §3.8).

Two things in the IR are surface a backend must name for a case that cannot
occur:

- `NarrowWidth::B32`. Lowering never produces a `Truncate` to 32 bits:
  `IntWidth::narrowed` gives a width only for 8 and 16 bits, and
  `lowering/src/string.rs` refuses the one path that could reach it. The
  bytecode backend handles it with an internal error
  (`body/expr.rs::emit_truncate`), and every backend would have to.
- `ScalarType::Interface` and the `StandardFunctionBlock` a POU can stand
  for. Both are placeholders for work that is decided and not done
  (ADR-0063, ADR-0060). They are not a problem of the IR's shape; they fall
  out when that work lands.

So the IR does not need to lose anything. What it needs is in
[Finding 5](#5-what-the-ir-lacks), and it is small.

### 3. Where the bytecode backend's meaning differs from the IR's

These are the places where the bytecode backend and a WebAssembly backend
*will* produce different results, because the VM does not yet do what the
lowered program means. All are recorded; this table only puts them beside
the code they cost and the tests they block.

| Construct | The IR's meaning | The bytecode backend today | Code that exists only for the difference | What a WebAssembly backend does |
|---|---|---|---|---|
| A body's instance fields | Worked on in place; a reference to a field stays valid; writes before a trap stay (ADR-0062, REQ-LOW-codegen-037, 038) | Copied into the block's slots before the body and back after it (`FB_CALL`, `METHOD_CALL`, `handle_frame_return` in `vm/src/vm.rs`); a string field of another instance holds a buffer index ([issue 1753](https://github.com/ironplc/ironplc/issues/1753)) | `held_instances.rs`, `body/instance_copy.rs`, `compile_fb_call.rs`, `copy_slots` and `Blocks` in `layout/`, `Stored::Field`, `ScopeId::Pou` for a block, `FbInstanceInfo::field_indices`, `UserFbTypeInfo::field_indices`, the `FbField` address, about 1,000 lines | Addresses `this + offset`. Differs on every program that takes a reference to a field, traps inside a body, or copies an instance with a string field |
| Standard function blocks | Ordinary function blocks lowering generates (ADR-0060, [issue 2288](https://github.com/ironplc/ironplc/issues/2288)) | The VM's native blocks (`vm/src/intrinsic.rs`, 705 lines); `_INT` counters count in 32 bits | `compile_call.rs::standard_fb_layout` and its field maps by name, `body/place.rs::reads_standard_block_field`, the truncating `Place::emit_assign`, `standard_blocks` in `CompileContext`, standard arms in `layout/region.rs`, `layout/calls.rs`, `held_instances.rs` | Cannot run one at all until 2288 lands. About 16 of the 181 end-to-end files use one |
| An aggregate passed to a `VAR_INPUT`, and a `Copy` inside a function body | The callee gets a copy; the destination has storage of its own (REQ-LOW-lowering-075, REQ-LOW-codegen-165) | Passes the caller's region offset (`body/aggregate.rs::emit_source`) and stores the source's slot in the destination's (`body/slot_copy.rs`): an alias. Hidden because a body that reads such a parameter or variable is P9999 | `body/slot_copy.rs`, `unstored` and `cleared` in `body/prologue.rs`, the `instance_slot` paths of `body/aggregate.rs` and `body/string.rs` | Copies. Agrees today only because the bytecode backend refuses the programs that would show the difference |
| Temporary string buffers | Each value's storage is sized from its own capacity (ADR-0064, REQ-LOW-codegen-055) | Every buffer has the size of the largest string in the program ([issue 2118](https://github.com/ironplc/ironplc/issues/2118)) | `temporary_buffers` in `layout/declarations.rs`, `max_string_capacity` and `has_wide_string` in `CompileContext`, `compile_string.rs` | Sizes each scratch buffer from its `StringShape`. No cutting program is known |

### 4. Where the backend recognizes patterns instead of folding

This is the finding that matters most for a second backend. Each row is a
place where the bytecode backend looks at how lowered nodes are arranged, or
inside an operand, to recover something, rather than emitting each node as
it comes. A WebAssembly backend would either copy the recognition (and its
corner cases) or not need it, and the two would then differ in exactly the
cases the recognition misses.

| Site | What it recognizes or reconstructs | Why it exists | Lines |
|---|---|---|---|
| `body/mod.rs` `can_emit`, `can_emit_all`, `expr_can_emit`, `operation_can_emit`, `args_can_emit`, `reaches`, `reads`, `subscripts_can_emit`; `body/fn_call.rs` `Call::can_emit`, `output_can_emit`, `in_place_can_emit`, `str_in_place_can_emit`; `body/string.rs` `str_can_emit`, `reaches_string`; `body/aggregate.rs` `can_emit`, `reaches_region`, `reaches_argument`; `body/fb_call.rs` `reaches_block` | A second walk of every statement that predicts whether the first walk, emission, will succeed, so that a P9999 is labelled at the statement. Every unsupported case is coded twice: a predicate that returns `false`, and an `internal_error` in the emitter that says `can_emit` should have rejected it (about 15 such pairs) | `compile` returns one `Diagnostic`, and during the two-route period the predicate chose the route | About 500 |
| `body/fn_call.rs` | Recovers a call from a run of statements: a `Call` statement, then the stores whose values read the temporaries it passed (`outputs_of`, `assigns_output`, `output_read`, `innermost`), then a `Copy` of the result temporary (`result_copy`), matched by `same_span`. Then reads each output not from the temporary the IR passes but from the callee's own slot (`Output::Scalar`, `Output::String`) | The temporaries lowering makes for outputs and results have no storage in this backend (REQ-LOW-codegen-175), so `Arg::Ref(temporary)` cannot be passed and the stores cannot be emitted as the `Assign`s they are | 617 |
| `body/fb_call.rs` `call_site`, `input`, `same_projection`, `untruncated` | Recovers a function block call from a run of stores to one instance followed by an `FbCall`, matched by `same_span`, to push the instance once. Strips the `Truncate` lowering placed on an input (`untruncated`) because `FbCallSite::store_input` narrows the value again by the field's type | The field copy model (`FB_STORE_PARAM` takes the instance below the value), and a leftover of narrowing by type at the store | About 150 of 241 |
| `body/init.rs`, `body/prologue.rs` | Regroups the statements of `init` and of each prologue by the declaration each stores into (`declarations`, `declaration`, `visited`, `planned`), so that the backend's setup of a variable's storage can be emitted immediately before the stores into it; decides which zero stores to leave out by walking the type (`kept`) | REQ-LOW-codegen-200 and 202 were written to reproduce the byte layout of the deleted AST route, which interleaved setup and stores | 550 |
| `body/stmt.rs` `exit_when`, `exit_unless`, `loop_` | Recovers the `Loop` shapes a `WHILE` and a `REPEAT` lower to, to emit a fused compare-and-branch | A target optimization. It changes no result, and it stays, but it is the pattern a fold must not need | About 100 |
| `access.rs`, `body/place.rs::access`, `compile_array.rs::resolve_access`, `compile_array_struct.rs`, `compile_struct.rs`, `compile_place.rs`, `compile_string_place.rs` | Translates an `ironplc_ir::Place` into `Access<S>`, then into one of six `ResolvedAccess` shapes, then into one of four `Address` or three `StringAddress` forms, with a function per pair of access kind and base shape (`resolve_struct_field_array`, `resolve_struct_array_element_field`, `string_element_field`, `walk_struct_chain`, `element_field_chain`) | `Access<S>` is generic over a subscript type because two routes once shared it; `Subscript` has one implementation now. The shape cross product is the VM's opcode families read back into the resolver | About 2,000 of 2,400 |
| `compile.rs` `OpWidth`, `Signedness`, `OpType`, `VarTypeInfo`, `DEFAULT_OP_TYPE`, `DEFAULT_VAR_TYPE_INFO`; `body/mod.rs::op_type`; `compile_expr.rs::emit_truncation` | A second type vocabulary beside `ScalarType`, converted at every boundary; 12 sites fall back to `DEFAULT_OP_TYPE` and 6 to `unwrap_or` | The design's dispositions for these (`OpType` becomes `ScalarType`, `VarTypeInfo` dissolves, `ParamPassing` dissolves into `Arg`) were not finished ([Lowered Program](../design/lowered-program.md), "Other codegen types") | About 200 |
| `compile_string.rs::allocate_string_temp` | Grows the data region while a body is emitted | A string function takes its operands as data-region offsets, so a computed operand needs a slot; the slot is allocated at the use rather than by the layout. This is the one site that modifies the layout during emission, against REQ-LOW-codegen-122 | 41 |
| `compile_array.rs::try_constant_flat_index` | Raises `P2027` (`ArrayIndexOutOfBounds`) from the backend for a literal subscript | A fallback for compiling without analysis, which no caller does since codegen takes a lowered program. Against REQ-LOW-codegen-113 | About 25 |

The rows share a cause. The bytecode backend was moved onto the lowered
program one statement at a time while the AST route still existed, and the
AST route set the shape: its layout, its calling convention, its order of
setup and stores. The lowered route reproduced that shape so that the two
routes agreed byte for byte. Now that the AST route is gone, the shape is
what remains of it.

### 5. What the IR lacks

Three things every backend computes for itself, where two computations can
disagree on which inputs trap or on what a debugger shows:

- **The flat position of an array element.** REQ-LOW-codegen-099 defines
  the bounds trap by the position "counted across all of the array's
  dimensions", and that position is a formula: row-major, each subscript
  less its dimension's lower bound, times the product of the later
  dimensions' sizes. The formula decides which combination of subscripts
  traps (`a[0, 5]` on `ARRAY[1..3, 1..4]` is the first element and does not
  trap), and the width it is computed at decides whether a wild subscript
  wraps back into range (the bytecode backend computes it at 64 bits for
  that reason, `compile_array.rs::emit_flat_index`). Both are meaning, not
  layout, yet the design places "the flat position and the stride" in the
  backend's layout. The stride, the bytes between elements, is layout. The
  position is not. A WebAssembly backend writes the same formula again, and
  a column-major slip or a 32-bit multiply is a silent disagreement that
  only an out-of-range subscript shows.
- **The trap codes.** Every backend reports a trap under the bytecode VM's
  code (REQ-LOW-codegen-098), and the codes live in `compiler/vm`.
  [Pull request 2306](https://github.com/ironplc/ironplc/pull/2306)
  already proposes a `TrapCode` in `ironplc-ir`; this plan adopts that.
- **The debug type of a variable.** `layout/debug_vars.rs` (438 lines)
  decides, from the type table, what tag and type name a debugger shows for
  each variable, with two sets of rules (as declared, as resolved). The
  end-to-end harness decodes every value it reads *by that tag*
  ([End-to-End Test Observation](../design/end-to-end-test-observation.md),
  §3). A WebAssembly backend's symbol map makes the same decision again. If
  the two classify a named subrange or a string alias differently, the
  harness decodes the two backends' values differently and reports a
  disagreement where there is none, or hides one.

And two conveniences that are not meaning but that every backend repeats:
the line and column of a span for a line map
(`compile_stmt.rs::record_position`), and `RoundTime`, which comes with
issue 2288.

### 6. The agreement suite

`codegen/tests/it/common/backend.rs` already has the `Backend` and
`Running` traits, `assert_backends_agree`, and a
`compile_on_every_backend` whose body is `let _ = run;` and a call of the
reference. The shape is right. Three things are not:

- It lives in the bytecode backend's own `tests/`, so a second backend's
  crate becomes a dev-dependency of the backend it is checked against, and
  the corpus of 181 `end_to_end_*.rs` files reads as the bytecode backend's
  tests.
- There is no way to mark a test a backend cannot run yet. The design
  leaves this to "the first new backend's design" and
  [pull request 2306](https://github.com/ironplc/ironplc/pull/2306) defers
  it again. Without it, the second backend's first months are either a
  separate corpus (two sources of truth) or a suite that cannot be green.
- There is no way to record an *expected disagreement*. Until issues 2120
  and 2288 land, the two backends will disagree on the programs in
  [Finding 3](#3-where-the-bytecode-backends-meaning-differs-from-the-irs),
  and an agreement check with no exception list is either red or disabled.

## Architecture

### A backend is a fold

The end state for every backend, the bytecode backend included, is three
functions that each do one thing:

```text
layout(program)            -> Layout          // once, before emission; keyed by id
emit(layout, stmt | expr)  -> instructions    // one match per node kind; no lookahead
check(execution_model)     -> Vec<Diagnostic> // target limits only
```

`emit` has these properties, each of which the bytecode backend meets today
in part:

1. **One match per node, exhaustive, with no predicate twin.** A node the
   backend does not handle yet is a P9999 returned from the arm that would
   emit it, at that node's span. There is no second walk that predicts the
   first. `compile` returns every diagnostic it found, one per body at most,
   so a user sees each gap in one run (the Backend Guide's rule, "report
   every target limit it finds").
2. **No lookahead across statements.** A `Call` statement is emitted as a
   call; the `Assign` after it that reads an output temporary is emitted as
   an `Assign`; an `FbCall` is emitted as a call of the instance; the
   `Assign`s to its fields before it are emitted as assignments. A backend
   that wants to push an instance once, or fuse a loop test, may recognize
   a shape *as an optimization* only, and the end-to-end suite must pass
   with the optimization off.
3. **No inspection inside an operand to undo lowering.** The backend never
   strips a `Truncate` lowering placed (`untruncated`) or searches for the
   innermost `Read` of an expression (`innermost`). It emits the operand.
4. **Every variable has storage.** A temporary lowering makes for an output
   or a result is a variable of the body that holds the call
   (`PouParts::locals` already lists them). The layout gives it a slot, or a
   run of the data region, as it does a `VAR_TEMP`. `Arg::Ref(place)` is
   then always the address of `place`, and a callee's `VAR_OUTPUT` and
   result parameter are what the IR says they are: non-null references the
   call binds (REQ-LOW-lowering-092, REQ-LOW-lowering-079).
5. **Setup is layout; values are `init`.** The backend sets up every piece
   of storage its layout allocated (string headers, the region offset an
   aggregate's slot holds) for every variable, in layout order, before the
   first statement of `init` or of a prologue, and then emits those
   statements in the order the IR lists them. It does not sort them by
   declaration. It may leave out a store that leaves cleared storage
   cleared, by one rule applied to the statement alone.

What stays target-specific, and should: the layout (slots, offsets,
descriptors, scratch for string operands), the choice of instruction for
each node, the implementation of each `Intrinsic`, the capability checks,
the encoding of debug information, and the peephole optimizer.

### What the IR gains

- **The flat position of an `Index`**, as an expansion in `ironplc-ir`, in
  the way `expand_for` gives the expansion of a `For`: given the array's
  dimensions and the subscripts of a `Projection::Index`, the `Expr` (at a
  stated width) whose value is the position, and the total number of
  elements it is checked against. A backend evaluates that expression and
  traps when the position is outside the total, then multiplies by its own
  stride. Both backends then trap on exactly the same subscripts. The design
  sentence that places the position in layout is amended; REQ-LOW-codegen-099
  gains the formula.
- **`TrapCode`**, as pull request 2306 proposes, with the test against the
  VM's problem-code list.
- **A debug classification of a type**: what a debugger reads a value of
  the type as (a signed or unsigned integer of a width, a real, a `BOOL`, a
  duration, a date, a time of day, a date and time, a string of an encoding,
  an enumeration, a structure, an array, a reference, an instance) and the
  name to show, as declared. Each backend maps the classification to its own
  tags. The harness decodes by the classification.
- **A position for a span**: `SourceLookup` gains the line and column of a
  span, so no backend converts offsets itself.
- `NarrowWidth::B32` is removed.

With issue 2288 the IR also gains `RoundTime` and loses
`StandardFunctionBlock`, as ADR-0060 decides; that is not new here.

### What does not change

ADR-0057 to ADR-0064 stand. The IR stays a tree. There is no shared backend
driver (see [Alternatives Considered](#alternatives-considered)). No layout
information enters the IR. The bytecode instruction set changes only where
issues 2120 and 2288 already require it.

### What the bytecode backend looks like after

| Module | Role after this plan | Lines (estimate) |
|---|---|---|
| `body/stmt.rs`, `body/expr.rs`, `body/string.rs`, `body/place.rs` | The fold: one arm per statement, expression, string expression and place; `For` fused; `Case` with the selector kept on the stack | About 1,600 |
| `body/init.rs`, `body/prologue.rs` | Setup for every variable, then the IR's statements | About 150 |
| `body/fn_call.rs`, `body/method.rs`, `body/fb_call.rs` | Arguments pushed as `Arg` says, then `CALL`, `METHOD_CALL` or `FB_CALL` | About 300 |
| `layout/` | Slots, regions, descriptors, debug entries, scratch for string operands; keyed by `VarId` and `TypeId` | About 1,800 |
| `address.rs` (new, replacing the addressing modules) | One pass from an `ironplc_ir::Place` to an `Address`, and the load, store and update of an `Address` | About 900 |
| `compile.rs`, `compile_fn.rs`, `compile_method.rs`, `compile_setup.rs`, `compile_call.rs`, `execution.rs` | The driver, the conversion opcodes, the capability checks | About 1,500 |
| `emit.rs`, `optimize/`, `stack_balance.rs`, `call_graph.rs`, `source_lookup.rs` | Unchanged | 4,634 |

About 13,000 non-test lines, from about 18,000, with the 5,000 lines of
encoding unchanged. The estimate is from reading, not from building; the
tasks below each say what they delete.

## Prefactoring

Every change but the three in group C is behaviour-preserving for programs:
every end-to-end test passes with its source and assertions unchanged. Most
change the container's bytes, so the `compile_*` and `wire_format` tests
that pin bytes are updated in the same PR, and `just container-diff` shows
what moved. The groups can proceed in parallel; the order within a group is
the order of dependence.

### Group A: the IR (target-neutral, small)

- **A1. Remove `NarrowWidth::B32`.** Lowering never produces it. Delete the
  variant and the arm that refuses it in `body/expr.rs::emit_truncate`.
- **A2. The flat position of an `Index` in `ironplc-ir`.** Add
  `expand_index` (name to taste) beside `expand_for`: the `Expr` for the
  position at 64 bits and the element total. Amend
  [Lowered Program](../design/lowered-program.md) §3.10 so that the formula
  is REQ-LOW-codegen-099's, and the sentence placing the position in layout
  says the stride alone is layout. The bytecode backend's `emit_flat_index`
  emits the expansion; `try_constant_flat_index` becomes constant folding
  of that expression with no diagnostic (an out-of-range literal subscript
  is P9998 behind the gate, since analysis reports P2027), which removes the
  last user-facing problem the backend raises for the program's content.
- **A3. `TrapCode` in `ironplc-ir`**, as pull request 2306's third
  prefactor.
- **A4. Shared debug helpers.** The debug classification of a type on
  `TypeTable`, and the line and column of a span through `SourceLookup`.
  `layout/debug_vars.rs` maps the classification to the container's tags;
  the harness's `Variables::decode` decodes by it. This one is optional for
  the bytecode backend and recommended before the WebAssembly symbol map
  is written.

### Group B: the bytecode backend as a fold (behaviour-preserving)

- **B1. Delete the predicate walk.** Remove `can_emit` and every predicate
  in [Finding 4](#4-where-the-backend-recognizes-patterns-instead-of-folding)'s
  first row. Each `internal_error` that says `can_emit` should have
  rejected the case becomes the `not_implemented` at the statement's span.
  `compile` returns `Result<Container, Vec<Diagnostic>>`: a body whose
  emission fails is abandoned at its first unsupported statement and the
  walk goes on to the next body. The six crates that call `compile`
  (`project`, `ironplc-cli`, `vm-cli`, `benchmarks`, `playground`, `mcp`)
  take the list; `ironplc_project::compile` already collects a list.
- **B2. Every temporary has storage; outputs and results go through the
  reference the IR passes.** The layout lays out each POU's `locals`, not
  only its `declarations`, so a temporary gets a slot or a region like a
  `VAR_TEMP`. A call passes `Arg::Ref(temporary)` as it passes a
  `VAR_IN_OUT`: a variable reference for a scalar or for the slot that holds
  a structure's region offset, a data-region reference for a string
  (`compile_reference.rs::reference_to`, as `REF_TO` makes them today). A
  callee's `VAR_OUTPUT` and result parameter are reference slots, as a
  `VAR_IN_OUT` is today (`in_out_params`), so `q^ := v` in the callee is
  `STORE_INDIRECT`, a string output `STR_STORE_ARRAY_DEREF` through the
  descriptor the layout registers for a reference to a string, and a field
  of a structure result `STORE_ARRAY_DEREF` through the structure's slot
  descriptor. A function that returns through its result parameter ends
  with `RET_VOID`. The stores after a call are then the ordinary `Assign`,
  `AssignStr` and `Copy` statements they are, emitted by the fold.
  `body/fn_call.rs` keeps only "push the arguments, call"; `outputs`,
  `string_outputs`, `return_string_info` and `return_struct_desc_index`
  leave `UserFunctionInfo` and `UserMethodInfo`; `CurrentFunctionReturn::String`
  goes. One program behaviour changes, as a correction: a function that
  reads the outputs of a function it calls runs instead of trapping
  (`InvalidVariableIndex`), since the temporary is the caller's own slot.
  REQ-LOW-codegen-175 is retired and the bytecode paragraph of
  [Lowered Program](../design/lowered-program.md) §3.8 "In-place calls" is
  rewritten.
- **B3. Setup first, then `init` in the IR's order.** `body/init.rs` sets
  up the storage of every global and every field of the program instance in
  layout order, then emits `init` as lowering lists it, leaving out a store
  of a zero constant into a scalar place (one rule, replacing `kept`) and a
  store into a resource global the layout keeps no storage for
  (REQ-EM-codegen-009). `body/prologue.rs` does the same for a body. The
  grouping by declaration, `declarations`, `declaration`, `visited` and
  `planned`, goes. REQ-LOW-codegen-200 and 202 are reworded to say setup
  precedes the statements, not that each store follows its declaration's
  setup.
- **B4. One type vocabulary.** `ScalarType` replaces `OpType`, `OpWidth`
  and `Signedness`; `VarTypeInfo` dissolves, its `storage_bits` into the
  `Truncate` the IR already carries and its op type into `ScalarType`;
  `ParamPassing` dissolves into `Arg` (`passes` and `passes_as` go, since
  the IR's constructors already checked the arguments against the
  parameters, REQ-LOW-lowering-072). `emit_truncation` by type goes with
  `FbCallSite::store_input`'s narrowing and `fb_call.rs::untruncated`: the
  backend trusts the `Truncate` lowering placed. The one remaining
  narrowing by type, a read of a standard block's field
  (`reads_standard_block_field`), stays until C1 and is marked as such.
  `DEFAULT_OP_TYPE` survives only where a slot holds a data offset.
- **B5. One address model.** Replace `access.rs`, the `Subscript` trait,
  `ResolvedAccess`, `compile_place.rs`, `compile_array.rs`,
  `compile_array_struct.rs`, `compile_struct.rs` and
  `compile_string_place.rs` with one module that folds an
  `ironplc_ir::Place` into an `Address` in a single pass over its
  projections: a base (a slot that holds the value, a slot that holds a
  region offset, a slot that holds a reference, or an instance until C2), a
  static slot offset accumulated from `Field` projections, an optional
  dynamic position from `Index` projections (A2's expansion), and what the
  value is (a scalar at a `ScalarType`, a string of a shape, an aggregate
  with its descriptor). Load, store and update are one match each over the
  base and the value kind. This is the largest bytecode-only change and the
  least urgent for agreement; it is here because it is where most of the
  2,400 lines go, and because the WebAssembly backend's `place` module will
  be written to this shape, so the two should look alike.
- **B6. Scratch for string operands in the layout.** The layout walks the
  bodies once and reserves a slot for every string operand that is not a
  plain place, so `allocate_string_temp` no longer grows the data region
  during emission (REQ-LOW-codegen-122 then holds without exception).

### Group C: close the VM's departures from the IR (corrections)

Each changes what some program does; each is already decided by an ADR and
tracked by an issue. The point of listing them here is their cost in the
backend and their order relative to the WebAssembly work.

- **C1. Lowering generates the standard function blocks
  ([issue 2288](https://github.com/ironplc/ironplc/issues/2288)).** Deletes
  `standard_fb_layout` and its five field maps, `standard_blocks`,
  `reads_standard_block_field`, the truncating `emit_assign`, and the
  standard arms of `layout/region.rs`, `layout/calls.rs` and
  `held_instances.rs` in the backend; `vm/src/intrinsic.rs` and the
  `FB_CALL` dispatch to it in the VM. Changes the `_INT` counters past the
  range of `INT`, as the issue records. **This must land before the
  WebAssembly backend supports any function block**, since it cannot run a
  standard block otherwise.
- **C2. The VM addresses a body's instance fields in place
  ([issue 2120](https://github.com/ironplc/ironplc/issues/2120)).** Deletes
  `held_instances.rs`, `body/instance_copy.rs`, `compile_fb_call.rs`, the
  `call_site` grouping in `body/fb_call.rs`, `copy_slots`, `Blocks`,
  `Stored::Field`, `ScopeId::Pou` for a block, and the field maps of
  `FbInstanceInfo` and `UserFbTypeInfo`; a field becomes an `Address` with
  a region base. Changes references to fields, writes before a trap, and
  string fields of other instances, as ADR-0062 records. **Until this
  lands, the agreement suite carries an expected-disagreement list (D3),
  and no claim of agreement is made.**
- **C3. Aggregates of a function, function block or method body get
  regions.** A structure, array or instance a body declares, and an
  `Arg::Copy` parameter, get a run of the data region, so a `Copy` copies
  and an input is a copy (REQ-LOW-codegen-165, REQ-LOW-lowering-075).
  Deletes `body/slot_copy.rs`, `unstored` and `cleared` in the prologue,
  and the `instance_slot` special cases. Lifts the P9999 on reading inside
  such a parameter. Can be done with or before C2.
- **C4. Size each temporary string buffer from its value
  ([issue 2118](https://github.com/ironplc/ironplc/issues/2118)).** A
  container format change with a design of its own; listed for
  completeness. No known program shows the difference, so it is last.

### Group D: the agreement suite

- **D1. Move the end-to-end suite and its harness to a crate that owns no
  backend.** `compiler/test` already holds the cross-crate guards
  (`backend_boundary_guard.rs`); the 181 `end_to_end_*.rs` files,
  `common/`, `harness_observation.rs` and `backend_agreement.rs` move there
  (or to a new `compiler/e2e`), with both backends as dev-dependencies. The
  `compile_*.rs` and `wire_format.rs` tests stay in `codegen/tests`, since
  they pin encoding. `compile_on_every_backend` runs every backend the
  crate knows. This should land before the WebAssembly crate exists, so the
  crate is born with the corpus.
- **D2. Expected failures, per backend.** A checked-in list of test names
  a backend cannot run yet (`wasm-not-yet.txt`, or a `not_yet = [Wasm]`
  argument on the `e2e!` macros). A listed test is run on that backend and
  **must fail**; when it starts passing the suite fails until the name is
  removed, so the list only shrinks and nothing is skipped silently. This is
  the mechanism [End-to-End Test Observation](../design/end-to-end-test-observation.md)
  leaves to the first new backend.
- **D3. Expected disagreements.** The same mechanism for programs on which
  the two backends are expected to differ until C1 and C2 land: the list
  names the test and the issue, the test must disagree while listed, and the
  list empties as the issues close.

### Order

```text
A1 ─┐
A2 ─┼─▶ B1 ─▶ B2 ─▶ B3 ─▶ B4 ─▶ (B5, B6 any time after B4)
A3 ─┘
A4 ──────────────────────────────────────▶ before the WebAssembly symbol map
D1 ─▶ D2 ─▶ D3 ───────────────────────────▶ before the WebAssembly crate
C1 ──────────────────────────────────────▶ before WebAssembly function blocks
C3 ─▶ C2 ────────────────────────────────▶ before any agreement claim
C4 ──────────────────────────────────────▶ last
```

The WebAssembly first pass of pull request 2306 can start after A2 and D1,
in parallel with group B. It should be written to the
[Architecture](#architecture) above from its first commit, which it mostly
already is.

## Decisions

Each is a choice about what IronPLC promises or a departure from recorded
text, so a person makes it. The recommendation and its reason follow each.

1. **The flat position of an array element is meaning, not layout (A2).**
   The design says the position and the stride are the backend's layout;
   this plan moves the position into the IR and leaves the stride. Options:
   amend the design as proposed; or keep the position in each backend and
   rely on the end-to-end programs for §3.10 to catch a disagreement.
   *Recommendation: amend.* A formula that decides which inputs trap is a
   rule of the language, and the programs that would catch a slip are the
   ones nobody writes.
2. **Output and result temporaries have storage, and a callee writes its
   outputs through the reference the call passes (B2).** This changes the
   bytecode calling convention for `VAR_OUTPUT` and for string and structure
   results: one indirection per output access in the callee, a slot per
   temporary in the caller, different `.iplc` bytes, and a function that
   reads a callee's outputs runs instead of trapping. Options: do it; or
   keep reading outputs from the callee's slots and keep `fn_call.rs`'s
   reconstruction. *Recommendation: do it.* It is the one change that makes
   every call site a fold, it is what a WebAssembly backend does without
   thinking, and ADR-0046's flat table is what makes the reference to a
   caller's slot valid. Record it in the design's §3.8; if the performance
   trade-off deserves a record of its own, as an ADR.
3. **Setup precedes `init`; stores are emitted in the IR's order (B3).**
   Changes the bytes of every container's `init` function and prologue, and
   reduces the zero-store elision to one rule. Options: as proposed; keep
   the interleaving and the type-walking elision. *Recommendation: as
   proposed.* The interleaving reproduces a layout that no longer exists.
4. **`compile` returns every diagnostic, one per body at most (B1).** An API
   change for six front-end crates; the CLI prints more than one P9999 per
   run. *Recommendation: yes*, as the Backend Guide already asks of a new
   backend.
5. **Where the end-to-end suite lives, and when it moves (D1).** Pull
   request 2306 keeps it in `codegen/tests` for the first pass and makes
   the WebAssembly crate a dev-dependency of codegen. Options: move now;
   move after the first pass. *Recommendation: move now.* A suite that
   belongs to one backend is that backend's tests; the day it has two
   backends it is the language's tests, and the second backend should not
   be born as a dev-dependency of the first.
6. **Order of C1 and C2 against the WebAssembly work.** Options: WebAssembly
   excludes function blocks until C1 and claims nothing about agreement
   until C2 (as proposed); or WebAssembly implements the standard blocks
   itself for now. *Recommendation: as proposed.* A second implementation
   of the standard blocks is the duplication ADR-0060 removes.
7. **A shared debug classification in the IR (A4).** Small; optional for
   the bytecode backend. *Recommendation: yes*, before the WebAssembly
   symbol map, so the harness decodes both backends by one rule.
8. **The fused compare-and-branch and the `WHILE`/`REPEAT` shape recovery
   stay (B4 leaves them).** They are target optimizations that change no
   result; the end-to-end suite already covers them. *Recommendation:
   keep*, and add a test that the suite passes with the fusion disabled, so
   the fold is shown correct without it.

## Design doc references

- [Lowered Program](../design/lowered-program.md): §3.10 (the flat
  position, A2), §3.8 "In-place calls" (B2), §7 "Backend Contract"
  (REQ-LOW-codegen-122, 175, 200, 202), §8 "Other codegen types" (the
  dispositions B4 finishes), and the `NarrowWidth` passage (A1).
- [Backend Guide](../steering/backend-guide.md): "Never Decide What Lowering
  Decided" gains rows for the flat position and the output temporaries;
  "Shared Pieces Still in Bytecode Crates" loses the harness and the trap
  codes once D1 and A3 land.
- [End-to-End Test Observation](../design/end-to-end-test-observation.md):
  "Backends" records where the suite lives (D1) and the expected-failure
  mechanism (D2, D3).
- [The Resolved Execution Model](../design/execution-model.md): unchanged.
- ADR-0057 to ADR-0064: unchanged. ADR-0046 is cited by B2.

## File map

| File | Change | Step |
|---|---|---|
| `compiler/ir/src/lowered/types.rs` | Remove `NarrowWidth::B32`; add the debug classification | A1, A4 |
| `compiler/ir/src/lowered/loops.rs` or a new `index.rs` | `expand_index`: the position `Expr` and the element total | A2 |
| `compiler/ir/src/trap.rs` (new) | `TrapCode` | A3 |
| `compiler/ir/src/source.rs` | Line and column of a span | A4 |
| `compiler/codegen/src/body/mod.rs` | Delete the predicate walk; `emit_body` returns the body's diagnostics | B1 |
| `compiler/codegen/src/compile.rs` | `compile` returns `Vec<Diagnostic>`; `CompileContext` loses `var_types`, `in_out_params` (outputs join it first), `max_string_capacity`, `has_wide_string`, `standard_blocks` | B1, B2, B4, B6, C1 |
| `compiler/codegen/src/body/fn_call.rs`, `body/method.rs`, `body/call_args.rs` | Push arguments as `Arg` says, then call | B2, B4 |
| `compiler/codegen/src/layout/declarations.rs`, `layout/calls.rs` | Lay out `locals`; outputs and results as reference slots; no `Calling::outputs` | B2 |
| `compiler/codegen/src/body/init.rs`, `body/prologue.rs` | Setup for every variable, then the IR's statements | B3 |
| `compiler/codegen/src/compile_expr.rs`, `compile_arith.rs`, `compile_call.rs` | `ScalarType` in every signature; `emit_truncation` deleted | B4 |
| `compiler/codegen/src/compile_fb_call.rs`, `body/fb_call.rs` | `store_input` stops narrowing; `untruncated` deleted; grouping deleted at C2 | B4, C2 |
| `compiler/codegen/src/address.rs` (new) | One pass from `Place` to `Address`; load, store, update | B5 |
| `compiler/codegen/src/access.rs`, `compile_place.rs`, `compile_array.rs`, `compile_array_struct.rs`, `compile_struct.rs`, `compile_string_place.rs`, `compile_subscript.rs` | Deleted | B5 |
| `compiler/codegen/src/compile_string.rs` | `allocate_string_temp` reads a slot the layout reserved | B6 |
| `compiler/codegen/src/held_instances.rs`, `body/instance_copy.rs`, `body/slot_copy.rs` | Deleted | C2, C3 |
| `compiler/vm/src/intrinsic.rs`, `vm/src/vm.rs` | Standard blocks removed; fields in place | C1, C2 |
| `compiler/test/tests/e2e/` (or `compiler/e2e`) | The 181 end-to-end files, `common/`, the agreement check, the expected-failure lists | D1, D2, D3 |
| `compiler/codegen/tests/it/` | Keeps `compile_*.rs` and `wire_format.rs` | D1 |
| `specs/design/lowered-program.md`, `specs/steering/backend-guide.md`, `specs/design/end-to-end-test-observation.md` | The amendments above | A2, B2, B3, D1 |

## Tasks

### A1: Remove the truncation width no program produces

- [ ] Delete `NarrowWidth::B32` and its arms in `ironplc-ir`, lowering and
      `body/expr.rs::emit_truncate`
- [ ] Reword the `NarrowWidth` doc comment; `cd compiler && just`

### A2: The flat position of an array element in the IR

- [ ] Add `expand_index` to `ironplc-ir`: the position `Expr` at 64 bits
      and the element total, from the array's dimensions and the subscripts
- [ ] Tests: one dimension, several dimensions, lower bounds other than 0
      and 1, a constant-folded position
- [ ] `emit_flat_index` emits the expansion; delete
      `try_constant_flat_index` and the backend's `P2027`
- [ ] Amend §3.10 and REQ-LOW-codegen-099; add the end-to-end program for a
      subscript out of range in one dimension that lands inside the array
- [ ] `cd compiler && just` and `cd specs && just`

### A3: Trap codes in the IR

- [ ] As pull request 2306's prefactor 3

### A4: Shared debug helpers

- [ ] The debug classification on `TypeTable`; `layout/debug_vars.rs` maps
      it to tags
- [ ] Line and column of a span through `SourceLookup`
- [ ] The harness's `Variables::decode` decodes by the classification
- [ ] `cd compiler && just`

### B1: Delete the predicate walk

- [ ] Remove every `can_emit`, `reaches` and `reads` predicate; turn each
      "`can_emit` should have rejected" internal error into the P9999 at the
      statement
- [ ] `emit_body` abandons a body at its first unsupported statement and
      returns its diagnostic; `compile` returns `Vec<Diagnostic>`
- [ ] The six callers take the list
- [ ] A test that two bodies with one unsupported statement each report two
      P9999s
- [ ] `cd compiler && just`

### B2: Every temporary has storage

- [ ] The layout lays out each POU's `locals` beyond its `declarations`
- [ ] A `VAR_OUTPUT` and a result parameter are reference slots; a call
      passes a reference to the temporary for each
- [ ] A string output and a string result are written through the
      descriptor the layout registers for a reference to a string; a
      structure result through the structure's slot descriptor
- [ ] A function returning through its result parameter ends with
      `RET_VOID`; `CurrentFunctionReturn::String` goes
- [ ] `body/fn_call.rs` keeps only arguments and the call; the stores are
      emitted by the fold
- [ ] Delete `outputs`, `string_outputs`, `return_string_info`,
      `return_struct_desc_index`; update `compile_*` byte tests
- [ ] Add the end-to-end test for a function that reads the outputs of a
      function it calls
- [ ] Rewrite the bytecode paragraph of §3.8 "In-place calls"; retire
      REQ-LOW-codegen-175
- [ ] `cd compiler && just` and `cd specs && just`

### B3: Setup first, then `init`

- [ ] `body/init.rs`: setup for every stored global and program field in
      layout order, then `init` as listed, with the one elision rule and the
      resource-global filter
- [ ] `body/prologue.rs`: the same for a body
- [ ] Reword REQ-LOW-codegen-200 and 202; update the byte tests
- [ ] `cd compiler && just` and `cd specs && just`

### B4: One type vocabulary

- [ ] `ScalarType` replaces `OpType`, `OpWidth`, `Signedness`; `VarTypeInfo`
      and `ParamPassing` dissolve; `emit_truncation` by type goes, as do
      `store_input`'s narrowing and `untruncated`
- [ ] `DEFAULT_OP_TYPE` only where a slot holds a data offset; mark
      `reads_standard_block_field` as the one narrowing by type, until C1
- [ ] Finish the "Other codegen types" dispositions in §8
- [ ] `cd compiler && just`

### B5: One address model

- [ ] `address.rs`: `Address` from a `Place` in one pass; load, store and
      update; the string and aggregate kinds
- [ ] Delete the seven addressing modules; move their unit tests onto
      `Address`
- [ ] `cd compiler && just`

### B6: Scratch for string operands in the layout

- [ ] The layout reserves a slot per computed string operand; emission reads
      it; REQ-LOW-codegen-122 holds without exception
- [ ] `cd compiler && just`

### C1 to C4

- [ ] Issue 2288, then issue 2120 (with C3), then issue 2118, each with the
      deletions listed under [Group C](#group-c-close-the-vms-departures-from-the-ir-corrections)
      and the expected-disagreement list of D3 emptied as each lands

### D1 to D3: the agreement suite

- [ ] Move the end-to-end files, the harness, `harness_observation.rs` and
      `backend_agreement.rs` to the neutral crate; both backends as
      dev-dependencies
- [ ] Expected-failure lists per backend, with the "must fail while
      listed" check
- [ ] Expected-disagreement list naming issues 2120 and 2288
- [ ] Record both in End-to-End Test Observation, "Backends"
- [ ] `cd compiler && just`

## Alternatives considered

**A shared backend driver.** Both the bytecode VM and WebAssembly are stack
machines, so one walk could call a target trait (`push_const`, `load`,
`store`, `binary`, `branch`) and the two backends would share about 1,500
lines of fold. Rejected for now. The control flow differs in kind (labels
and jumps against `block`, `loop` and `br`), an LLVM backend would fit
neither, and the exhaustive match per backend is the guard ADR-0061 relies
on. The project's rule is to extract a shared shape when the second caller
exists; the WebAssembly backend is that caller, so the comparison should be
made once it is written to the shape above. The IR's expansions
(`expand_for`, A2's `expand_index`) are the right place for sharing today.

**Do nothing in the backend; the IR is minimal.** The IR is close to
minimal, but a backend that reconstructs calls from statement runs by span
and reads outputs from the callee's slots is a second reading of the IR, and
a second backend written from the IR alone will read it differently. The
agreement risk is in the backend, not the IR.

**Have lowering write outputs straight to their targets, so no temporary
exists.** Rejected by the design for aliasing, traps and conversion
([Lowered Program](../design/lowered-program.md), §3.8). The temporary is
right; what is wrong is that one backend gives it no storage.

**Put offsets in the IR so no backend computes a layout.** Rejected: the two
targets' layouts differ (slots and a data region against one linear memory),
and REQ-LOW-lowering-027 keeps the type table free of them. Only the
position of an element, which decides a trap, moves.

**Share the string intrinsics.** The IR has no node for a code unit of a
string, so `CONCAT` or `FIND` cannot be lowered to IR; each backend
implements them, and the end-to-end programs for each rule of §3.10 are the
guard. This is the irreducible per-backend surface, along with the
conversions and the real functions.

## Not in this plan

Calls through an interface (pull request 1870), sequential function charts,
`RETAIN`, a different representation of time, and the WebAssembly backend
itself. Each is constrained by this plan only in that it should be written
as a fold.
