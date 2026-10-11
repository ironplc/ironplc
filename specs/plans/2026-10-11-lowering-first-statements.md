# Plan: Start Lowering with the Statements That Need No Call Binding

## Goal

Start building the lowered program now, rather than after call binding is
finished. Build the IR, the `ironplc-lowering` crate and the bytecode
backend's route for lowered statements. Then move every statement that does
not need a binding decision onto that route, one top-level statement at a
time, as the design's Delivery Constraints describe.

Call binding ([pull request 2199](https://github.com/ironplc/ironplc/pull/2199))
runs alongside this work. Lowering takes a function block call, a method call
or a call with named arguments only after call binding records how that call
binds. Until then such a statement stays on the route that reads the AST.

This plan covers the infrastructure and scalar statements. Strings, function
block and method calls, standard function blocks, interfaces, `EN`/`ENO` and
the `init` function come in later plans, once this route exists and has been
measured.

## Architecture

### Why lowering does not wait for call binding

- **The design already allows a partial start.** Lowering grows one top-level
  statement at a time. A statement that needs a decision the analyzer does
  not record yet answers "unsupported" and stays on the route that reads the
  AST.
- **Calls are separable at statement granularity.**
  - A function block call is a top-level statement of its own, so it simply
    stays on the old route.
  - A statement whose expression contains a method call falls back as a whole.
- **Function calls are nearly ready.**
  - `xform_named_to_positional_args` rewrites named arguments into parameter
    order.
  - Every built-in function signature carries the analyzer's enum of built-in
    functions.
  - Codegen cannot pick a different function than the analyzer resolved (see
    the "Current behaviour" section of pull request 2199).
- **The first stretch touches no call at all:** the crate, the IR, the
  printer, the entry point, the route switch, the comparison of the two
  routes, and assignments, arithmetic and control flow.

### Why call binding still runs now

Most reproductions in pull request 2199 are programs that pass `check` and
then compute the wrong value:

- positional function block inputs are dropped;
- a user block named `CTU_INT` runs the built-in counter;
- a method's `VAR_IN_OUT` is passed by value;
- `NOT z => nz` ignores the `NOT`;
- a repeated input is accepted.

These are correctness defects whatever lowering does. If lowering took those
calls before they were fixed, it would copy today's binding, and the two
routes would agree on a bug.

### Gates between the two streams

| Lowering takes | Only after |
|---|---|
| A statement with no call | Nothing; it starts now |
| A call of a built-in function with positional arguments | Core 5 below |
| A call of a user function with positional arguments | Core 5 below |
| A call with named arguments, or with an omitted input | Call binding Core 4 (one binder, the call's normal form) |
| A function block call | Call binding Core 1 and Core 2, and issue 2125 (the conversion of a stored output) |
| A method call | Call binding Core 1 and Core 3 |
| A subscript whose type does not fit `DINT` | The analyzer records its conversion as checked (REQ-LOW-analyzer-162) |
| A directly represented variable | Issue 2050 |

Call binding Core 4 changes how a function call is recorded. If it lands
early, Core 5 below reads the normal form from the start and is not reworked.
Doing call binding Core 4 before Core 5 is preferred, but not required.

### Shape of the route

- **`ironplc-ir`** gains the lowered program's types alongside the execution
  model:
  - the ids;
  - `ScalarType`;
  - `Expr`, `Place` and `Stmt`;
  - constructors that check the invariants (REQ-LOW-ir-080, REQ-LOW-ir-081);
  - a text printer.
- **`ironplc-lowering`** is new. It takes a `CleanAnalysis`
  (REQ-LOW-lowering-011) and offers, while both routes exist, an entry point
  that lowers one top-level statement of one body. The answer is one of:
  - a lowered statement;
  - `Unsupported`, which is not a defect and is counted;
  - a P9998 for a broken invariant.
- **Codegen** asks lowering for each top-level statement, depending on a route
  option (`Ast`, `PreferLowered`, `RequireLowered`). It emits a lowered
  statement through a new module that reads only `ironplc-ir` types. It finds
  each variable's storage by the variable's source name and owning POU,
  through the body's current `Scope`, as the design allows while both routes
  exist.
- **The end-to-end helpers** compile each program under `Ast` and under
  `PreferLowered`, and compare every variable by name (REQ-LOW-codegen-131 is
  already met: the helpers resolve names through the debug section). A
  disagreement fails the test unless the program is on the list of known
  differences.

### Requirements tests while the design is partly built

A crate that lists a design in `build.rs` must test every requirement of that
design it owns. Until then its `spec_conformance` test fails on `UNTESTED`. So
each core PR below tests the requirements it meets with ordinary tests, and
names the requirement in each test's doc comment. Pull request 2167 did the
same for the execution model. A crate lists `lowered-program.md` in the PR
that tests the last requirement it owns, and its tests become `#[spec_test]`
then.

## Prefactoring

Each prefactor below is mechanical and its own PR.

1. **Rename the analyzer's `Intrinsic` to `BuiltinFunction`.** The design's
   Names section says this lands as one mechanical prefactor before the IR
   exists, so that `Intrinsic` names only the IR's typed enum. About 18 files
   in the analyzer and codegen use the name.
2. **Call `xform_insert_implicit_conversions` the recording pass.**
   - Its module doc comment opens "Lowering pass recording…".
   - ADR-0056 already has the postscript that renames it, so only the doc
     comment changes.
3. **Re-export the diagnostic types from `ironplc-ir`.** `ironplc-ir`
   re-exports `SourceSpan`, `FileId`, `Label` and `Diagnostic` (lowered
   program design, Position in the Pipeline), and the execution model's code
   names them through it. Nothing else changes.

No other prefactor is needed first. Codegen's statement compiler already has
one entry point per top-level statement (`compile_statements` in
`compile_stmt.rs`), which is where the route switch goes. Codegen's
addressing already goes through one `Place` path for most targets.

## Design doc reference

- [Lowered Program](../design/lowered-program.md):
  - Position in the Pipeline;
  - §3.1–3.7, §3.9 and §3.10;
  - Testing;
  - Delivery Constraints.
- [The Resolved Execution Model](../design/execution-model.md): the IR crate
  this work extends.
- [ADR-0057](../adrs/0057-backends-consume-a-target-neutral-lowered-program.md),
  [ADR-0058](../adrs/0058-the-analyzer-decides-and-lowering-translates.md),
  [ADR-0059](../adrs/0059-backends-depend-only-on-the-ir-crate.md),
  [ADR-0065](../adrs/0065-the-analyzer-builds-the-execution-model-until-lowering-exists.md).
- Call binding: pull request 2199 (plan), which this plan coordinates with.

## File map

| File | Change |
|---|---|
| `compiler/analyzer/src/intrinsic.rs` and its users | Prefactor 1: `Intrinsic` renamed `BuiltinFunction` |
| `compiler/analyzer/src/xform_insert_implicit_conversions.rs` | Prefactor 2: doc comment |
| `compiler/ir/src/lib.rs` | Prefactor 3: re-exports. Core 1: new modules |
| `compiler/ir/src/ids.rs` | Core 1: `VarId`, `PouId`, `FieldIdx`, `LoopId` newtypes |
| `compiler/ir/src/scalar.rs` | Core 1: `ScalarType`, `Signedness` |
| `compiler/ir/src/expr.rs`, `place.rs`, `stmt.rs` | Core 1: nodes and checking constructors |
| `compiler/ir/src/print.rs` | Core 1: text form of a lowered statement |
| `compiler/lowering/` (new crate: `Cargo.toml`, `build.rs`, `src/lib.rs`, `src/statement.rs`, `src/expr.rs`, `src/place.rs`, `src/variables.rs`) | Core 1 onwards: the pass |
| `compiler/Cargo.toml` | Core 1: workspace member |
| `compiler/codegen/Cargo.toml` | Core 2: depends on `ironplc-lowering` while both routes exist |
| `compiler/codegen/src/compile.rs` | Core 2: `Route` in `CodegenOptions` |
| `compiler/codegen/src/compile_stmt.rs` | Core 2: asks lowering per top-level statement |
| `compiler/codegen/src/lowered/` (new: `mod.rs`, `stmt.rs`, `expr.rs`, `place.rs`) | Core 2 onwards: emission from the IR |
| `compiler/codegen/tests/it/common/snapshot.rs`, `run.rs` | Core 2: compile both routes and compare |
| `compiler/codegen/tests/it/common/known_differences.rs` (new) | Core 2: programs where the routes may differ |
| `compiler/justfile` or the CI workflow | Core 2: report the count of lowered top-level statements |

## Core change PRs

### Core 1: the IR's statements and the lowering crate

- **IR types:**
  - ids, `ScalarType`, and the scalar `Expr` kinds (`Const`, `Read`,
    `Convert`, `Truncate`, `Unary`, `Binary`, `Compare`, `ShortCircuit`);
  - `Place` with `Var` and `Field` of the current instance;
  - `Stmt::Assign`.
- **Constructors that check** the invariants each node states
  (REQ-LOW-lowering-040 to 047).
- **A text printer**, used by lowering's tests from this PR on.
- **`ironplc-lowering`:**
  - `lower_statement(&CleanAnalysis, BodyRef, &StmtKind) -> StatementLowering`;
  - variable allocation by `DeclRef` through `resolve_variable`
    (REQ-LOW-lowering-020, REQ-LOW-lowering-022).
- **Lints:** the `deny` attributes of REQ-LOW-lowering-082 on `ironplc-ir` and
  `ironplc-lowering`.
- **No caller yet.** Nothing in codegen calls lowering, so every program
  compiles exactly as before.

### Core 2: the route switch and the first emitted statements

- **`Route` option.**
  - `Route::{Ast, PreferLowered, RequireLowered}` in `CodegenOptions`.
  - The default is `Ast`.
  - `ironplcc` gains a hidden option to choose the route, for tests and
    benchmarks.
- **Emission.** Codegen emits a lowered `Assign` of a scalar expression to a
  scalar variable or field of the current instance.
- **Comparing the two routes.**
  - The end-to-end helpers compile both routes and compare the results.
  - The known-differences list starts empty.
- **Progress report.** CI reports the number of top-level statements of the
  end-to-end programs that took the lowered route. The number must not fall.
- **Expected outcome.** Assignments of integer, real, boolean and bit string
  arithmetic take the lowered route.

### Core 3: control flow

- **Statements lowered:**
  - `If`, with `ELSIF` desugared;
  - `Case`, with the selector evaluated once (REQ-LOW-codegen-124);
  - `WHILE` and `REPEAT` desugared to `Loop`;
  - `For`, keeping the fused compare and branch (REQ-LOW-ir-068);
  - `Exit` and `Continue` by `LoopId`;
  - `Return`.
- **The `Case` correction.** Evaluating the selector once is the design's
  first correction. The programs that show it, a selector with a side effect,
  go on the known-differences list.

### Core 4: places

- **Places lowered:**
  - `Index`, for subscripts already of type `DINT` or narrower;
  - `Deref`, nullable and non-null (REQ-LOW-lowering-035, REQ-LOW-lowering-036);
  - structure fields;
  - bit and partial reads (REQ-LOW-lowering-100);
  - `AssignBits` (REQ-LOW-lowering-102).
- **The `AssignBits` correction.** It names its place once, so its subscripts
  are evaluated once. That is the design's second correction, and it goes on
  the known-differences list.
- **Subscripts that may not fit `DINT`.** A subscript of a type that holds
  values outside the range of `DINT` stays unsupported until the analyzer
  records its conversion as checked.

### Core 5: calls of functions with positional arguments

- **What lowers.** Calls of built-in and user functions whose arguments are
  all positional and all scalar:
  - `Callee::Intrinsic`, mapped from `BuiltinFunction` to `Intrinsic`
    (REQ-LOW-ir-145, REQ-LOW-lowering-147);
  - `Callee::User(PouId)`;
  - `Arg::Copy` for a scalar input.
- **The bytecode backend** maps `Intrinsic` to a `func_id`
  (REQ-LOW-codegen-074).
- **What stays on the AST route:**
  - a call with named arguments or an omitted input, until call binding
    Core 4;
  - a string or aggregate argument or result.

## Tracking issue

This plan needs more than one core change PR, so
[issue 2205](https://github.com/ironplc/ironplc/issues/2205) lists
Prefactors 1–3 and Cores 1–5. It also names the gates that call binding (pull
request 2199) must pass before lowering takes calls.

## Tasks

### Prefactor 1: `BuiltinFunction`

- [ ] Rename the analyzer's `Intrinsic` to `BuiltinFunction` and
  `intrinsic.rs` to `builtin_function.rs`; update the analyzer and codegen
- [ ] Check that no `Intrinsic` names the analyzer's enum anywhere
- [ ] `cd compiler && just`

### Prefactor 2: the recording pass

- [ ] Reword the module doc comment of `xform_insert_implicit_conversions.rs`
  to call it the recording pass
- [ ] `cd compiler && just`

### Prefactor 3: re-exports from `ironplc-ir`

- [ ] Re-export `SourceSpan`, `FileId`, `Label` and `Diagnostic` from
  `ironplc-ir`, and name them through it in `ironplc-ir`'s own modules
- [ ] `cd compiler && just`

### Core 1: IR statements and `ironplc-lowering`

- [ ] Add the id newtypes, `ScalarType`, the scalar `Expr` kinds, `Place`
  (`Var`, current-instance `Field`) and `Stmt::Assign` to `ironplc-ir`, with
  private fields and checking constructors
- [ ] Add a text printer and test that every node prints
- [ ] Test REQ-LOW-ir-019, REQ-LOW-ir-080, REQ-LOW-ir-081 and
  REQ-LOW-lowering-040 to 047 with ordinary tests naming each requirement
- [ ] Create `compiler/lowering` with `lower_statement` taking a
  `CleanAnalysis`, returning a lowered statement, `Unsupported`, or P9998
- [ ] Allocate `VarId`s by `DeclRef`; test REQ-LOW-lowering-020 and 022
- [ ] Add the `deny` attributes (REQ-LOW-lowering-082) and a test that each
  crate root carries them
- [ ] `cd compiler && just`

### Core 2: route switch and assignments

- [ ] Add `Route` to `CodegenOptions` (default `Ast`) and a hidden `ironplcc`
  option
- [ ] In `compile_statements`, ask lowering per top-level statement under
  `PreferLowered` and `RequireLowered`
- [ ] Add `codegen/src/lowered/` to emit `Assign` and the scalar `Expr` kinds,
  finding storage through the current `Scope` by name and owning POU
- [ ] Compile each end-to-end program under both routes in the helpers and
  compare variables by name; add the empty known-differences list
- [ ] Report the count of lowered top-level statements in CI and fail if it
  falls below the recorded baseline
- [ ] `cd compiler && just`

### Core 3: control flow

- [ ] Lower `If`/`ELSIF`, `Case`, `WHILE`, `REPEAT`, `For`, `Exit`,
  `Continue`, `Return`; emit each
- [ ] Add the side-effecting `Case` selector programs to the known
  differences
- [ ] Test REQ-LOW-lowering-059, 062 to 067 and REQ-LOW-codegen-124
- [ ] `cd compiler && just`

### Core 4: places

- [ ] Lower `Index` (subscripts of `DINT` or narrower), `Deref`, structure
  fields, bit and partial reads and `AssignBits`
- [ ] Leave a subscript whose type may not fit `DINT` unsupported
- [ ] Add the side-effecting bit-write subscript programs to the known
  differences
- [ ] Test REQ-LOW-lowering-030 to 036, 100, 102, 161 and
  REQ-LOW-codegen-120, 121
- [ ] `cd compiler && just`

### Core 5: positional function calls

- [ ] Add `Callee` (`Intrinsic`, `User`), `Arg::Copy` and the `Intrinsic`
  enum, one variant per operation and operand type
- [ ] Map `BuiltinFunction` to `Intrinsic` in one exhaustive match
- [ ] Lower calls whose arguments are all positional and scalar; leave the
  rest unsupported
- [ ] Map `Intrinsic` to `func_id` in the bytecode backend
- [ ] Test REQ-LOW-ir-145, REQ-LOW-lowering-147, 071, 072 and
  REQ-LOW-codegen-074
- [ ] `cd compiler && just`

### Verification (every PR)

- [ ] The end-to-end suite passes unchanged under `Ast`
- [ ] From Core 2 on, the two routes agree on every end-to-end program not on
  the known-differences list
- [ ] The count of lowered statements does not fall
- [ ] `cd specs && just`

## Out of scope

These come in a later plan:

- strings (`StrExpr`, `StringShape`, `AssignStr`);
- aggregate copies (`Copy`);
- function block and method calls;
- standard function blocks (ADR-0060);
- interfaces;
- `EN`/`ENO`;
- `init` and the function prologue;
- a layout keyed by id (REQ-LOW-codegen-122, 123);
- moving `execution_model` into `ironplc-lowering` (ADR-0065);
- `check` running lowering (REQ-LOW-project-004);
- the minimal WebAssembly path.

## Risks

- **Moving the execution model.** `CleanAnalysis::new` resolves the
  execution model inside the analyzer. Lowering depends on the analyzer, so
  moving `execution_model` into `ironplc-lowering` (ADR-0065) also moves that
  resolution out of `CleanAnalysis`. The later plan has to settle where it
  goes. This plan does not touch it.
- **Merge conflicts with call binding.** Both streams edit codegen's call
  files (`compile_stmt.rs`, `compile_method.rs`, `compile_call.rs`). Cores 1–4
  touch none of them beyond the route switch in `compile_statements`, so the
  overlap is small until Core 5.
- **The cost of comparing routes.** Every end-to-end program compiles and runs
  twice. If that slows the suite noticeably, compare under a feature or a
  separate test target, and keep the CI count either way.
