# Backend Guide

This guide describes everything needed to add a new backend to the IronPLC
compiler: a code generator for a target other than the bytecode VM, such as
WebAssembly, native code through LLVM, or C. It also covers what every
existing backend has to do when the IR gains a node.

> **Note**: The contract a backend works to is specified in
> [Lowered Program](../design/lowered-program.md); this guide is the checklist
> and the rules for meeting it, and cites that design by section rather than
> restating it. The decisions behind it are
> [ADR-0057](../adrs/0057-backends-consume-a-target-neutral-lowered-program.md)
> to [ADR-0064](../adrs/0064-every-string-has-an-explicit-capacity.md). For the
> rest of the pipeline, see [compiler-architecture.md](compiler-architecture.md).

## What a Backend Is

A **backend** consumes a **lowered program**, a program in IronPLC's **IR**
(the crate `ironplc-ir`), and nothing else. Lowering has already resolved
every name, made every decision the language makes, desugared every construct
that can be expressed with others, and generated the standard function blocks
as ordinary function blocks. The bytecode backend, `ironplc-codegen`, is the
existing backend: read it for patterns, but the rules of the design, not its
behavior, define what a program means. The VM has defects of its own.

A backend decides exactly four things
([Backend Contract](../design/lowered-program.md#7-backend-contract)):

1. **Where each variable is stored** — its layout.
2. **Which instructions express each node.**
3. **How each `Intrinsic` is implemented.**
4. **How debug information is encoded.**

What each node *computes* is not among them. That is
[Meaning of operations](../design/lowered-program.md#310-meaning-of-operations),
and it is the same on every target.

**Why this matters**: a program that behaves differently on two targets is a
safety defect in a PLC
([ADR-0005](../adrs/0005-safety-first-design-principle.md)). Every decision a
backend makes again is a decision two backends can make differently.

## Quick Checklist

- [ ] **Design doc**: Add `specs/design/<target>-backend.md` with requirement
      IDs slugged to the new crate (`REQ-<AREA>-<crate-slug>-NNN`; see
      [Design Requirement](development-standards.md#design-requirement))
- [ ] **Shared pieces**: Move anything the backend needs out of a crate it may
      not depend on (see [Shared Pieces Still in Bytecode Crates](#shared-pieces-still-in-bytecode-crates))
- [ ] **Crate**: A new crate under `compiler/`, a member of the workspace,
      depending on `ironplc-ir` and none of the forbidden crates
- [ ] **Lint**: `#![deny(clippy::wildcard_enum_match_arm)]` at the crate root
- [ ] **Capability checks**: Check the execution model against what the
      target can run, and report each limit
- [ ] **Layout**: Computed once, before emission, keyed by id
- [ ] **Emission**: Every statement, expression, place, callee, argument and
      `Intrinsic`, with no wildcard arm
- [ ] **Meaning of operations**: For each rule, emit what makes up the
      difference between the rule and the target's instruction
- [ ] **Runtime**: Round time, scheduling, traps and the watchdog
- [ ] **Debug info**: Built from the names and spans the lowered program
      carries
- [ ] **End-to-end suite**: Runs on the new backend, compared with the
      bytecode backend by variable name
- [ ] **Pipeline**: `ironplc_project` passes the program it lowered once to
      the selected backend
- [ ] **Problem codes**: Each new target limit has a documented code
- [ ] **User docs**: How to select the target, and what it cannot run
- [ ] **CI**: `cd compiler && just`

## Never Decide What Lowering Decided

A backend that wants to decide something has almost always found a decision
that is already in the lowered program. Read it from there.

| If you are about to… | Read instead |
|---|---|
| Look a variable, field or POU up by name | Its `VarId`, `FieldIdx` or `PouId`. Names are on declarations for debug information only (REQ-LOW-lowering-021) |
| Convert between operand types because they differ | They do not differ. Every conversion is a `Convert` or `ConvertChecked` node (REQ-LOW-lowering-045) |
| Narrow a value before storing it | The `Truncate` is already there (REQ-LOW-lowering-046) |
| Choose between logical and bitwise `AND` | They are distinct operators (REQ-LOW-lowering-048) |
| Apply the default string capacity, or size a buffer from the largest string | Every `StringShape` states its capacity (REQ-LOW-lowering-054, REQ-LOW-codegen-055) |
| Recognize `TON`, `CTU_DINT` or another standard function block | There is nothing to recognize: each is an ordinary function block POU, called by `FbCall` ([ADR-0060](../adrs/0060-lowering-generates-the-standard-function-blocks.md)) |
| Read `CompilerOptions` or a behavior policy | The `Intrinsic` variant already encodes the policy (REQ-LOW-lowering-073, REQ-LOW-lowering-090) |
| Pick an overload of `SQRT` from its operand type | Each `Intrinsic` variant is one operation at one set of operand types ([ADR-0061](../adrs/0061-one-name-per-built-in-operation-and-operand-type.md)) |
| Work out which configuration, task or program instance runs | The execution model ([The Resolved Execution Model](../design/execution-model.md)) |
| Find the implementers of an interface | `Callee::Interface` lists them ([ADR-0063](../adrs/0063-a-call-through-an-interface-lists-its-implementers.md)) |
| Report that the program is invalid | Analysis already did. A backend reports only a limit of its target or a compiler defect (REQ-LOW-codegen-113) |
| Write an error path for a state analysis rules out | The IR's constructors refuse that state (REQ-LOW-ir-081), so it cannot reach you |

If the lowered program genuinely lacks something every target needs, the
change belongs in lowering and the IR, not in the backend (see
[When the IR Changes](#when-the-ir-changes)).

## Crate and Dependencies

A backend is a crate of its own under `compiler/`, named for its target (for
example `compiler/wasm`, published as `ironplc-wasm`). Add it to `members` in
`compiler/Cargo.toml`. Copy the version from a sibling crate's manifest:
versions are managed by the release tooling (`shared-version = true` in
`compiler/release.toml`) and are never edited by hand after that.

### What the crate may depend on

| Dependency | Allowed | Why |
|---|---|---|
| `ironplc-ir` | Yes | The lowered program, and what it re-exports from `ironplc-dsl` |
| `ironplc-problems` | Yes | The `Problem` a target-limit diagnostic names |
| The target's own libraries (an encoder, LLVM bindings) | Yes | Keep them minimal and cross-platform (Windows, macOS, Linux) |
| `ironplc-dsl`, `ironplc-parser`, `ironplc-analyzer`, `ironplc-lowering` | **Never**, outside `[dev-dependencies]` | The compiler, not a reviewer, keeps a backend from reaching the AST or analysis ([ADR-0059](../adrs/0059-backends-depend-only-on-the-ir-crate.md)) |
| `ironplc-codegen`, `ironplc-vm`, `ironplc-container` | **Never**, outside `[dev-dependencies]` | They are another backend and its target. Depending on one couples two targets that must stay independent |

Name what `ironplc-ir` re-exports from `ironplc-dsl` (see the lowered
program design's
[Position in the Pipeline](../design/lowered-program.md#1-position-in-the-pipeline))
through `ironplc-ir`, never through `ironplc-dsl`.

**The backend does not test its own boundary.** A check outside every backend
(`compiler/test/tests/backend_boundary_guard.rs`) reads each backend crate's
manifest and fails if `[dependencies]` names a forbidden crate (the rule the
design states for the bytecode backend as REQ-LOW-codegen-002). It finds
backend crates itself: every crate whose `[dependencies]` name `ironplc-ir` is
a backend, unless the guard lists it as one that builds the lowered program or
passes it to a backend. A new backend is therefore covered from its first
commit without registering anywhere and without a test of its own.

### Shared pieces still in bytecode crates

Some things every backend needs grew up inside the bytecode backend. When you
need one, move it to `ironplc-ir` (or to a small crate both backends can depend
on) before the backend uses it. Never add a dependency on the crate that holds
it.

- **The trap codes** (`V4001`–`V4006`, in `compiler/vm/resources/problem-codes.csv`
  and `Trap`): every backend reports a trap under the code the bytecode VM
  gives it (REQ-LOW-codegen-098).
- **The end-to-end harness** (`codegen/tests/it/common/`): `Snapshot` and
  `Session` run a program through the `Backend` and `Running` traits, which a
  new backend implements. The first new backend's design decides where the
  suite lives and how a backend marks a test it cannot run yet
  ([End-to-End Test Observation](../design/end-to-end-test-observation.md#backends)).

### The crate root

```rust
//! <Target> backend: compiles a lowered program to <artifact>.
//! See `specs/design/<target>-backend.md`.

// A match over a lowered enum names every variant, so a variant added later
// fails to compile wherever it is not handled (REQ-LOW-lowering-082).
#![deny(clippy::wildcard_enum_match_arm)]
```

The same check outside every backend fails when a backend's crate root lacks
the attribute, since a test cannot observe a lint that is not configured. The
backend writes no test for it.

The lint does not see a wildcard inside a tuple or slice pattern. Split
`match (op, ty) { …, _ => … }` into one exhaustive match per enum, which
leaves a wildcard only over a number or a slice length.

The rest of [compiler-standards.md](compiler-standards.md) applies unchanged:
no `panic!`, `unwrap` or `unreachable!`, a 1000-line module limit, and BDD
test names.

## The Entry Point

A backend's public entry point takes the lowered program, the backend's own
options and the source lookup, and returns the target's artifact or the
diagnostics that stopped it:

```rust
pub fn compile(
    program: &ironplc_ir::lowered::Program,
    options: &WasmOptions,              // target choices only
    sources: &dyn SourceLookup,
) -> Result<WasmModule, Vec<Diagnostic>>;
```

- **It takes a lowered program, never a `CleanAnalysis`.** `ironplc_project`
  lowers once and passes the same program to the backend
  (REQ-LOW-project-005). A backend never lowers.
- **Its options are target choices only**: a feature set, a target triple, an
  optimization level. They never include `CompilerOptions`, a dialect or a
  behavior policy (REQ-LOW-lowering-090).
- **Report every target limit it finds, not the first.** Return them all, as
  lowering does (REQ-LOW-lowering-110), so a user fixes them in one pass.

## Inside a Backend

Work in this order. Each phase reads only what the earlier ones produced.

### 1. Check the execution model against the target

The execution model says what runs and when: configurations, resources,
tasks, program instances and globals. Check it against what your runtime can
schedule before emitting anything, and report what it cannot as a target
limit. The bytecode backend's checks are REQ-EM-codegen-001 to
REQ-EM-codegen-008 in [The Resolved Execution Model](../design/execution-model.md#backend-capability-checks),
implemented in `codegen/src/execution.rs`; use them as the pattern. These
checks read the model and resolve nothing.

Typical limits: an event (`SINGLE`) task, more than one program instance, a
priority or interval wider than the runtime stores.

### 2. Compute the layout

The layout answers "where does this live, and how wide is it" for every
variable, field, type and POU. It is the backend's own, and the IR
deliberately carries none of it (REQ-LOW-lowering-027).

- **Compute it once, before any instruction is emitted, and never modify it
  during emission** (the bytecode backend's REQ-LOW-codegen-122).
- **Key it by `VarId`, `PouId`, `TypeId` and `FieldIdx`; no layout lookup
  takes a name** (REQ-LOW-codegen-123).
- **A `BOOL` is a type of its own.** Choose its storage (a byte, an `i1`, a bit
  of a packed word); it is not a 32-bit integer unless your layout says so.
- **Operation widths are not storage widths.** An `INT` is stored in 16 bits
  but computed at 32 ([ADR-0001](../adrs/0001-bytecode-integer-arithmetic-type-strategy.md));
  compute at the `ScalarType` the expression carries, whatever your target's
  native word is.
- **A reference and an interface value have no width in the IR.** Choose one
  (an address, an index, a pointer and a type id).
- **Size string storage from the capacity of each value**, never from other
  strings in the program (REQ-LOW-codegen-055).
- **Each instance has one storage for its fields**, which a body addresses in
  place through its instance parameter, so a write is visible at once and
  survives a trap
  ([ADR-0062](../adrs/0062-a-body-works-on-its-instances-fields-in-place.md);
  REQ-LOW-codegen-037, REQ-LOW-codegen-038).

### 3. Emit

Walk `init`, then the POUs. Every match over an IR enum names every variant.

- **Evaluation order**: operands, arguments and subscripts in the order the
  lowered program lists them (REQ-LOW-codegen-120).
- **Evaluate a place once per statement that names it once**
  (REQ-LOW-codegen-121): an `AssignBits` reads and writes its place, and must
  compute the place's address once.
- **Evaluate a `Case` selector once** (REQ-LOW-codegen-124).
- **`For`**: either use its explicit bounds (as the bytecode backend fuses its
  test), or call the IR's `For` expansion and implement only `Loop`
  (REQ-LOW-ir-068). Do not hand-write your own expansion.
- **Arguments**: `Arg::Copy` passes a copy, so the callee's writes never reach
  the caller; `Arg::Ref` passes the caller's place. If your target passes both
  as an address, the difference is whether you copy first — do not forget the
  copy.
- **`Intrinsic`**: map every variant to the target's implementation with one
  match that has no wildcard arm (the bytecode backend's
  REQ-LOW-codegen-074), so a new variant fails to compile until you handle
  it.
- **`Callee::Interface`**: dispatch however suits the target (a `switch`, a
  table of functions, `br_table`); a call through a null value traps
  (REQ-LOW-codegen-106).
- **`RoundTime`**: every evaluation in one round gives the value the host gave
  that round (REQ-LOW-codegen-096).

The design's table of how the bytecode VM, WebAssembly and LLVM each realize
a node is in [Backend Contract](../design/lowered-program.md#7-backend-contract).
For another target, write the same table in your design doc.

### 4. Build debug information

Build it from the lowered program alone: the names and spans on variables,
fields, POUs and types (REQ-LOW-lowering-006), and the span on every statement
for the line map (REQ-LOW-lowering-063). Read source bytes through
`ironplc_ir::source::SourceLookup`. A debugger shows what the backend allocated, so a variable an
optimization removed does not appear.

The end-to-end suite reads variables **by name** through the backend's debug
information, so a wrong name table fails the suite, not only the debugger
([End-to-End Test Observation](../design/end-to-end-test-observation.md#resolve-names-through-the-debug-section)).

## Meaning of Operations: Emit the Difference

Every lowered operation is total: for every operand it gives a stated result
or traps. Target instructions often are not. **Never map an operation to an
instruction whose result is undefined for an operand the lowered program can
supply.** Where the target's instruction differs from the rule, the backend
emits the difference; the lowered program does not change.

For each rule in
[Meaning of operations](../design/lowered-program.md#310-meaning-of-operations)
(REQ-LOW-codegen-083 to REQ-LOW-codegen-099 and REQ-LOW-codegen-164), write
down what your target's instruction does, and list every difference in your
design doc. The design already lists them for WebAssembly and LLVM; start
there. The hazards recur across targets:

- **Integer division**: a zero divisor, and the most negative value divided
  by -1.
- **Real to integer conversion**: a NaN, and a value out of range.
- **Shifts**: a count at or above the operand's width.
- **Signed overflow**: wraps in the IR. It is undefined in C, and LLVM's
  `nsw` and `nuw` flags assume it never happens, so a C backend computes on
  unsigned types and an LLVM backend omits the flags.
- **Floating point**: no fast-math; every comparison with a NaN is false
  except `<>`.
- **Null and bounds**: where address 0 is readable, or an out-of-bounds load
  is undefined, the checks are explicit branches.
- **Loops that never end**: a `FOR` whose bound is its control variable's
  largest value never ends (REQ-LOW-lowering-066), so never assume forward
  progress (LLVM's `mustprogress`).

A real function such as `SIN` uses the target's own primitive, so results may
differ from the bytecode VM's in the last bits. That is allowed; everything
else must be exact (REQ-LOW-codegen-132).

## Runtime Responsibilities

What the host and the generated code do at run time, on every target:

- **Rounds**: the program instances of a round run one at a time, each to
  completion. The lowered program assumes no preemption.
- **Round time**: the host passes each round's time in, in the unit
  `RoundTime` defines (today an unsigned 64-bit count of microseconds; see
  [Scalar expressions](../design/lowered-program.md#35-scalar-expressions)).
  The standard timers read it, so a backend without it cannot run one.
- **Traps**: a trap ends the round. No later statement or program instance in
  the round runs (REQ-LOW-codegen-097), every write made before the trap stays
  (REQ-LOW-codegen-038), and the trap is reported under the bytecode VM's code
  for it (REQ-LOW-codegen-098).
- **Watchdog**: `Loop` and `For` are the only repetition, and no call is
  recursive (REQ-LOW-lowering-059), so a backend that must bound a round checks
  a deadline at each iteration of a `Loop` or `For`.
- **Initialization**: `init` runs once, before the first round, and holds
  values only. A backend sets up its own storage before the statements of
  `init` run, such as a string's header or the data-region offset an
  aggregate's slot holds; that setup is layout, not part of `init`. A
  function's locals, and every body's `VAR_TEMP`, a program's included, are
  re-initialized by statements already at the head of its body, which also
  hold values only; the backend adds nothing to the values.
- **Location and retention**: a variable carries its direct address and its
  `RETAIN`/`PERSISTENT` qualifier. Decide in your design doc whether the
  target honors each, and say so in the user docs.

## Optimization

Optimizations that do not depend on the target belong in lowering, after the
lowered program is built, where every backend benefits
([Optimization](../design/lowered-program.md#10-optimization)). Only an
optimization that depends on the target's instructions belongs in the backend.

- **A backend produces a correct program from the unoptimized lowered program**
  (REQ-LOW-lowering-142). Never rely on a target-neutral pass having run.
- **A backend with an optimizer of its own, such as LLVM, takes the
  unoptimized program.** Its own passes do the same work with more
  information.

## Testing

Each leg asserts something the others do not.

| Leg | Asserts | Do **not** write |
|---|---|---|
| **End-to-end suite** (shared) | Run results by variable name, the same programs on every backend | A test that names a slot, an offset or an instruction |
| **Backend comparison** | The new backend leaves every variable equal to the bytecode backend's, reals within the helpers' tolerance (REQ-LOW-codegen-132) | A bit-for-bit comparison of a real |
| **Meaning of operations** | One program per rule of §3.10: the most negative `DINT` divided by -1, `REAL_TO_DINT` of a NaN, `SHL` by 33, each trap and its code | A copy of a case the end-to-end suite already runs |
| **Capability checks** | Each target-limit diagnostic and its code | A check analysis makes |
| **Structure** (backend crate) | Only what end-to-end cannot localize: layout, encoding, branch offsets | "The right instruction was emitted", which end-to-end covers |
| **Encoding guard** | If the artifact outlives a compile (a file another tool loads), the encoding of each instruction is pinned, as `codegen/tests/it/wire_format.rs` pins opcodes | — |

The dependency boundary and the crate-root lint are checked outside the
backend (see [What the crate may depend on](#what-the-crate-may-depend-on)), so
they are not among a backend's tests.

- **The end-to-end suite is the regression net.** Every behavior a backend
  implements is proven there, on every backend, by programs that read
  variables by name
  ([End-to-End Test Observation](../design/end-to-end-test-observation.md)).
  A disagreement between two backends is a defect in one of them, because
  both consumed the same lowered program.
- **A test the backend cannot run yet is marked, never silently skipped.**
  The mechanism is the first new backend's to design (see
  [Shared pieces](#shared-pieces-still-in-bytecode-crates)).
- **Run with target-neutral passes off and on**, so a backend never depends on
  one (REQ-LOW-lowering-142).
- **Every requirement in the backend's design doc has a conformance test** in
  the crate that owns it: list the doc in the crate's `build.rs` and annotate
  each test `#[spec_test(REQ_<AREA>_<crate_slug>_NNN)]`. The rules that apply
  to every backend (§3.10) are proven by the shared end-to-end programs, not
  restated as requirements of each backend.
- **Keep test files small and feature-focused** (see
  [Test File Organization](syntax-support-guide.md#test-file-organization-avoid-merge-conflicts)).

## Wiring the Backend In

- **Pipeline**: `ironplc_project::compile` lowers once, and passes the
  lowered program to the backend the user selected (REQ-LOW-project-005).
  `check` already runs lowering, so it reports nothing new for a target;
  target limits come from the backend at compile time.
- **Target selection**: document how a user selects the target in
  `docs/reference/compiler/ironplcc.rst`. The bytecode backend stays the
  default.
- **The playground**: runs bytecode only. A new backend does not plug into
  it.
- **Other front ends**: the language server and the MCP server compile to
  bytecode. Wire the new backend into one only when the target runs there.
- **Problem codes**: choose between P9997 (a fixed limit of the target's
  format), P9999 (not built yet) and P9998 (a broken invariant) as
  [problem-code-management.md](problem-code-management.md#choosing-between-p9997-and-p9999)
  defines them, and report one limit reached from several places with one
  code. A limit with a code of its own, as P4047 is for event tasks, is added
  and documented like any other problem code.

## When the IR Changes

Adding a statement kind, an expression kind, a projection, a `Callee`, an
`Arg` or an `Intrinsic` variant fails to compile in every backend until each
handles it. That is the point
([ADR-0061](../adrs/0061-one-name-per-built-in-operation-and-operand-type.md)).

- **Handle it in every backend in the same change.** Never add a wildcard arm
  to make a backend compile.
- **Or give it an expansion.** If the new node is an optimization some targets
  can use and others cannot, `ironplc-ir` provides its expansion into existing
  nodes, as it does for `For` (REQ-LOW-ir-068), and a backend with no use for
  the node calls the expansion. A vectorized operation would be added this way
  ([Vectorized operations](../design/lowered-program.md#vectorized-operations)).
- **Specify its meaning once.** Add the rule to
  [Meaning of operations](../design/lowered-program.md#310-meaning-of-operations),
  and a program for it to the shared end-to-end suite.
- **A node exists for the program's meaning, not for one target's
  instructions.** Every backend implements or expands each node (Goal 4 of the
  design); an optimization only one target's instructions can use lives in
  that backend.

## Common Mistakes

- **Reaching for the AST "just here"**: the backend depends on a forbidden
  crate, or a function lowering already exposes is reimplemented from source
  names. The dependency check outside the backend catches the first; the
  [table above](#never-decide-what-lowering-decided) catches the second.
- **Wildcard arms**: `_ => Err(Diagnostic::internal_error())` over an IR enum
  turns the next new variant into a run-time defect instead of a compile
  error.
- **Copying function block fields in and out around a body**, as the bytecode
  VM does until [issue 2120](https://github.com/ironplc/ironplc/issues/2120)
  is resolved. That is a limit of the VM, not the meaning: a body works on its
  instance's fields in place.
- **Giving every temporary string one size**, as the bytecode VM does until
  [issue 2118](https://github.com/ironplc/ironplc/issues/2118) is resolved.
  That is a limit of the VM, not the meaning: each value's storage is sized
  from its own capacity.
- **Mapping an operation onto undefined behavior**: LLVM's `sdiv` and
  `fptosi`, C's signed overflow, a shift by the full width. The result
  compiles and passes nominal tests, and the optimizer miscompiles the edge.
- **Reporting a problem with the program**: a backend that finds one has
  found a gap in analysis. Fix analysis, so the problem is reported there
  beside every other problem, in `check` and the language server.
- **Evaluating twice**: a `Case` selector compared once per label, or an
  `AssignBits` place addressed once for the read and again for the write.
  Neither shows without a side effect, which is why the end-to-end suite has
  programs with one (`end_to_end_evaluation_order.rs`).
- **Comparing reals exactly across backends**, or comparing anything else
  approximately.
- **Recognizing a standard function block**: there is no standard function
  block in the lowered program. A native fast path for `TON` reintroduces the
  second implementation ADR-0060 removed.

## Step-by-Step Walkthrough

### Example: Adding a WebAssembly Backend

#### Step 1: Design

Write `specs/design/wasm-backend.md`: the encoder and the host runtime, the
layout, how each node is realized, every difference between WebAssembly's
instructions and §3.10, the capability checks, and requirement IDs such as
`REQ-WASM-wasm-001`.

#### Step 2: Shared pieces

Implement the end-to-end harness's
`Backend` and `Running` traits, and add a way to mark a test a backend cannot
run yet; share the trap codes.

#### Step 3: The crate

Create `compiler/wasm` with the crate root above and an entry point that
reports every construct it does not handle yet as P9999 at that construct's
span.

#### Step 4: A minimal path

One `PROGRAM`, integer arithmetic and assignment, run under the end-to-end
harness and compared with the bytecode backend. Mark every other end-to-end
test as not yet supported by this backend.

#### Step 5: Grow

Add one node kind or one group of `Intrinsic` variants at a time, each
removing the marks from the end-to-end tests it makes pass, until none remain. Add the
programs for each §3.10 rule as the operation lands.

#### Step 6: Wire in

Select the backend in `ironplc_project::compile`, add the CLI option, and
document it and every new problem code.

#### Step 7: Run CI

```bash
cd compiler && just
```
