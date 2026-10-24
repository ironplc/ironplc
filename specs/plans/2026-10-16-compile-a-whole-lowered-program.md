# Plan: Compile a Whole Lowered Program

Tracking issue: #2344. It lists every PR below and stays open until they have
all landed.

## Goal

Move code generation from compiling one lowered statement at a time to
compiling a whole lowered program:

- lowering produces every reachable POU's body, its declarations and the
  initial values;
- the bytecode backend builds its layout once, from the lowered declarations,
  keyed by id;
- `ironplc-codegen`'s entry point takes the lowered program;
- the route that reads the AST is deleted.

The end state is the one [Lowered Program](../design/lowered-program.md)
fixes: §3.2 (program, POUs and variables), §7 (REQ-LOW-codegen-120 to 124),
§8 (existing structures) and "Delivery Constraints".

## Where main is

Measured on main at `5da7d74`. The hooks behind these numbers were temporary
and are not committed. The method is under
[Checking a refactor](#checking-a-refactor).

### Statements and bodies

`just lowered-count` reports **21,498** top-level statements lowered and
**38** from the AST. A hook in the route counter records, for each
`PreferLowered` compilation of the end-to-end suite, which body each statement
belongs to:

| Body | Compiled | Every statement lowered | With no statement |
|---|---|---|---|
| `PROGRAM` | 6,685 | 6,648 | 84 |
| `FUNCTION` | 833 | **833** | 1 |
| `FUNCTION_BLOCK` | 785 | **785** | 270 |
| `METHOD` | 118 | **118** | 0 |

- **6,648 of 6,685 compilations (99.4%) lower every statement of every
  body.** The other 37 programs have 39 statements that fall back. All of them
  are in a `PROGRAM` body, at the call site of something still being lowered
  elsewhere. By program:
  - 31 call a function with a string result, 24 of them `LREAL_TO_FMTSTR`
    (#2322 core 2, #2341);
  - 3 call a function with a structure result;
  - 1 has calls with outputs inside an expression (#2324, #2335);
  - 1 calls an inherited method (#2327, #2338);
  - and one test falls back on purpose
    (`compile_route::compile_when_prefer_lowered_then_unsupported_statement_from_ast`).
- **Every reachable declaration already lowers.** For every reachable POU of
  those 6,685 compilations, a probe built the body's frame and lowered the type
  of every declaration other than `VAR_EXTERNAL`. Of 8,421 bodies, none failed.
  The function block `VAR_TEMP` and `VAR_IN_OUT` that lowering does not express
  yet occur only in POUs no program instance reaches.
- **The rest of the workspace compiles 402 programs through
  `ironplc_codegen::compile`.** These come from `ironplc_project`, the CLI, MCP
  and the VM command line, which all use `Route::Ast`. A shadow compilation of
  each with `PreferLowered` lowered all 460 of their statements, with no
  fallback. Another 13 are tests that expect an error, and both routes give
  it.
- **Docs and examples.** No Rust test compiles them. 202 snippets were
  extracted, from every `.. playground::`, `playground-with-program` and
  `playground-link` in `docs/`, and `examples/*.st`, each with its dialect and
  allows:
  - 176 compile, and all 310 of their statements lower;
  - 25 are rejected by analysis, as written or without a library;
  - one fails on both routes.
- **Bundled libraries.** Every top-level statement of every POU of
  `Tc2_BuiltIns`, `Tc2_Math`, `Tc2_System` and `Tc2_Utilities` lowers through
  `Lowering::lower_statement`, 72 of 72, whether or not a program reaches the
  POU.

So the statement work is close to done, and nothing outside `PROGRAM` bodies
holds it up.

### Layout order

The AST route numbers variable-table slots in this order (`compile.rs`,
`compile_setup.rs`, `compile_fn.rs`, `compile_method.rs`):

1. the system globals, then the declared globals;
2. the program's variables other than `VAR_EXTERNAL`, in declaration order,
   `VAR_TEMP` included;
3. each reachable function, in library order:
   - `VAR_INPUT` and `VAR_IN_OUT`, in declaration order;
   - then `VAR`, `VAR_TEMP` and `VAR_OUTPUT`, in declaration order;
   - then the result;
4. each reachable function block, in library order: its copy slots, one per
   field, in field order, then its methods, laid out as functions are.

A function block's fields are numbered **inputs, then outputs, then locals**,
each group in declaration order. The data-region run of every instance uses
that numbering, and so do the per-type copy slots and `FB_LOAD_PARAM` field
indices. Data-region offsets follow the same walk, since each string, array,
structure and instance is reserved when its variable is laid out.

The lowered type of a function block has its fields in **declaration order**
(REQ-LOW-lowering-170), and includes a `VAR_IN_OUT`.

A probe checked each reachable POU of the 6,685 compilations:

- **Plain declaration order changes a slot in 14 compilations**, all of them
  function blocks that declare an output or a local before an input. The tests
  are in `compile_route_lowered_fb_field` (5), `end_to_end_adr` (4),
  `end_to_end_user_fb` (3) and `end_to_end_name_resolution` (2).
- **Declaration order grouped by section, as above, matches the AST layout in
  all 6,685.** Every program, function and method already declares its
  variables in group order in these tests. That makes the grouping a property
  of the AST layout, not of the programs.

`assert_routes_agree` (`codegen/tests/it/common/run.rs`) compares the two
routes slot by slot by index. It names a variable only in the failure message.
Any layout that changes an index breaks it, so the grouping is required, and
the comparison moves to names first (REQ-LOW-codegen-131).

One hazard turned up while reading the layout code.
`CompileContext::allocate_scratch_variable` numbers a scratch slot by the size
of the current name map. Inside a function's or a block's scope, that is not
the next free slot. The extraction keeps the numbering exactly, so containers
stay identical, and its PR records whether the slot can collide. If it can,
that is a new issue, not part of this plan.

### The byte-identical check

- **Codegen's suite is deterministic.** A hook in `compile_with_route_counts`
  wrote out every container the end-to-end suite compiles: 13,315, by both
  routes. Each file is named by the test thread's name and a per-thread
  sequence number. Two runs gave identical bytes for all of them.
- **So is the workspace.** `cargo test --workspace` compiles 13,709 containers,
  and two runs gave identical bytes. Two came from spawned CLI processes whose
  arguments hold a temporary path, so their names differed between runs; their
  contents did not. They are matched by content hash instead.

That is enough to call a refactor byte-identical. See
[Checking a refactor](#checking-a-refactor).

### Dependencies

Files of `codegen/src` that name each crate:

- `ironplc_dsl`: 60;
- `ironplc_analyzer`: 31;
- `ironplc_parser`: 11, for `CompilerOptions`;
- `ironplc_lowering`: 11.

`compile.rs` is 1,844 lines, over the 1,000-line limit.

## Decisions

### 1. This plan builds the whole program, not #2306

This plan owns:

- `lowered::Program`;
- `ProgramBuilder::finish`;
- `Lowering::lower_program`;
- `init`, and the statements at the head of each body.

They are on the path to deleting the AST route, and the bytecode backend
consumes them first, through PRs B1 to C3 below. They are designed so that
#2306's backend uses them unchanged:

- `Program` owns its data;
- `finish` refuses an incomplete program;
- `PartialProgram::into_program` reports every statement that does not lower
  as P9999 (REQ-LOW-lowering-110 to 112), which is #2306's Core 1 walk.

This replaces two of #2306's PRs:

- **Prefactor 4**, a whole `lowered::Program`, is B1 and B2;
- **Core 1**, `lower_program` and `init`, is C1 and C2.

#2306's Core 1 limits `init` to scalars, arrays, structures and strings.
C2 covers function block instances and references too, since the bytecode
backend needs them.

#2306 Prefactor 2, the boundary check outside every backend, overlaps with H3
below. If it lands first, H3 removes codegen's exemption from it. If not, H3
adds the REQ-LOW-codegen-002 check that it would have exempted codegen from.
D4 moves `SourceLookup` into `ironplc-ir`, which #2306 lists as a follow-up.
Prefactors 1 and 3 do not overlap.

### 2. The layout switches for the whole program at once; statements keep switching one at a time

The design says that "once a POU's statements all lower, the backend builds
its layout from the POU's lowered declarations". This plan switches the layout
**for the whole program in one step** instead, and leaves the route of each
statement as it is until `RequireLowered`.

Reasons:

- **The layout is global.** Variable-table indices run in one sequence across
  every POU. Data-region offsets are one sequence too. A function block's type
  id, field numbering and copy slots are laid out once and read by every caller
  in other POUs. Switching one POU means two layout builders agreeing on a
  shared numbering, with the result checked for every mix of switched and
  unswitched POUs.
- **The statement work is close.** Every function, function block and method
  body already lowers whole. The only bodies left are 37 `PROGRAM` bodies, and
  the open PRs above cover them. A per-POU switch would pay for two builders to
  serve 37 programs for a few weeks.
- **The whole-program switch does not wait for the statements.** The layout
  built from the lowered declarations also derives the name-keyed views that
  the AST statement compilers read. A statement that falls back still finds its
  storage. Every declaration of every reachable POU already lowers, so the
  layout can be built from lowered declarations for every program the suite
  compiles.
- **A safety net that costs little.** Until `RequireLowered`, a program with
  a declaration lowering cannot express keeps the layout built from the AST,
  whole, and the route counter counts it. While both builders exist, the
  test harness builds both layouts for every program and fails unless they are
  equal.

The design's Delivery Constraints change to say this (see
[Design changes](#design-changes)).

### 3. Declarations are lowered eagerly only in `lower_program`

`lower_program` declares every reachable POU's frame and variables before it
lowers any statement:

- in library order, the order the AST route compiles them;
- each POU's declarations in declaration order, with their sections and spans,
  locations and retention;
- the globals in the execution model's order;
- one instance variable for each program instance.

`lower_statement` and `lower_method_statement_of` keep allocating lazily, the
first time a statement reaches a variable.

Lowering's tests print variable ids (`v0^.f1`), and some index the variable
table (`outcome.variables[1]`). They go through `lower_statement`
(`lowering/src/test_support.rs`), so their text does not change. The other
sessions' statement PRs keep adding tests in that form. The alternative,
allocating a body's variables when its frame is first made, renumbers every
variable a test prints after the first, in every statement test.

`lower_program` reuses the lazy path's functions (`frame`, `declared_variable`,
`instance_fields`). It calls them for every declaration up front, so the
variable a statement reaches is the one already declared, and the lazy path
finds it in `declared`.

The statement entry stays public while codegen calls it, until D0, and then
becomes the crate-private entry lowering's tests use. It is deleted only if
those tests move to whole programs, which this plan does not require.

### 4. `init` and prologues are proved by values, not bytes

Instructions may change; values may not. Three checks prove it.

**Values after `init` alone, and after `init` and one scan, by name, on both
routes.** `assert_routes_agree` compares the variable table and the data
region today, after one scan. T1 below makes it:

- also compare the state left by `init`, before the first round;
- compare every variable by its name and owning function, not by its index
  (REQ-LOW-codegen-131);
- compare the data region by the string, array, structure and instance it
  holds, as the debug section names them.

Every end-to-end program is compiled by both routes, so every one of them
becomes a check of `init` and of the prologues. It runs from the PR that first
emits `init` from the lowered program (D2), with no new test needed for most
cases.

**Coverage of each kind of starting value.** These suites already exercise
each kind through `compile_to_run`. They become `init` checks unchanged:

| Kind | Existing tests | Added in C2 or D2 |
|---|---|---|
| Elementary, alias, enumeration, subrange | `end_to_end_initial_value_matrix` (542 compilations), `end_to_end_initial_values` | A zero that today's `init` omits, for each kind, so a lowered `init` that stores it still agrees |
| `STRING`, `WSTRING` | `end_to_end_string*`, `end_to_end_wstring` | A string field of a structure, and an array of strings |
| Structures, nested structures | `end_to_end_struct`, `end_to_end_constant_initializer_expressions` | A structure inside an array of structures |
| Arrays, arrays of structures | `end_to_end_array*`, `end_to_end_array_of_struct`, `end_to_end_array_of_struct_string` | — |
| Function block instances | `end_to_end_fb_*`, `end_to_end_user_fb` | An instance of a block that declares an instance of another block |
| An instance's own initializer, `t : MyFb := (x := 7)` | `end_to_end_fb_ton`, `end_to_end_fb_tof` (`TON := (PT := …)`) | The same for a user-defined block, and for a nested instance |
| References | `end_to_end_ref`, `end_to_end_reference_to`, `end_to_end_ref_to_array` | `NULL` and `REF(x)` in a structure field |
| A function's locals, result and `VAR_TEMP` | `end_to_end_call_semantics` ("locals start again") | A method's locals and result |
| A program's `VAR_TEMP` | none in the suite or the docs | A program whose `VAR_TEMP` is read before it is written in the second scan. This is the listed correction in D3 |

**Lowering's own tests.** C2 and C3 compare the text of `init` and of each
prologue, as lowering's other tests do.

**What may differ.**

- **Today's `init` omits a store that leaves storage cleared**
  ([Initial Values](../design/initial-values.md), §3). The bridge keeps that
  omission, so that most containers stay identical and the byte-identical
  check keeps catching other changes. A container whose `init` differs only in
  order is accepted when the value checks agree.
- **A program's `VAR_TEMP` is re-initialized at the head of every scan**, as
  the design says of every body, instead of once by `init`. This is a
  correction, listed in D3 with its test, and added to the design's list of
  corrections (see [Design changes](#design-changes)).

### 5. The layout path

The order recommended in the task, with step 3 split in two:

1. **A1 and A2.** #2052's steps 1 and 2, on the AST declarations, with
   byte-identical containers. A1 extracts the layout before emission, one PR
   per kind of POU. A2 keys it by scope and deletes `Scope` and `swap_scope`.
2. **A3.** The layout gains tables keyed by the lowered ids, while it is
   still built from the AST. It needs every declaration to have an id before
   emission, which C1 provides, and codegen to lower the whole program before
   emitting, which D0 provides. The ids are matched to their AST entries
   once, by owner and name, when the layout is built. After that, the bridge's
   lookups take an id.
3. **D1.** The layout is built from the lowered declarations. It walks the
   type table and the variables in the grouped order of
   [Layout order](#layout-order), so the containers stay byte-identical. It
   derives the name-keyed views for the AST statement compilers.

**What the layout describes for function block bodies while the VM copies
fields** (issue 2120 is out of scope):

- **Each function block type:**
  - its container type id (`UserFbDescriptor::type_id`);
  - the function id of its body;
  - the first of its copy slots (`var_offset`) and their number;
  - for each `FieldIdx`, its VM field index: inputs, then outputs, then
    locals. A `VAR_IN_OUT` or `VAR_TEMP` has none, since codegen does not
    support one in a reachable block yet.
- **Each instance:** its slot, and the start of its data-region run of
  `num_fields * 8` bytes.
- **Inside the body:** `this^.f` is the copy slot `var_offset + vm_index(f)`.
  The body and its methods share the copy slots, since the VM copies the
  instance in and out around a body and around each method.
- **A caller:** `inst.f` is `FB_LOAD_PARAM` or `FB_STORE_PARAM` of
  `vm_index(f)` on the instance's slot.
- **Each method:** the first slot of its parameters, the slots of its
  outputs, and whether it has a result.
- **A standard function block:**
  - its VM type id and its VM field numbering, from `standard_fb_layout`;
  - matched to the lowered fields by name, once, when the layout is built;
  - the one lookup by name that remains in layout construction, until #2288
    makes the standard blocks ordinary blocks.

  No lookup during emission takes a name (REQ-LOW-codegen-123).

### 6. The last steps

| Step | PR | Waits for |
|---|---|---|
| 1. Default `PreferLowered` | E1 | D1 to D4, so that one switch moves the shipped compiler to the lowered layout, `init`, prologues and debug information together. It does not wait for the statement work: a statement lowering does not support still falls back. |
| 2. Default `RequireLowered` | E2 | **The statement work**: no fallback left in the suite, the docs, the examples or the bundled libraries. That is #2341, #2335, #2338, the structure results, and function block calls with a string input or output (#2329). **The diagnostics work**: `RequireLowered` reports what the AST route would. Check the docs, the examples and the bundled libraries with the measurement above as well as the suite. |
| 3. The entry point takes `&lowered::Program` | F1 | E2 |
| 4. `ironplc_project` lowers once, in `check` | F2 | F1 |
| 5. Delete the AST compilers and `Route` | G1 to G4 | F2. Under `RequireLowered` nothing reaches them. |
| 6. Remove `program_declaration` and `global_declaration` | H1 | G. Lowering still reaches a program's declaration by `ProgramId` through them. The execution model moves into `ironplc-lowering` (ADR-0065), and the declarations behind each model id move with it, so the accessors go. |
| 7. Move `type_info.rs` and `string_width.rs` into lowering | H2 | G. Their last readers outside lowering are the AST compilers. |
| 8. Drop codegen's dependencies | H3 | H1 and H2 |

E2 is the only step that waits for other sessions' work. Every step before it
can start now, or as soon as its stage's own PRs land.

## Architecture

### The end state

```
CleanAnalysis ─▶ lowering::lower_program ─▶ lowered::Program ─▶ codegen::compile
   (check)          (check, once)              (owned)            layout::build ─▶ emit init, bodies ─▶ Container
```

- **`lowered::Program`**, in `ironplc-ir`:
  - the type table, variables, POUs with their bodies, and `init`;
  - the execution model, with a binding of each `ProgramId`, `GlobalId` and
    program instance to its `PouId` or `VarId`.

  It owns its data. `ProgramBuilder::finish` returns one only when every
  reachable POU has a body and every model entry is bound.
- **`Variable`** gains the section, location and retention it lacks.
  It already has a name, an owner, a type and a span (REQ-LOW-lowering-006,
  023, 024).
- **`Pou`** has:
  - its kind;
  - its instance parameter, parameters, result and locals;
  - its body;
  - its span.
- **`codegen::compile(program, options, sources)`** builds a `Layout` once,
  keyed by `VarId`, `PouId`, `TypeId` and `FieldIdx`. It then emits `init`
  and each body against `&Layout`, which emission cannot modify
  (REQ-LOW-codegen-122, 123).

### While both routes exist

- **`PartialProgram`** (lowering, C1) is what one `lower_program` walk makes:
  the builder's tables, `init`, each body's prologue, and the result of
  lowering each top-level statement of each body, by index.
  - `into_program()` gives the `Program`, or one P9999 for each statement that
    did not lower;
  - codegen reads the per-statement results directly (D0).
- **The AST compilers** still compile a statement that did not lower. They read
  the name-keyed views of the same `Layout`.
- **The two layout builders**, from the AST (A1 to A3) and from the lowered
  declarations (D1), both exist until G. A test-only check builds both for
  every program and fails unless they are equal.

## Prefactoring

The plan is mostly prefactoring. Every PR in stages A and B and T1 is
behaviour preserving, with byte-identical containers, and so are D0, A3 and
D1. The core changes are:

- C1 to C3, new lowering entry points nothing else calls yet;
- D2 and D3, which emit `init` and prologues from the lowered program;
- D4, which builds debug information from it;
- the route switches;
- the deletions.

The prefactor that makes the rest cheap is A0: accessors for the layout maps,
so that the later moves change no call site. See the
[conflict plan](#conflict-plan).

## Design doc reference

- [Lowered Program](../design/lowered-program.md): §1, §2, §3.1, §3.2, §7,
  §8, §9 and Delivery Constraints.
- [Initial Values Completed by the Analyzer](../design/initial-values.md).
- [The Resolved Execution Model](../design/execution-model.md), "What a
  backend still reads".
- [Debug Info in the IPLC Container](../design/debug-info-in-iplc-container.md).
- [End-to-End Test Observation](../design/end-to-end-test-observation.md).
- [Backend Guide](../steering/backend-guide.md).
- ADR-0024, ADR-0045 and ADR-0046 (initialization and the flat variable table);
  ADR-0057 to ADR-0065.

## Design changes

Each lands in the PR that makes it true, never here.

| Where | Change | PR |
|---|---|---|
| Lowered Program, Delivery Constraints, "Storage found by name at first" | The layout is built from the lowered declarations for the whole program in one step, not one POU at a time. It derives the name-keyed views the AST statement compilers read. A program with a declaration lowering cannot express keeps the AST layout, whole, until `RequireLowered`. | D1 |
| Delivery Constraints, "Unsupported is not a defect" | Lowering lowers the whole program in one walk (`lower_program`). While both routes exist, the walk keeps the result of each top-level statement, and codegen reads it rather than asking per statement. | D0 |
| §3.2 | The lowered program keeps the execution model unchanged and binds each `ProgramId`, `GlobalId` and program instance to a `PouId` or `VarId`. This settles "become, or are replaced by". | B2 |
| §3.2, §7 and Backend Guide "Runtime Responsibilities" | `init` and the prologues hold values only. A backend sets up its own storage before `init` runs, such as a string's header or the data-region offset an aggregate's slot holds. That setup is layout, not part of `init`. The guide's "the backend adds nothing" becomes "adds nothing to the values". | D2 |
| §7, the `Place` row | The bytecode VM numbers a block's fields inputs, then outputs, then locals, and the bytecode layout maps each `FieldIdx` to that number. | A3 |
| Delivery Constraints, "Behaviour preserving" | Add to the listed corrections: a program's `VAR_TEMP` starts again at the head of every scan. | D3 |
| Initial Values, REQ-IV-analyzer-050 | The `VAR_TEMP` of every body is flagged `reset_on_call`, a program's included, so lowering reads one flag. | D3 |
| Execution Model, "What a backend still reads" | Delete the section. Nothing reads a declaration by model id once the execution model moves into lowering. | H1 |
| ADR-0065 | Flip to `superseded` or record the move, as its own text asks, when the model moves into lowering. | H1 |

### Requirements

New IDs, in gaps of the design's numbering:

| ID | Claim | PR |
|---|---|---|
| **REQ-LOW-ir-029** | `ProgramBuilder::finish` returns a program only when every POU has a body and every program, global and program instance of the execution model is bound. | B2 |
| **REQ-LOW-lowering-177** | Every declaration of every reachable POU, and every global, has its variable or field in the lowered program, whether or not a statement names it, in declaration order within its POU. | C1 |
| **REQ-LOW-lowering-178** | `init` stores the completed initializer of every global and of every field of every program instance through the place that holds it, an instance's own initializer included. | C2 |
| **REQ-LOW-lowering-179** | The body of a function or method begins with statements that store the completed initializer of each declaration flagged `reset_on_call`, and then of its result. | C3 |
| **REQ-LOW-lowering-180** | The body of a program or function block begins with statements that store the starting value of each of its `VAR_TEMP`. | D3 |
| **REQ-LOW-codegen-125** | The bytecode backend's `init` function sets up the storage its layout gives (string headers, the data-region offsets that aggregate slots hold), then runs the statements of `init`, and stores nothing else. | D2 |

The lowered-program design is not listed in any crate's `build.rs` yet, so its
requirements are not yet checked by the spec conformance tests. Each PR names
the requirements it meets in its description and tests them, without the
`#[spec_test]` marker. Listing the design in `build.rs` would require a test
for every one of its requirements at once, and is its own PR, outside this
plan.

## Checking a refactor

### Byte-identical containers

Used by T1 and every PR of stage A, by D0 and by D1:

1. **T1 commits the dump to the test harness**, as `IRONPLC_ROUTE_COUNTS` is.
   - When `IRONPLC_CONTAINER_DUMP` names a directory, `compile_counting`
     writes each container there.
   - The file is named by the test thread's name, a per-thread sequence number
     and the route.
   - `just container-dump DIR` runs the codegen suite with it, with a fixed
     proptest seed.

   This covers 13,315 of the 13,709 containers the workspace compiles.
2. **Run it on main and on the branch, and compare the directories** with
   `diff -rq`.
3. **For a difference, `just container-diff A B`** names the containers that
   differ and prints the disassembly of each pair
   (`compiler/project/src/disassemble.rs`), side by side. It is an `#[ignore]`d
   test in `ironplc-project`, since `ironplc-codegen` cannot depend on the
   crate that depends on it.
4. **For the other 394 containers** (the CLI, `ironplc_project`, MCP and the
   VM command line), a PR that changes what `ironplc_codegen::compile` does
   before calling the shared driver runs the uncommitted hook #2078 used, in
   `compile`, as these measurements did. The containers of spawned processes
   are matched by content hash.

The PR description gives the number of containers compared and the number
identical. If a container differs, the PR lists each program with its before
and after behaviour, as #2078 did, or the PR is not a prefactor.

### Values

Used by D2 and D3, which change instructions on purpose: the value checks of
[decision 4](#4-init-and-prologues-are-proved-by-values-not-bytes), after
`init` and after one scan, by name, on both routes, for every end-to-end
program.

### The count

`just lowered-count` must not fall in any PR. D0 changes how statements reach
lowering, so its PR states the count before and after; they must be equal.

## Stages and PRs

PRs in one stage may land in any order unless a PR names one it waits for.
Every PR runs `cd compiler && just`, and `cd specs && just` when it changes
`specs/`. No PR changes `lowered_floor`.

### Stage T: the harness (now)

#### T1 (prefactor): Compare the routes by name, after `init` and after a scan, and dump containers

- [ ] `assert_routes_agree` also compares after `init`, before the first round
      (split `run_one_scan` into load and run)
- [ ] Compare each variable by name and owning function through `Variables`,
      not by index (REQ-LOW-codegen-131). Keep the whole-data-region
      comparison while both routes share one layout, and add a by-name
      comparison of each string layout entry
- [ ] `IRONPLC_CONTAINER_DUMP` in `compile_counting`, and the
      `container-dump` and `container-diff` recipes in `compiler/justfile`
- [ ] The `#[ignore]`d `container_diff` test in `ironplc-project`
- [ ] Show behaviour is unchanged: every end-to-end test passes unchanged, and
      a deliberately mismatched pair of containers (a test of the helper) fails
      by name
- Waits for: nothing

### Stage A: layout before emission (now)

The goal is #2052's: emission holds only `&Layout`, so it cannot change the
layout, and `Scope` and `swap_scope` are deleted. Every PR in stage A is
byte-identical.

#### A0 (prefactor, mechanical): Read the layout maps through accessors

- [ ] Give `CompileContext` one read accessor for each layout map:
  - `string_var`, `array_var`, `struct_var`, `struct_array_var`;
  - `fb_instance`, `is_in_out`, `user_function`, `user_fb_type`;
  - and the existing `var_index`, `var_type_info` and `in_out_ref_slot`.
- [ ] Replace the 166 direct reads in `codegen/src` with them, the bridge's 11
      included. Writes stay where they are
- [ ] No other change. Land it at a quiet moment (see the
      [conflict plan](#conflict-plan))
- Shown by: byte-identical containers
- Waits for: #2341, #2338 and #2335 to merge, or an agreed pause

#### A1 (prefactor, three PRs): Lay out every body before emitting any

A new module `codegen/src/layout/`, which splits `compile.rs` below 1,000
lines:

- `layout/mod.rs`: `Layout` and its accessors;
- `layout/globals.rs`, `layout/program.rs`, `layout/function.rs` and
  `layout/block.rs`: the walk;
- `layout/scratch.rs`.

The walk visits declarations in the AST route's order. `CompileContext` keeps
emission state only (§8, "`CompileContext`"), and holds `&Layout` and the
current body's scope by reference.

- [ ] **A1a: globals and the program.** Move `assign_system_globals` and
      `assign_variables`, with `debug_var_names` and `debug_string_layouts`,
      into the layout walk
- [ ] **A1b: functions.** Move the slot, string and return-value allocation of
      `compile_user_function` into the walk. Record what a call site needs
      (`UserFunctionInfo` without `max_stack_depth`). Move `max_stack_depth`,
      an emission result, to an emission-side map by `FunctionId`
- [ ] **A1c: function blocks and methods.** Move the block pre-scan of
      `compile_program_with_functions`, the copy slots of
      `compile_user_function_block` and the method slots of
      `compile_user_method` into the walk. Delete `swap_scope` and the
      program-scope hand-back
- [ ] Each PR states whether a scratch slot can collide (see
      [Layout order](#layout-order))
- Shown by: byte-identical containers; every end-to-end test unchanged
- Waits for: A0

#### A2 (prefactor): Key the layout by scope, and delete `Scope`

- [ ] Store each layout map once, keyed by the scope that declares the name
      (global, POU, or block and method) and the name
- [ ] Turn the visibility rules of `Scope::for_function_body` and
      `Scope::for_fb_body` into one lookup that states its scope. The rules
      stay as they are, for example "a global array is not visible in a
      function", because changing them changes behaviour
- [ ] Delete `scope.rs`. Move its tests to the lookup
- Shown by: byte-identical containers
- Waits for: A1c

### Stage B: the whole `lowered::Program` in the IR (now)

#### B1 (prefactor): A variable carries its section, location and retention

- [ ] Give `Section` the variants §3.2 lists and it lacks: `Global`,
      `Temporary` and `Instance`. Today it has the five a field needs. Every
      match over it names each variant, so each match gains the new arms
- [ ] Add `Section`, `Option<DirectAddress>` and `Retention` to
      `ir::lowered::Variable`. The span is already on its `DebugName`
- [ ] Add `ProgramBuilder::push_declared_variable(name, owner, section, ty,
      location, retention)`. Keep `push_variable` for temporaries (section
      `Temporary`), so no current caller changes
- [ ] Lowering's lazy path (`declared_variable`) passes the declaration's
      section, location and retention. Text tests do not change, since the
      printer prints no declaration
- [ ] Tests in `ir`: each field is kept; a variable of an owner that is not a
      POU is still refused
- Shown by: unchanged containers and IR text
- Requirements: REQ-LOW-lowering-006 (section), 023, 024
- Waits for: nothing

#### B2 (prefactor): Hold a whole lowered program

- [ ] `PouKind` (`Program`, `Function`, `FunctionBlock`, `Method { of }`) and
      the span on each POU entry
- [ ] `ProgramBuilder::declare_pou_parts(pou, instance, parameters, result,
      locals)`, `set_body(pou, Vec<Stmt>)` and `push_init(Stmt)`. The function
      and method signatures stay where `declare_function` and `declare_method`
      put them
- [ ] `ExecutionBindings`: `ProgramId` to `PouId`, `GlobalId` to `VarId`, and
      each program instance to the `VarId` of its instance
- [ ] `Program { types, variables, pous, init, execution, bindings }` and
      `ProgramBuilder::finish(model) -> Result<Program, Diagnostic>`, which
      refuses a POU with no body, and an unbound program, global or instance,
      as P9998 (REQ-LOW-ir-081, REQ-LOW-ir-029)
- [ ] The printer prints a whole program: types, variables with section and
      owner, each POU's parts and body, then `init`. Lowering's statement tests
      keep printing statements, so their text does not change
- [ ] Tests in `ir`: a hand-built program prints as expected; each refusal of
      `finish`
- Shown by: nothing in the compiler builds a `Program` yet
- Replaces: #2306 Prefactor 4
- Waits for: B1

### Stage C: lowering declarations and `init` (after B2)

#### C1 (core): Lower a whole program

- [ ] `Lowering::lower_program(self) -> Result<PartialProgram, Vec<Diagnostic>>`
      in `lowering/src/program.rs`:
  - [ ] allocate the `PouId` of each reachable POU (`SemanticContext::reachable`,
        REQ-LOW-lowering-025) in library order, each method after its block;
  - [ ] declare the globals in the execution model's order (the system globals
        first), then one instance variable of each program instance (section
        `Instance`, owner `Global`);
  - [ ] declare each POU's frame and every declaration, in declaration order,
        through `frame`, `instance_fields` and `declared_variable`
        (REQ-LOW-lowering-177);
  - [ ] lower each top-level statement of each body with the existing
        statement lowering, keeping a `StatementLowering` per index.
- [ ] `PartialProgram`:
  - `statement(pou, index)`;
  - `tables()`;
  - `into_program()`, which gives one P9999 at each unsupported statement's
    span and keeps walking, so every one is reported (REQ-LOW-lowering-110,
    111, 112).
- [ ] Bind the execution model: each `ProgramId` to its body, each `GlobalId`
      to its variable, each program instance to its instance variable
- [ ] A declaration lowering cannot express makes the program a
      `PartialProgram` with `declarations_complete() == false`, not an error.
      D1 reads it as "keep the AST layout"
- [ ] Tests in `lowering/src/program_tests.rs`:
  - a program, a function, a block and a method print as a whole program;
  - two unsupported statements in two POUs give two P9999;
  - an unreachable POU is absent;
  - an unused variable is present;
  - a block's fields follow declaration order.
- Requirements: REQ-LOW-lowering-001, 011, 025, 170, 177, 110 to 112
- Replaces: #2306 Core 1 (the walk)
- Waits for: B2

#### C2 (core): Lower `init` from the completed initializers

- [ ] `lowering/src/init.rs`:
  - read each declaration's completed initializer
    ([Initial Values](../design/initial-values.md)). Codegen already
    completes a copy of the library with `xform_resolve_initial_values::apply`
    and hands lowering an analysis of that copy. The entry `ironplc_project`
    calls from F2 completes the copy itself, so the completion runs once per
    compilation either way;
  - translate each completed initializer into statements through the place
    that holds it: `Assign`, `AssignStr`, or a field-by-field and
    element-by-element walk of a structure, array or function block instance.
- [ ] Order: the globals, then each program instance's fields, each in
      declaration order, as today's `emit_initial_values` visits them
- [ ] Cover what codegen's `init` covers:
  - scalars, enumerations and subranges;
  - strings;
  - arrays, structures and arrays of structures;
  - function block instances of user and standard blocks, with their own
    initializers and their member expressions (REQ-IV-codegen-041);
  - `NULL` and `REF(x)`.

  A kind that lowering does not express yet leaves `init` incomplete
  (`PartialProgram::init() == None`), not an error, and the whole `init` falls
  back in D2.
- [ ] Tests compare `init`'s text for each kind in the table of
      [decision 4](#4-init-and-prologues-are-proved-by-values-not-bytes)
- Requirements: REQ-LOW-lowering-178
- Replaces: #2306 Core 1 (`init`)
- Waits for: C1

#### C3 (core): Lower the statements at the head of each body

- [ ] For a function or method: one statement storing the completed
      initializer of each declaration flagged `reset_on_call`, and of the
      result, in declaration order, the result last, as
      `emit_function_local_prologue` stores them
- [ ] A `VAR_TEMP` of a function or method is flagged already, and is
      covered. A program's is not flagged until D3 changes the analyzer, and
      C3 reads only the flag, so a program's prologue stays empty until D3
      (REQ-LOW-lowering-180)
- [ ] `PartialProgram::prologue(pou) -> Option<&[Stmt]>`, `None` when one
      starting value does not lower
- [ ] Tests compare each prologue's text
- Requirements: REQ-LOW-lowering-179
- Waits for: C1

### Stage D: codegen consumes the lowered program

Outlined; each PR is detailed in its own description.

- **D0 (prefactor): One lowering walk per compilation.**
  `compile_with_route_counts` calls `lower_program` once, before emission.
  `try_compile_statement` reads the statement's result from the
  `PartialProgram`, by the body's `PouId` and the statement's index, instead of
  calling `lower_statement_of`. `Lowering::lower_statement_of` and
  `lower_method_statement_of` are deleted.
  - Shown by: byte-identical containers on both routes, and an unchanged
    `just lowered-count`.
  - Waits for: C1.
- **A3 (prefactor): Key the layout by lowered ids, while it is still built
  from the AST.** Once the AST layout is built, match each lowered variable,
  POU, type and field to its entry, by owner and name, in one function. Add
  tables keyed by `VarId`, `PouId`, `TypeId` and `(TypeId, FieldIdx)`, with
  the function block contents of
  [decision 5](#5-the-layout-path). The bridge's lookups
  (`place::resolve`, `storage_name`, `variable_name`, `user_function`,
  `method_site`, `reaches_block`) take ids, and `is_current_body` compares
  `PouId`s.
  - Shown by: byte-identical containers.
  - Waits for: A2 and D0.
- **D1 (prefactor): Build the layout from the lowered declarations.** A second
  builder walks the lowered type table and variables in the grouped order. It
  computes slots, data-region offsets, descriptors and debug entries from
  `TypeKind`, and fills the same `Layout`, its name-keyed views included. A
  program whose declarations are not complete keeps the AST builder.
  - A test-only check builds both layouts for every program the suite
    compiles, and fails unless they are equal.
  - Where they differ, lowering records what was missing, such as the debug
    type name of an alias or an array spelled in place. The layout never reads
    the AST to make up for it.
  - It may split by kind of storage (scalars, strings, arrays, structures,
    instances, references), the AST builder covering the rest.
  - Shown by: byte-identical containers, and the equal-layouts check.
  - Waits for: A3.
- **D2 (core): Emit `init` from the lowered program.** The storage setup
  (REQ-LOW-codegen-125), then the statements of `init`, through the bridge.
  The bridge keeps today's omission of a store that leaves storage cleared.
  An `init` that is not complete, or that the bridge cannot emit whole, falls
  back to today's `init`, whole, and is counted on a line of its own,
  separate from the statement count.
  - Shown by: the value checks of T1 for every end-to-end program, and the
    new programs of decision 4.
  - Waits for: C2, D1 and T1.
- **D3 (core): Emit each body's prologue from the lowered program, and
  re-initialize a program's `VAR_TEMP`.** The analyzer flags every body's
  `VAR_TEMP` (REQ-IV-analyzer-050 amended), and the bridge emits each
  prologue in place of `emit_function_local_prologue`.
  - This is the listed correction: a program's `VAR_TEMP` starts again each
    scan, with its test (REQ-LOW-lowering-180).
  - A method without a result still has a result slot, which today's
    prologue clears. No lowered variable holds it, so the bridge clears it as
    the backend's own setup, as REQ-LOW-codegen-125 allows for `init`.
  - Shown by: the value checks, `end_to_end_call_semantics`, and the new
    program `VAR_TEMP` test.
  - Waits for: C3, D1 and T1.
- **D4 (core): Build debug information from the lowered program.**
  - The variable and function name tables come from the lowered names.
  - `register_pou_source_file` takes the POU's span.
  - `record_statement_position` takes the lowered statement's span
    (REQ-LOW-codegen-007, REQ-LOW-lowering-006, REQ-LOW-lowering-063).
  - `SourceLookup` moves to `ironplc-ir`, as the Backend Guide asks.
  - Shown by: byte-identical debug sections where the spans are equal, and
    the line map checked by what it covers, as §1 allows.
  - Waits for: D1.

### Stages E to H: the switch and the deletion

Outlined.

- **E1:** the default route becomes `PreferLowered`. One line, and so is
  undoing it.
  - Waits for: D1 to D4.
- **E2:** the default becomes `RequireLowered`.
  - First rerun the measurement of [Where main is](#where-main-is): the suite,
    the 402 other workspace compilations, the docs and examples snippets, and
    every POU of every bundled library. Each must have no fallback.
  - Add a test in `ironplc-project`, which loads the bundled libraries, that
    lowers every statement of every POU of each one, whether or not a program
    reaches it. A library change then cannot bring a fallback back.
  - No Rust test compiles the docs snippets. The PR runs the extraction once
    and reports the result, and a test that compiles them becomes an issue.
  - Waits for: the statement work and the diagnostics work
    ([decision 6](#6-the-last-steps)).
- **F1:** `ironplc_codegen::compile(program: &lowered::Program, options,
  sources)` (REQ-LOW-codegen-003). The strings' policy moves out of
  `CodegenOptions`: `Intrinsic` already encodes it (REQ-LOW-lowering-073).
  - A wrapper keeps the old signature for callers not yet moved: it lowers,
    then calls the new entry. Its callers are `lsp_runner.rs`, the benchmarks,
    the VM command line's tests and codegen's own tests.
  - Waits for: E2.
- **F2:** `ironplc_project`'s `semantic()` runs lowering after a clean
  analysis, reports its diagnostics in `check` (REQ-LOW-project-004), and
  caches the program. `compile` passes that program on (REQ-LOW-project-005).
  The wrapper of F1 goes.
  - Waits for: F1.
- **G1 to G4:** delete the AST statement compilers, in four PRs:
  - expressions;
  - statements and calls;
  - the AST layout builder;
  - initial values (`initial_value.rs`, `compile_initial_value.rs`,
    `compile_struct_init.rs`, `compile_fb_init.rs`) and `Route`.

  Then delete the equal-layouts check, which has one builder left to check.
  - Waits for: F2.
- **H1:** move `execution_model` into `ironplc-lowering` (ADR-0065). Remove
  `CleanAnalysis::program_declaration` and `global_declaration`, with the
  design changes listed for H1.
  - Waits for: G.
- **H2:** move what lowering still needs from `type_info.rs` and
  `string_width.rs` into lowering, and delete the rest (§8).
  - Waits for: G.
- **H3:** remove `ironplc-dsl`, `ironplc-analyzer`, `ironplc-parser` and
  `ironplc-lowering` from codegen's `[dependencies]` (REQ-LOW-codegen-002).
  Remove codegen's exemption from #2306's boundary check if it landed, or add
  the check.
  - Waits for: H1 and H2.

## File map

| File | Change | PRs |
|---|---|---|
| `compiler/codegen/tests/it/common/run.rs`, `common/variables.rs` | Compare by name, after `init` and after a scan; container dump | T1 |
| `compiler/project/tests/container_diff.rs` (new) | The `#[ignore]`d disassembly diff | T1 |
| `compiler/justfile` | `container-dump`, `container-diff` | T1 |
| `compiler/codegen/src/compile.rs` | Accessors (A0); split into `layout/` and emission (A1); one lowering walk (D0); new entry (F1) | A0, A1, D0, F1 |
| `compiler/codegen/src/layout/{mod,globals,program,function,block,scratch}.rs` (new) | The layout walk from the AST | A1, A2 |
| `compiler/codegen/src/layout/ids.rs` (new) | The tables keyed by lowered ids | A3 |
| `compiler/codegen/src/layout/lowered.rs` (new) | The builder from lowered declarations | D1 |
| `compiler/codegen/src/compile_setup.rs`, `compile_fn.rs`, `compile_method.rs`, `data_region.rs`; the allocation parts of `compile_struct.rs`, `compile_array.rs`, `compile_array_struct.rs`, `compile_reference.rs` | Allocation moves into `layout/` | A1 |
| `compiler/codegen/src/scope.rs` | Deleted | A2 |
| 20 files of `compiler/codegen/src`, among them `compile_expr.rs`, `compile_stmt.rs`, `compile_place.rs`, `compile_string*.rs`, `compile_call*.rs` and `lowered/*.rs` | Reads through accessors | A0 |
| `compiler/codegen/src/lowered/{mod,place,fn_call,method,fb_call,aggregate,string}.rs` | Statement results from the `PartialProgram` (D0); lookups by id (A3); `init` and prologues (D2, D3) | D0, A3, D2, D3 |
| `compiler/codegen/src/lowered/init.rs` (new) | Storage setup and `init` emission | D2 |
| `compiler/codegen/src/source_lookup.rs` | Moves to `compiler/ir/src/source_lookup.rs` | D4 |
| `compiler/ir/src/lowered/variables.rs` | `Section`, location, retention; POU kind, parts, body, span | B1, B2 |
| `compiler/ir/src/lowered/program.rs` | `push_declared_variable`, `set_body`, `push_init`, `finish`, `Program` | B1, B2 |
| `compiler/ir/src/lowered/bindings.rs` (new) | `ExecutionBindings` | B2 |
| `compiler/ir/src/lowered/print.rs`, `print_program.rs` (new), `tests/` | The whole-program printer and its tests | B2 |
| `compiler/lowering/src/program.rs`, `program_tests.rs` (new) | `lower_program`, `PartialProgram` | C1 |
| `compiler/lowering/src/init.rs`, `init_tests.rs` (new) | `init` | C2 |
| `compiler/lowering/src/prologue.rs`, `prologue_tests.rs` (new) | Prologues | C3 |
| `compiler/lowering/src/lib.rs`, `variables.rs` | Section, location and retention on the lazy path (B1); statement entry private (D0) | B1, D0 |
| `compiler/analyzer/src/xform_resolve_initial_values.rs` | Flag every `VAR_TEMP` | D3 |
| `compiler/project/src/compile.rs`, `project.rs` | Lower in `check`, pass the program on | F2 |
| `compiler/ironplc-cli/src/lsp_runner.rs`, `compiler/benchmarks/src/lib.rs` | Call the new entry | F1, F2 |
| `compiler/codegen/Cargo.toml` | Drop the dependencies | H3 |
| `specs/design/lowered-program.md`, `initial-values.md`, `execution-model.md`; `specs/steering/backend-guide.md`; `specs/adrs/0065-…` | The [design changes](#design-changes) | as listed |

## Conflict plan

#2052 warns that the layout work touches most of codegen. The bridge reads
the layout maps directly 11 times and through an accessor once. Three bridge
PRs are open: #2341, #2338 and #2335.

- **One mechanical PR takes the churn, at a quiet moment.** A0 replaces every
  direct read of a layout map (166 across `codegen/src`) with an accessor whose
  name says what it answers. It changes nothing else, so an open PR rebases
  onto it by renaming. It lands when the three bridge PRs have merged, or at a
  pause agreed with their sessions. After A0, A1 to A3 and D1 change only the
  layout walk and the bodies of the accessors, and no call site in
  `compile_*.rs` or `lowered/*.rs`.
- **The bridge's helpers keep their names and signatures through A3.**
  `place::resolve`, `resolve_load`, `storage_name`, `variable_name`,
  `user_function`, `method::method_site` and `fb_call::reaches_block` keep
  their call shape, and only their bodies switch from names to ids. A new
  bridge PR calls them as it does today.
- **Lowering's statement tests do not churn.** Eager declaration happens only
  in `lower_program` (decision 3). `lower_statement` keeps its lazy
  allocation, so the IR text and the variable indices other sessions' tests
  compare against do not change. B1 adds `push_declared_variable` beside
  `push_variable` rather than changing it.
- **D0 changes how codegen reaches lowering, not what lowering does.** A
  statement PR that lands after D0 adds lowering as before, and the bridge
  finds the result in the `PartialProgram`.
- **Stage A waits for nothing in lowering, and stage B and C touch only `ir`
  and `lowering`.** So stages A and B can proceed in parallel without touching
  the same files. C1 adds files rather than editing the statement modules.
- **Other open PRs that touch the files A1 moves.**
  - #2187 (alias return types): `compile_fn.rs`, `compile_method.rs` and
    `type_info.rs`.
  - #1954 (arrays of arrays): `compile_array.rs`, `compile_array_struct.rs`,
    `compile_struct.rs` and `compile_struct_init.rs`.
  - #1938 (resource globals): `compile.rs`, written against an older version
    of it.

  Whichever lands second rebases. If one of them is about to merge, A1 waits
  for it. #1972 touches only the analyzer, and #2018 only emission.
- **#2098 (every scalar access through `Place`) and #2050.** #2098 rewrites the
  same reads as A0. Doing A0 first gives #2098 accessors to call. If #2098 goes
  first, A0 is smaller. Either order works, but not both at once. #2050 is
  mostly analyzer work, and its codegen half touches conversions, not the
  layout.
- **Deletions in G land one area at a time**, each after the last statement PR
  touching that area.

## Out of scope

- Lowering the remaining statements, and the diagnostics parity work.
- Function block fields in place (issue 2120), standard function blocks
  generated in lowering (#2288), and the WebAssembly backend itself (#2306).
- Function block call binding.
- Optimization passes.
- Listing the lowered-program design in a `build.rs`.
- `RETAIN` and `PERSISTENT` semantics: the lowered program carries them, and no
  backend honours them yet.
