# Design: The Resolved Execution Model

status: implemented
date: 2026-10-09

## Overview

An IEC 61131-3 library says what runs, and when, through its `CONFIGURATION`:
the resources it declares, the `TASK`s each resource schedules, the `PROGRAM`
instances bound to those tasks, and the `VAR_GLOBAL`s they share (see
[Task Support](61131-task-support.md)). Which configuration is built, which
program runs under which task, what a task's parameters mean, and which globals
exist are language decisions.

Codegen's driver used to make these decisions while it compiled, reading
`ConfigurationDeclaration`, `ResourceDeclaration`, `TaskConfiguration` and
`ProgramConfiguration` nodes from the library. This design moves them out of
codegen, and makes them the first part of the
[Lowered Program](lowered-program.md) (proposed in pull request 2011 and not
yet merged):

- **`ironplc-ir`**, a new crate, holds the **execution model**: what runs and
  when, as plain data.
- **The analyzer** builds it, in `execution_model::resolve`, and stores it on
  the `SemanticContext`, next to `reachable()`. It also checks the task
  parameters that can make a program invalid, as rules: a negative
  `INTERVAL`, and a `SINGLE` that names no `BOOL` global.
- **Codegen** takes the model through `CleanAnalysis`, builds the task table,
  the program instance and the global variables from it, and never reads the
  configuration's declarations.

A second backend reads the same model, so it cannot decide differently. This
follows ADR-0056: backends lower; they do not decide what the language means.

## Crates

| Crate | Holds | Depends on |
|---|---|---|
| `ironplc-ir` (new) | The execution model's types, in a module `execution` | `ironplc-dsl`, for `SourceSpan` alone (see [Source positions](#source-positions)); `time` |
| `ironplc-analyzer` | `execution_model::resolve`, which builds the model; the two task rules | `ironplc-ir`, for now (see below), and its dependencies today |
| `ironplc-codegen` | The capability checks; the task table and globals, built from the model | `ironplc-ir`; and `ironplc-analyzer` as today, while it compiles bodies from the library |

### A temporary departure from the lowered program's crates

The lowered program design has a lowering stage, in a crate of its own, build
everything in `ironplc-ir`, and the analyzer not depend on `ironplc-ir` at all
(proposed ADR-0059). It also gives lowering the decisions no valid program can
get wrong (proposed ADR-0058), and which instance runs under which task, how a
task is scheduled and which global goes where are decisions of that kind.

This design departs from both, on purpose and for now (ADR-0065). `ironplc-lowering` does
not exist yet, and creating it to hold one function would be a crate with no
other reason to exist. So the analyzer depends on `ironplc-ir` and builds the
model itself.

The departure is kept small enough to undo by moving one module:

- **`resolve` reads only what a clean analysis holds**: the library, the
  type environment and the compiler options, never the analyzer's internals. When `ironplc-lowering`
  exists, `execution_model` moves into it unchanged and takes a
  `CleanAnalysis`, and the analyzer drops its dependency on `ironplc-ir`.
- **The model's types do not change** when it moves. They are `ironplc-ir`'s
  from the start.
- **The analyzer reports nothing about the model.** A library that cannot be
  built into an executable is a valid library, so `resolve` returns that as
  data (`NotExecutable`), not as a diagnostic, as lowering would
  (REQ-LOW-lowering-109). The rules that can make a program invalid are the
  analyzer's under ADR-0058 too, and stay there after the move.

## Representation

This section is the shape of the data a backend receives. It follows the
identity rules of the lowered program (§3.1): nothing is referred to by its
source name, and a name is kept only to label a diagnostic or debug
information.

The model is built to two rules:

- **No source identity is a key.** The model holds no `ConfigurationDeclaration`,
  `ResourceDeclaration`, `TaskConfiguration`, `ProgramConfiguration`, `VarDecl`
  or other declaration node, and no `Id` or `TypeId` that a backend looks
  anything up by. A backend that has the model has every decision already
  made, and nothing to make it again from.
- **A relationship is ownership where it can be, and an id the model allocated
  where it cannot.** A backend never resolves a name, and never searches a
  table for an entry that might be missing.

### The types

```rust
// ironplc-ir, module `execution`

/// What `resolve` built: a model a backend can build, or why it cannot.
pub enum Execution {
    Executable(ExecutionModel),
    NotExecutable(NotExecutable),
}

/// The fields are private, so `ExecutionModelBuilder` is the only way to
/// make one.
pub struct ExecutionModel {
    configuration: Configuration,         // declared, or the implicit one
    programs: Vec<ProgramType>,           // indexed by ProgramId
    globals: Vec<Global>,                 // indexed by GlobalId: variable-table order
}
impl ExecutionModel {
    pub fn configuration(&self) -> &Configuration;
    pub fn program(&self, id: ProgramId) -> Option<&ProgramType>; // None: another model's id
    pub fn global(&self, id: GlobalId) -> Option<&Global>;        // None: another model's id
    pub fn programs(&self) -> impl Iterator<Item = (ProgramId, &ProgramType)>; // in added order
    pub fn globals(&self) -> impl Iterator<Item = (GlobalId, &Global)>;        // variable-table order
}

pub struct Configuration {
    pub name: Option<DebugName>,          // None: the implicit configuration
    pub resources: Vec<Resource>,         // declaration order; implicit one last
}

pub struct Resource {
    pub name: Option<DebugName>,          // None: the implicit resource
    pub tasks: Vec<Task>,                 // declared tasks, then the implicit one
}

pub struct Task {
    pub name: Option<DebugName>,          // None: an implicit task
    pub priority: u32,                    // as declared
    pub schedule: Schedule,
    pub instances: Vec<ProgramInstance>,  // the instances this task runs, in order
}

pub enum Schedule {
    Cyclic { interval: time::Duration },                          // positive INTERVAL, no SINGLE
    Freewheeling,                                                 // zero or no INTERVAL, no SINGLE
    Event { trigger: Trigger, interval: Option<time::Duration> }, // SINGLE
}

pub enum Trigger {
    Global(GlobalId),                     // SINGLE := <global>
    Constant,                             // SINGLE := <constant>
}

pub struct ProgramInstance {
    pub name: Option<DebugName>,          // None: the implicit instance
    pub program: ProgramId,
}

pub struct ProgramType {
    pub name: DebugName,
}

pub struct Global {
    pub name: DebugName,
    pub kind: GlobalKind,
}

pub enum GlobalKind {
    System(SystemGlobal),                 // provided by the compiler
    Declared(GlobalScope),                // declared in the source
}

pub enum SystemGlobal { UpTime, UpLTime }

pub enum GlobalScope { TopLevel, Configuration, Resource }

/// Allocated by `ExecutionModelBuilder`; valid only for the model it builds.
pub struct ProgramId(usize);              // private field: a position
pub struct GlobalId(usize);               // private field: a position

/// The only way to make a model. Adding a program or a global returns its id,
/// so every id names an entry the model holds.
pub struct ExecutionModelBuilder { /* private */ }
impl ExecutionModelBuilder {
    pub fn add_program(&mut self, program: ProgramType) -> ProgramId;
    pub fn add_global(&mut self, global: Global) -> GlobalId;
    pub fn build(self, configuration: Configuration) -> ExecutionModel;
}

/// A source name, for a diagnostic or debug information. It is not `Eq`,
/// `Hash` or `Ord`, so it cannot be compared or used as a key.
pub struct DebugName { text: String, span: SourceSpan }
impl DebugName {
    pub fn new(text: impl Into<String>, span: SourceSpan) -> Self;
    pub fn span(&self) -> SourceSpan;
}
impl Display for DebugName { /* the name as written */ }

pub enum NotExecutable {
    NoProgram,
    SeveralConfigurations(Vec<DebugName>),
    SeveralPrograms(Vec<DebugName>),
    UndeclaredPrograms(Vec<DebugName>),
}
```

### How each relationship is held

| Relationship | Held by | Why it cannot be wrong |
|---|---|---|
| A resource belongs to the configuration | `Configuration::resources` | Ownership; there is always exactly one configuration |
| A task belongs to a resource | `Resource::tasks` | Ownership |
| An instance runs under a task | `Task::instances` | Ownership: an instance cannot name a task of another resource, or none |
| An instance instantiates a program | `ProgramId` | Returned by the builder when the program is added; `resolve` adds only declared `PROGRAM`s, and an instance of anything else makes the library `NotExecutable` |
| A task is triggered by a global | `GlobalId` | Returned by the builder when the global is added; an undeclared one is an analysis error |
| A global's place in the variable table | Its position in `globals` | The position is the id |

The two ids are positions in vectors the model owns, never keys into a map.
Their fields are private to `ironplc-ir`, and `ExecutionModelBuilder` is the
only way to make one, so an id always names an entry of the model it came
from. `ExecutionModel` answers `program(ProgramId)` and
`global(GlobalId)` from its own vectors. The only way to miss is to ask one
model about an id another model allocated, which is a compiler defect and is
reported as P9998. A backend that keys its layout by `GlobalId` needs no
lookup at all (REQ-LOW-codegen-123 asks the same of the lowered program).

`instance → task` is the one relationship that has to be either ownership or
an id, and `instance → program` the other. They cannot both be ownership: an
instance has one task and one program type, and several instances share both.
Ownership goes to scheduling, which is what this model is for.

### Decisions in the representation

- **The schedule is one value.** `Schedule` carries the data each kind needs,
  so a cyclic task without an interval, or an event task without a trigger,
  cannot be written. The analyzer decides the kind ([Task kind](#task-kind));
  no backend reads `INTERVAL` and `SINGLE` to decide it again.
- **An executable model and a reason are two types.** A backend matches
  `Execution` once. Inside `ExecutionModel` every instance has a program
  declared as a `PROGRAM`, every task has a schedule and every trigger names
  a global, with no `Option` to handle.
- **Implicit objects are present, not inferred.** A library with no
  `CONFIGURATION` gets an implicit one, so every executable model has exactly
  one configuration. A program nothing binds runs under a task the model
  lists, in a resource the model lists. Only their names are absent.
- **A system global is named by what it is.** `SystemGlobal::UpTime` tells a
  backend what the runtime writes into it. The backend does not recognise the
  global by its name, or by its type.
- **A global carries no type.** What a backend stores for a declared global is
  its type in the lowered program's type table, which does not exist yet.
  Until it does, see [What a backend still reads](#what-a-backend-still-reads).
- **The interval is a duration, not microseconds.** Microseconds are the
  container's unit, and the bytecode backend converts.
- **The priority is as declared.** `u32` is what the source can write; the
  container's `u16` is the bytecode backend's limit and its check.
- **Only names carry a position.** A diagnostic about a task's priority or
  interval is labelled at the task's name, so the interval carries no span of
  its own.

### What a backend still reads

The model replaces the configuration's declarations. It does not yet replace
two things codegen compiles from the analyzed library, and both are reached
by id, never by name in codegen:

- **A program's body**, from `ProgramId`. Moving bodies to the lowered program
  is that design's work.
- **A declared global's type and initial value**, from `GlobalId`. The
  lowered program carries both: its variables have a type in its type table,
  and initial values are statements in `init`.

`CleanAnalysis` answers both by id, as `program_declaration(ProgramId)` and
`global_declaration(GlobalId)`, from the same walk of the library that
allocated the ids. `CleanAnalysis::new` runs that walk on the library it
holds, so the model codegen receives is always that library's, and the walk
keeps a reference to each declaration rather than a position in the library:
the borrow checker, not a convention, ties a declaration to the library it
came from. The `SemanticContext` keeps its own copy of the model, without the
declarations, for `check` and the editor. `CleanAnalysis` lives in the
analyzer, not `ironplc-ir`, because it hands out declaration nodes. It is the only place a backend reaches a declaration
through the model, and it goes away when the lowered program carries bodies,
types and initial values.

### Relationship to the lowered program

The model is the part of the lowered program that says what runs and when,
and is the first thing in `ironplc-ir`:

| Execution model | Lowered program |
|---|---|
| `Resource`, `Task`, `ProgramInstance` | `Program::tasks`, `Program::instances` |
| `ProgramId` | The `PouId` of the program's body |
| `GlobalId` | The `VarId` of the global |
| The storage of a `ProgramInstance` | A global holding the instance (§3.2), which lowering allocates |
| `DebugName` | The names kept on declarations for debug information (REQ-LOW-lowering-021) |

When the lowered program's `Program` exists, it holds the `ExecutionModel`
rather than repeating it, and `ProgramId` and `GlobalId` become, or are
replaced by, its `PouId` and `VarId`.

The lowered program design says that a library with no configuration lowers
nothing, because only POUs reachable from a program instance are lowered
(REQ-LOW-lowering-025). With this model a library has no configuration only in
its source: the model gives it an implicit one, whose instance makes the only
`PROGRAM` reachable, so it lowers as it compiles today. That sentence of the
lowered program design should read "a library with no program instance".

### Alternatives for review

- **Instances owned by their program type** rather than by their task, with a
  task id on each instance. Compiling one body once per type reads naturally
  from it, but every scheduling question goes through an id. Rejected because
  scheduling is what the model is for.
- **One flat list of tasks across resources.** The bytecode container's task
  table is flat, so the bytecode backend flattens the model's resources. A
  flat model would lose which resource a task belongs to, which IEC 61131-3
  scopes task names and resource globals by.
- **The `SINGLE` trigger by `GlobalId` or carried on the task as the global's
  slot.** A slot is the backend's layout, so the model gives the global and
  the backend's layout gives the slot.
- **The types in the analyzer until `ironplc-ir` exists.** Rejected: creating
  `ironplc-ir` now puts the model where the lowered program will hold it, and
  sets up the crate the lowering work needs.
- **`ironplc-lowering` now, to build the model.** Rejected for now: it would
  hold one function. The analyzer builds the model until lowering exists (see
  [A temporary departure](#a-temporary-departure-from-the-lowered-programs-crates)).
- **A source position type of the model's own.** Rejected; see
  [Source positions](#source-positions).

### Source positions

The model keeps `SourceSpan` rather than a position type of its own, and a
backend reaches one only through `DebugName::span`.

- **A span is not what the identity rule is about.** `SourceSpan` is a file id
  and two offsets. It is not syntax, and nothing is looked up by it. The rule
  keeps declaration nodes and source names from being used as keys, and a
  span is neither.
- **A type of the model's own would be converted straight back.** Every
  problem a backend reports is an `ironplc-dsl` `Diagnostic`, whose labels are
  built by `Label::span(SourceSpan)`, and the debug section finds a source
  file by the span's `FileId`. A position type of the model's own would be
  turned back into a `SourceSpan` at every diagnostic, and the two would be
  one thing under two names, free to drift apart.
- **A span is reachable, not stored.** It is a private field of `DebugName`,
  read through an accessor, so a backend uses it to label a diagnostic and has
  no field it could keep or compare.

For a backend to drop `ironplc-dsl` altogether (REQ-LOW-codegen-002 in the
lowered program design), it is not the span alone that has to come from
elsewhere: `SourceSpan`, `FileId`, `Label` and `Diagnostic` go together. The
way to get there is to move those four types into a small crate of their own,
which `ironplc-dsl` re-exports so that nothing else changes, and which
`ironplc-ir` and every backend depend on in place of `ironplc-dsl`. That move
is mechanical, independent of this design, and a prefactor of its own. Until
it lands, `ironplc-ir` imports `SourceSpan` from `ironplc-dsl`.

## Behaviour

### What the model holds

**REQ-EM-ir-001** The model's types hold no declaration node, no reference into a library, and no `Id` or `TypeId`: a name is a `DebugName`, which implements neither `Eq`, `Hash` nor `Ord`, and every reference between parts of the model is ownership or an id the model allocated.

**REQ-EM-ir-002** `ironplc-ir` depends on no compiler crate but `ironplc-dsl`.

The model holds every resource, task and instance the configuration declares.
It does not know that the VM runs one program instance; codegen checks that
against it (see [Backend capability checks](#backend-capability-checks)).

### Binding programs to tasks

**REQ-EM-analyzer-010** With no `CONFIGURATION`, the model has an implicit configuration holding one implicit resource, which holds one implicit freewheeling task of priority 0, which runs one implicit instance of the only `PROGRAM`.

This is what codegen does today, and the model keeps it. The default
configuration sketched in [Task Support](61131-task-support.md) has a cyclic
task of 10 ms; the compiler has never built that, and this design does not
adopt it.

**REQ-EM-analyzer-011** An instance with no `WITH` clause is owned by an implicit freewheeling task of its resource, listed after the declared tasks and shared by every such instance of that resource.

A `WITH` that names a task the resource does not declare is reported by
`rule_program_task_definition_exists` (P4006); the model binds that instance to
the implicit task, so every instance has a task.

A configuration whose resources instantiate no program binds the only `PROGRAM`
implicitly, as with no configuration.

Every instance in an `ExecutionModel` is owned by exactly one task, so an
instance cannot be bound to no task, or to a task of another resource.

### Task kind

**REQ-EM-analyzer-020** A task that declares `SINGLE` has an event schedule, whatever its interval; a `SINGLE` naming a global triggers on that global's `GlobalId`.

**REQ-EM-analyzer-021** A task with a positive `INTERVAL` and no `SINGLE` has a cyclic schedule, whose interval is recorded as a duration at the precision written.

**REQ-EM-analyzer-022** A task with an absent or zero `INTERVAL` and no `SINGLE` has a freewheeling schedule. A zero interval means "as fast as possible", which is what a freewheeling task does; scheduling it as cyclic with a zero period would leave it permanently overdue.

**REQ-EM-analyzer-023** The declared priority is recorded unchanged, whatever its size; whether a backend can represent it is the backend's check.

### Several resources, tasks and instances

**REQ-EM-analyzer-030** Every resource, task and program instance of the configuration is resolved: a configuration of two resources with several tasks and several instances resolves completely.

The structured text grammar accepts one `RESOURCE` per configuration; the
PLCopen XML front end gives a configuration as many as it declares.

### Globals

**REQ-EM-analyzer-040** The globals in scope are listed system first, then top-level `VAR_GLOBAL` in the order the files were given, then the configuration's, then each resource's in declaration order; a global's `GlobalId` is its position in that list.

**REQ-EM-analyzer-041** With `allow_system_uptime_global`, the first globals are the system globals `SystemGlobal::UpTime` and `SystemGlobal::UpLTime` (`__SYSTEM_UP_TIME` and `__SYSTEM_UP_LTIME`); without it there are none.

The VM writes the uptime globals by slot, which is why they come first.

A resource's `VAR_GLOBAL` is in the model, but codegen does not give it storage
yet: it builds the variable table from the system, top-level and configuration
globals, in the order above, so that a program that compiles today keeps its
variable indexes.

### When no executable can be built

These cases are recorded in the model, not reported by analysis: `check`, the language server and the MCP server analyze library
files that legitimately declare no `PROGRAM`, or several.

**REQ-EM-analyzer-050** A library that declares no `PROGRAM` resolves to `NotExecutable::NoProgram`, whatever else it declares.

**REQ-EM-analyzer-051** A library that declares more than one `CONFIGURATION` is recorded as not executable, with their names in source order; the model resolves none of them rather than picking one.

**REQ-EM-analyzer-052** A library that declares more than one `PROGRAM` and no configuration that binds one is recorded as not executable, with their names in source order. A configuration that binds one of several programs is executable.

**REQ-EM-analyzer-053** A configuration with a program instance whose type is not a `PROGRAM` declaration of the library, such as a function block or a name declared nowhere, is recorded as not executable, with those type names in declaration order. An `ExecutionModel` therefore never holds an instance without a program.

A configuration is commonly kept in a file of its own, and checking that file
alone must not report the programs it names as missing, which is why this is
recorded rather than reported.

### Language rules on tasks

These are errors in the library whatever backend compiles it, so they are
analyzer rules, reported by `check` and in the editor.

**REQ-EM-analyzer-060** A task whose `INTERVAL` is negative is reported as P4078.

**REQ-EM-analyzer-061** A task whose `SINGLE` names a variable that is not a declared global is reported as P4007.

**REQ-EM-analyzer-062** A task whose `SINGLE` names a global that is not `BOOL` is reported as P4079.

Codegen used to reject a negative interval as out of range (P4048), and to
accept an undeclared or non-`BOOL` `SINGLE` until it rejected the event task.

### Backend capability checks

Codegen reports why the model is not executable, then checks the model against
what the VM can run. These checks read the model; they resolve nothing. Only
the task of the instance that runs is checked: a task no instance runs under is
never emitted.

**REQ-EM-codegen-001** The task table entry takes its priority, its type and its interval in microseconds from the task the program instance runs under: a cyclic task's interval, and 0 for a freewheeling task.

The container counts the interval in whole microseconds, so a cyclic interval
shorter than one microsecond is 0 there, and the entry is freewheeling rather
than cyclic with a period of 0, as before the model.

**REQ-EM-codegen-002** A model with more than one program instance, or a library with more than one `PROGRAM` declaration, is P9999 with the help naming #1613. The VM runs one program instance; lifting that limit removes this check.

**REQ-EM-codegen-003** An event task is P4047: the VM does not schedule event tasks.

**REQ-EM-codegen-004** A priority above 65535 or an interval that does not fit 64-bit microseconds is P4048, labelled at the task's name: the container stores them in those widths.

**REQ-EM-codegen-005** A model recorded as not executable is reported for its reason: no `PROGRAM` is P4020, several `PROGRAM`s is P9999 with the help naming #1613, several `CONFIGURATION`s is P4081, and an instance of a type that is not a `PROGRAM` is P4080.

Codegen used to compile the only `PROGRAM` freewheeling and ignore an instance
of a function block or of an undeclared program; such a library now reports
P4080 instead of compiling. Codegen also used to build the last of several
configurations without a word; that now reports P4081.

**REQ-EM-codegen-006** The container header's system-uptime flag is set exactly when the model lists `SystemGlobal::UpTime`.

**REQ-EM-codegen-007** `ironplc-codegen` builds the task table and the global variables from the execution model, and reaches a declaration only through `CleanAnalysis::program_declaration` and `CleanAnalysis::global_declaration`.

A program that compiled before the model compiles to the same container bytes
under it.
