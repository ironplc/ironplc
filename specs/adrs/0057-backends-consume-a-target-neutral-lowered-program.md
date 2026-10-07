# Backends Consume a Target-Neutral Lowered Program

status: proposed
date: 2026-10-07

## Context and Problem Statement

Code generation compiles the analyzed `Library`. That is the tree the parser
built, annotated in place by the analyzer, and it serves three consumers with
different needs:

* **`plc2plc`** needs it faithful to the source.
* **The language server** needs it to hold a broken program.
* **Code generation** needs it total: every name resolved, every type known.

A type that can hold a broken program cannot also promise a resolved one, so
codegen checks at run time. It finds variables in maps keyed by name, keeps
an error path for each state analysis has ruled out, and needs wildcard match
arms for states that cannot occur.

A WebAssembly backend is planned, and an LLVM backend is possible. Whatever the
bytecode backend decides for itself today, each of them would decide again,
and two implementations of one decision can disagree. A program that behaves
differently on two targets is a safety defect in a PLC
([ADR-0005](0005-safety-first-design-principle.md)).

What should a backend consume?

## Decision Drivers

* **One implementation of each language decision**, upstream of every backend.
* **States that cannot occur cannot be written**, so a backend's matches are
  exhaustive without wildcard arms or error paths for them.
* **Target neutral:** nothing a backend consumes names a slot, an offset, an
  opcode or a function id of the bytecode VM.
* **Small:** each construct is one every backend must implement.
* **Delivered in behaviour-preserving steps**, with the end-to-end tests
  unchanged.
* **The `Library` stays source faithful** and able to hold a broken program.

## Considered Options

* A separate, target-neutral lowered program
* One tree indexed by phase
* Side tables keyed by node id
* A control-flow graph
* A tree in the bytecode VM's terms
* Decisions written into the AST, with no lowered program
* A validator in front of codegen

## Decision Outcome

Chosen option: "A separate, target-neutral lowered program", because it is the
only option that removes both the error paths and the repeated decisions from
every backend.

* **Lowering** turns a clean analysis into a **lowered program**: a program in
  IronPLC's intermediate representation, the **IR**. The pass lives in the
  crate `ironplc-lowering`, and the IR in `ironplc-ir`
  ([ADR-0059](0059-backends-depend-only-on-the-ir-crate.md)).
* **Everything is referred to by id**, never by name: `VarId`, `PouId`,
  `FieldIdx`, `LoopId` and `TypeId`. Each id is a newtype of its own, so one
  cannot be passed where another is expected.
* **A value is a scalar, a string or an aggregate.** Each class has its own
  representation, and an aggregate is only ever a place, so an addition of two
  strings or a comparison of two arrays cannot be written.
* **The IR is a tree, not a graph.** Its structured control flow (`If`,
  `Case`, `Loop`, `For`, `Exit`, `Continue`) maps directly onto the bytecode VM
  and onto WebAssembly. Building a graph from it for LLVM is mechanical.
* **Nodes are built only through constructors** that check the IR's
  invariants, so a backend relies on them without checking again.
* **Codegen moves onto the IR one top-level statement at a time.** A statement
  lowering cannot express yet is compiled from the AST, and the AST route is
  deleted once lowering covers everything.

The other decisions this design needs are separate ADRs: what the analyzer and
lowering each decide ([ADR-0058](0058-the-analyzer-decides-and-lowering-translates.md)),
the crate boundaries ([ADR-0059](0059-backends-depend-only-on-the-ir-crate.md)),
and what each operation means
([ADR-0060](0060-each-lowered-operation-has-one-total-meaning.md)).

### Consequences

* Good, because each language decision is made once, before any backend, so
  two backends cannot make it differently.
* Good, because a backend's matches over the IR are total, with no arm for a
  state analysis has ruled out.
* Good, because a second backend implements the IR's nodes, not the
  language's decisions.
* Good, because the `Library` is unchanged, so `plc2plc` and the language
  server keep what they need.
* Bad, because there are two new crates and a second tree to maintain.
* Bad, because while both routes exist, a statement can be compiled two ways,
  and the end-to-end tests compile each program both ways to keep them in
  agreement.
* Bad, because a source language with jumps must be structured by lowering
  before it fits a tree.

### Confirmation

The requirements of `specs/design/lowered-program.md` confirm this decision:

* A test over the crate manifests asserts that `ironplc-codegen` depends on
  neither the analyzer nor the AST (REQ-LOW-codegen-002).
* Clippy denies wildcard enum match arms in `ironplc-ir`, `ironplc-lowering`
  and every backend (REQ-LOW-lowering-082).
* Each constructor has a test that it refuses operands violating an invariant
  (REQ-LOW-ir-081).
* The end-to-end tests pass unchanged at every step, and while both routes
  exist they compile each program both ways and compare variable values by
  name (REQ-LOW-codegen-131).

## Pros and Cons of the Options

### A separate, target-neutral lowered program (chosen)

* Good, because every name is an id and every decision is a node, so a backend
  has nothing to resolve or decide.
* Good, because the IR can be written as text, so lowering's tests compare
  against readable output.
* Bad, because it is the largest change of the options.

### One tree indexed by phase

`Library<P: Phase>` with associated types, so a checked library has a
non-optional `expr_type` and no `LateBound`, as GHC ("Trees That Grow") and
Scala 3 do.

* Good, because it removes the leftovers of earlier phases.
* Bad, because it removes none of the name, call or typing work: the tree
  still refers by name.
* Bad, because it changes every type in the DSL crate, the derive macro, every
  rule and every transform.

### Side tables keyed by node id

The AST stays syntactic and analysis results live in maps beside it, as in
rustc's `TypeckResults` and RuSTy's annotation map.

* Good, because the AST needs no change.
* Bad, because a lookup in a map is partial, which is what this decision sets
  out to remove. rustc itself builds a typed tree (THIR) from its tables
  before it generates code.

### A control-flow graph

Typed instructions over basic blocks, as in rustc's MIR.

* Good, because LLVM takes a graph, and jumps need no structuring.
* Bad, because the bytecode VM and WebAssembly have structured control flow,
  so a graph would have to be structured again for them. Building a graph from
  a tree is the easy direction.

### A tree in the bytecode VM's terms

Places as slots and data-region offsets, callees as `func_id`s.

* Good, because it is simpler for one backend.
* Bad, because a second backend would make every decision again.

### Decisions written into the AST, with no lowered program

The analyzer already records conversions this way
([ADR-0056](0056-analyzer-records-implicit-conversions-in-the-ast.md)).

* Good, because the language server can show each decision.
* Bad, because the AST still refers by name, still holds broken programs and
  still carries source-only forms such as `ELSIF` and `REF=`, so every backend
  keeps the name, call and desugaring work and its error paths. This design
  keeps recording decisions in the AST
  ([ADR-0058](0058-the-analyzer-decides-and-lowering-translates.md)), but not
  as the only step.

### A validator in front of codegen

One pass that asserts the invariants, with accessors that unwrap.

* Good, because it puts the checks in one place.
* Bad, because every state stays representable, so the backend's matches stay
  partial.

## More Information

The design is `specs/design/lowered-program.md`.
