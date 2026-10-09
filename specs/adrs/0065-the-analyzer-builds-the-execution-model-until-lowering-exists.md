# The Analyzer Builds the Execution Model Until Lowering Exists

status: accepted
date: 2026-10-09

## Context and Problem Statement

Which `CONFIGURATION` is built, which `PROGRAM` instance runs under which
`TASK`, how a task is scheduled and which globals exist are decisions of the
language. Codegen's driver made all of them while it compiled, reading the
configuration's declarations from the library. It took the first configuration
in the toposorted library, which was the last one in the file, and ignored the
others; it bound the program to a task by matching type names; it synthesized
the uptime globals itself; and it rejected a negative `INTERVAL` as out of the
container's range, so `check` accepted it.

`specs/design/execution-model.md` moves these decisions out of codegen into an
execution model of plain data, held by the new crate `ironplc-ir`, which every
backend reads (ADR-0056: backends lower; they do not decide).

The lowered program design (pull request 2011) says who builds such data. Its
proposed ADR-0058 gives the decisions no valid program can get wrong to a
lowering stage, and its proposed ADR-0059 has lowering, in a crate of its own,
build everything in the IR crate, with the analyzer not depending on that
crate at all. Which instance runs under which task, how a task is scheduled
and which global goes where are decisions of that kind. But
`ironplc-lowering` does not exist, and nothing else would go in it yet.

Who builds the execution model now?

## Decision Drivers

* **Backends lower; they do not decide** what the language means (ADR-0056).
* **No crate without a reason to exist.** A crate that holds one function is
  cost without structure.
* **The departure from proposed ADR-0058 and ADR-0059 can be undone by moving
  code, not by redesigning it.**
* **`check` accepts libraries**: a library with no `PROGRAM`, or several, or a
  configuration kept apart from its programs, is valid on its own.

## Considered Options

* The analyzer builds the model, depending on `ironplc-ir`, until lowering
  exists
* Create `ironplc-lowering` now, to build the model
* Leave the decisions in codegen until lowering exists
* Keep the model's types in the analyzer until lowering exists

## Decision Outcome

Chosen option: "The analyzer builds the model, depending on `ironplc-ir`,
until lowering exists", because it moves the decisions out of codegen now,
creates no crate without a purpose, and puts the model's types where the
lowered program will hold them.

* **`execution_model::resolve`** in the analyzer builds an
  `ironplc_ir::execution::Execution` at the end of `stages::resolve_types`,
  next to `reachable()`, and the `SemanticContext` stores it. `CleanAnalysis`
  hands it to codegen.
* **`resolve` reads only what a clean analysis holds**: the library and the
  compiler options, never the analyzer's environments. When
  `ironplc-lowering` exists, the module moves into it unchanged and takes a
  `CleanAnalysis`, and the analyzer drops its dependency on `ironplc-ir`.
* **The analyzer reports nothing about the model.** A library that cannot be
  built into an executable resolves to `NotExecutable`, which codegen reports.
  The task parameters that make a program invalid (a negative `INTERVAL`, a
  `SINGLE` that names no `BOOL` global) are analyzer rules, which is where
  ADR-0058 puts them too, so they stay after the move.
* **Codegen reaches a declaration only by id.** It still compiles a program's
  body, and a declared global's type and initial value, from the library.
  `CleanAnalysis::program_declaration` and `global_declaration` answer them
  from the same walk that allocated the ids; they go away when the lowered
  program carries bodies, types and initial values.

### Consequences

* Good, because a second backend reads the same model and cannot choose a
  different configuration, task schedule or global order.
* Good, because the model's types do not change when lowering takes over:
  they are `ironplc-ir`'s from the start.
* Good, because `check` now reports a negative interval (P4078) and a
  `SINGLE` that is undeclared (P4007) or not `BOOL` (P4079), and codegen
  refuses several configurations (P4081) and an instance of a type that is not
  a `PROGRAM` (P4080) instead of quietly compiling something else.
* Bad, because until lowering exists the analyzer depends on `ironplc-ir`,
  contrary to proposed ADR-0059, and so does every crate that depends on the
  analyzer.
* Bad, because `CleanAnalysis` now hands out declaration nodes by model id,
  an interface that exists only until the lowered program carries bodies.

### Confirmation

`ironplc-ir`, the analyzer and codegen each list
`specs/design/execution-model.md` in their build scripts, so every
`REQ-EM-*` requirement has a conformance test in the crate that owns it.
`analyzer/src/execution_model.rs` names no analyzer environment, which a
reviewer checks when the module moves. Codegen's
`codegen_sources_when_scanned_then_read_no_configuration_declaration` fails if
codegen names a configuration declaration again. Every structured text file in
the repository compiles to the same container bytes as before the change, under
the default options, `--dialect rusty`, and the uptime and top-level global
flags.

## Pros and Cons of the Options

### The analyzer builds the model, depending on `ironplc-ir` (chosen)

* Good, because the decisions leave codegen now.
* Good, because the departure is one module and one dependency, both named
  here, to move.
* Bad, because it is a departure from proposed ADR-0059 for as long as
  lowering does not exist.

### Create `ironplc-lowering` now

* Good, because it follows proposed ADR-0058 and ADR-0059 from the start.
* Bad, because the crate would hold one function, and its shape would be
  decided before anything else that lowering does is designed.

### Leave the decisions in codegen until lowering exists

* Good, because nothing moves twice.
* Bad, because a second backend would have to make the decisions again, and
  `check` still could not report the ones that are errors in the program.

### Keep the model's types in the analyzer until lowering exists

* Good, because the analyzer needs no new dependency.
* Bad, because the types would move when lowering exists, and every backend
  that reads them would change with them.
