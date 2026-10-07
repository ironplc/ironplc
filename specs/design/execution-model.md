# Design: The Resolved Execution Model

status: proposed
date: 2026-10-07

## Overview

An IEC 61131-3 library says what runs, and when, through its `CONFIGURATION`:
the resources it declares, the `TASK`s each resource schedules, the `PROGRAM`
instances bound to those tasks, and the `VAR_GLOBAL`s they share (see
[Task Support](61131-task-support.md)). Which configuration is built, which
program runs under which task, what a task's parameters mean, and which globals
exist are language decisions.

Today codegen's driver makes these decisions while it compiles, reading
`ConfigurationDeclaration`, `ResourceDeclaration`, `TaskConfiguration` and
`ProgramConfiguration` nodes from the library. This design moves them into the
analyzer. An analyzer step, `execution_model::resolve`, records the answer as
an **execution model** on the `SemanticContext`, next to `reachable()`. Codegen
takes it through `CleanAnalysis` and lowers it: it builds the task table, the
program instance and the global variables from the model, and never reads the
configuration's declarations. A second backend reads the same model, so it
cannot decide differently. This follows the principle of
ADR-0056: backends lower; they do not decide what the language means.

## Representation

This section is the shape of the data the analyzer hands to codegen. The
constraint that shapes it: **the model is not the DSL.** It holds no
`ConfigurationDeclaration`, `ResourceDeclaration`, `TaskConfiguration`,
`ProgramConfiguration`, `VarDecl`, `DurationLiteral`, `DataSourceKind` or any
other declaration node, and no reference into the library. A consumer that has
the model has every decision already made and nothing to make it again from.

### What the model borrows from `ironplc_dsl`

The model uses three leaf value types from `ironplc_dsl`, and nothing else from
it:

| Type | Used for | Why this type |
|---|---|---|
| `core::Id` | every name: configuration, resource, task, instance, program, global | A name compares case-insensitively, as IEC 61131-3 names do, and carries the `SourceSpan` it was written at, so a backend's diagnostic ("2nd CONFIGURATION", "Task declares SINGLE") can point at the source |
| `core::SourceSpan` | where a task's `INTERVAL` is written | A diagnostic about the interval's value labels the value, not the task name |
| `type_id::TypeId` | a global's type | The analyzer's handle for a type; codegen already maps `TypeId`s to representations through the `TypeEnvironment` |

None of these is a declaration node: each is a value with no children, and
holding one holds nothing else of the library.

### The types

```rust
// compiler/analyzer/src/execution_model.rs

/// Constructed only by `resolve`; read through accessors.
pub struct ExecutionModel {
    configuration: Option<Id>,       // None: no CONFIGURATION, or several
    resources: Vec<Resource>,        // declaration order; implicit one last
    programs: Vec<Id>,               // PROGRAM declarations, source order
    globals: Vec<GlobalVariable>,    // variable-table order, see Globals
    not_executable: Option<NotExecutable>,
}

pub enum NotExecutable {
    NoProgram,
    SeveralConfigurations(Vec<Id>),  // source order
    SeveralPrograms(Vec<Id>),        // source order
    UndeclaredPrograms(Vec<Id>),     // instance types that are no PROGRAM
}

/// Constructed only by `resolve`; read through accessors.
pub struct Resource {
    name: Option<Id>,                // None: the implicit resource
    tasks: Vec<Task>,                // declared, then the implicit one if any
    instances: Vec<ProgramInstance>, // declaration order
}

pub struct TaskIndex(usize);         // position in its resource's `tasks`

pub struct Task {
    pub name: Option<Id>,            // None: an implicit task
    pub priority: u32,               // as declared
    pub interval: Option<TaskInterval>,
    pub single: Option<EventTrigger>,
    pub kind: TaskKind,              // decided by the analyzer
}

pub struct TaskInterval {
    pub duration: time::Duration,    // signed, at the precision written
    pub span: SourceSpan,
}

pub enum EventTrigger {
    Global(Id),                      // SINGLE := <global variable>
    Constant,                        // SINGLE := <constant>
}

pub enum TaskKind { Cyclic, Freewheeling, Event }

pub struct ProgramInstance {
    pub name: Option<Id>,            // None: the implicit instance
    pub program: Id,                 // the PROGRAM type, by name
    pub task: TaskIndex,
}

pub struct GlobalVariable {
    pub name: Id,
    pub type_id: Option<TypeId>,     // None only if its type did not resolve
    pub scope: GlobalScope,
}

pub enum GlobalScope { System, TopLevel, Configuration, Resource(Id) }
```

`ExecutionModel::instances()` yields each instance with its resource and task
(`BoundInstance { resource, instance, task }`), so a consumer never indexes
`tasks` itself.

### Decisions in the representation

- **Implicit objects are present, not inferred.** A program nothing binds
  runs under a task that the model lists. An implicit resource, task or
  instance is an ordinary entry whose `name` is `None`. A consumer iterates
  instances and tasks without asking whether the source declared them.
- **The task kind is a field, not something to derive.** `TaskKind` is
  computed once from `SINGLE` and `INTERVAL` (see [Task kind](#task-kind)).
  The parameters stay in the model so a backend can report a value it cannot
  represent, but no backend decides cyclic versus freewheeling again.
- **An interval is a duration, not microseconds.** Microseconds are the
  container's unit, and a backend converts. The duration is signed because
  the source can write a negative one, which a rule reports.
- **The task binding is an index into the instance's own resource.** Tasks
  are scoped to a resource in IEC 61131-3, so two resources may each declare a
  task with the same name. An index into the resource's `tasks` cannot point
  at another resource's task, and a name lookup is never repeated downstream.
- **A program is referenced by name.** There is no POU id in the compiler
  today. A backend that compiles the program's body finds the declaration by
  this name; that is the POU body, which is outside this design.
- **Ambiguity is a value beside a partial model, not an error.** When the
  library cannot be built into an executable, `not_executable` says why, and
  the rest of the model is still filled in as far as it is unambiguous: the
  programs, and the globals outside any configuration, always; the
  configuration's resources and globals unless there are several
  configurations. It is not a `Result`, because analysis of the library
  succeeded and the language server can still use what was resolved.
- **The two containers are opaque; the leaves are not.** `ExecutionModel` and
  `Resource` keep their fields private, because `TaskIndex` is only valid
  against the resource that made it and only `resolve` keeps that true.
  `Task`, `ProgramInstance` and `GlobalVariable` have public fields: they hold
  no invariant across fields, and a backend's unit test can build a `Task`
  directly to test a capability check.

### What codegen still reads from the library

- **Program bodies.** Codegen compiles the instance's `PROGRAM`, found by
  `ProgramInstance::program`, and the functions and function blocks it
  reaches. Moving POU bodies off the AST is a separate change.
- **Global initial values.** The model gives each global its name, type and
  scope, but not its initializer. Until initial values are resolved, codegen
  looks a declared global's declaration up by name in its scope (top level or
  the configuration). That is the one place codegen reads a declaration to
  build the execution model, and the code says so. A system global has no
  declaration; codegen declares it from the model's name and type.

### Alternatives for review

- **Names as `Id` or as a model-owned name type.** `Id` brings the
  case-insensitive comparison and the span for diagnostics, but it ties the
  model to `ironplc_dsl`. A `Name { text, span }` defined in the analyzer
  would break that tie at the cost of a second name type with the same rules.
- **Implicit objects as `Option<Id>` or as an explicit origin.**
  `name: Option<Id>` is compact, but `None` reads as "unnamed" rather than
  "the compiler supplied this". An `enum Origin { Declared(Id), Implicit }`
  would say it outright.
- **Nested or flat.** Tasks and instances are nested in their resource. The
  container's task table is flat with `u16` task ids, so a backend that emits
  more than one task flattens them. A flat model with global `TaskId`s would
  match the container but lose the resource scoping above.
- **The `SINGLE` trigger by name or by position in `globals`.**
  `EventTrigger::Global(Id)` repeats a lookup a backend must make to find the
  variable's slot. An index into `globals` would make that lookup the
  analyzer's, as the task binding is. A constant trigger keeps no value: no
  backend runs event tasks yet.
- **The priority as declared or as the container's width.** `u32` is what
  the parser produces; `u16` is what the container stores. The model keeps the
  declared value so that the container's limit stays a container check.

## Behaviour

### What the model holds

**REQ-EM-analyzer-001** The model is plain data: names, type ids, durations, integers and enums. It holds no configuration, resource, task or program configuration node and no reference to one, so it outlives the library it was resolved from.

The model holds every resource, task and instance the configuration declares.
It does not know that the VM runs one program instance; codegen checks that
against it (see [Backend capability checks](#backend-capability-checks)).

### Binding programs to tasks

**REQ-EM-analyzer-010** With no `CONFIGURATION`, the only `PROGRAM` runs as an implicit instance under an implicit freewheeling task, in an implicit resource the model lists.

**REQ-EM-analyzer-011** An instance with no `WITH` clause runs under an implicit freewheeling task of its resource, listed after the declared tasks and shared by every such instance of that resource.

A `WITH` that names a task the resource does not declare is reported by
`rule_program_task_definition_exists` (P4006); the model binds that instance to
the implicit task, so every instance has a task.

A configuration whose resources instantiate no program binds the only `PROGRAM`
implicitly, as with no configuration.

### Task kind

**REQ-EM-analyzer-020** A task that declares `SINGLE` is an event task, whatever its interval.

**REQ-EM-analyzer-021** A task with a positive `INTERVAL` and no `SINGLE` is cyclic, and its interval is recorded as a duration at the precision written.

**REQ-EM-analyzer-022** A task with an absent or zero `INTERVAL` and no `SINGLE` is freewheeling. A zero interval means "as fast as possible", which is what a freewheeling task does; scheduling it as cyclic with a zero period would leave it permanently overdue.

**REQ-EM-analyzer-023** The declared priority is recorded unchanged, whatever its size; whether a backend can represent it is the backend's check.

### Several resources, tasks and instances

**REQ-EM-analyzer-030** Every resource, task and program instance of the configuration is resolved: a configuration of two resources with several tasks and several instances resolves completely.

The structured text grammar accepts one `RESOURCE` per configuration today; the
PLCopen XML front end gives a configuration as many as it declares.

### Globals

**REQ-EM-analyzer-040** The globals in scope are listed system first, then top-level `VAR_GLOBAL` in the order the files were given, then the configuration's, then each resource's in declaration order, each with the type id its declaration records.

**REQ-EM-analyzer-041** With `allow_system_uptime_global`, `__SYSTEM_UP_TIME` and `__SYSTEM_UP_LTIME` are the first globals, typed `TIME` and `LTIME` like any other entry; without it there are none.

The VM writes the uptime globals by slot, which is why they come first.

A resource's `VAR_GLOBAL` is in the model, but codegen does not give it storage
yet: it builds the variable table from the system, top-level and configuration
globals, in the order above, so that a program that compiles today keeps its
variable indexes.

### When no executable can be built

These cases are recorded in the model, not reported by analysis: `check`, the
language server and the MCP server analyze library files that legitimately
declare no `PROGRAM`, or several.

**REQ-EM-analyzer-050** A library that declares no `PROGRAM` is recorded as not executable for that reason, whatever else it declares.

**REQ-EM-analyzer-051** A library that declares more than one `CONFIGURATION` is recorded as not executable, with their names in source order; the model resolves none of them rather than picking one.

**REQ-EM-analyzer-052** A library that declares more than one `PROGRAM` and no configuration that binds one is recorded as not executable, with their names in source order. A configuration that binds one of several programs is executable.

**REQ-EM-analyzer-053** A configuration with a program instance whose type is not a `PROGRAM` declaration of the library, such as a function block or a name declared nowhere, is recorded as not executable, with those type names in declaration order. The instances are still resolved.

A configuration is commonly kept in a file of its own, and checking that file
alone must not report the programs it names as missing, which is why this is
recorded rather than reported.

### Language rules on tasks

These are errors in the library whatever backend compiles it, so they are
analyzer rules, reported by `check` and in the editor.

**REQ-EM-analyzer-060** A task whose `INTERVAL` is negative is reported as P4073.

**REQ-EM-analyzer-061** A task whose `SINGLE` names a variable that is not a declared global is reported as P4007.

**REQ-EM-analyzer-062** A task whose `SINGLE` names a global that is not `BOOL` is reported as P4074.

Today codegen rejects a negative interval as out of range (P4048), and accepts
an undeclared or non-`BOOL` `SINGLE` until it rejects the event task.

### Backend capability checks

Codegen reports why the model is not executable, then checks the model against
what the VM can run. These checks read the model; they resolve nothing. Only
the task of the instance that runs is checked: a task no instance runs under is
never emitted.

**REQ-EM-codegen-001** The task table entry takes its priority, its type and its interval in microseconds from the task the program instance runs under: a cyclic task's interval, and 0 for a freewheeling task.

**REQ-EM-codegen-002** A model with more than one program instance, or a library with more than one `PROGRAM` declaration, is P9999 with the help naming #1613. The VM runs one program instance; lifting that limit removes this check.

**REQ-EM-codegen-003** An event task is P4047: the VM does not schedule event tasks.

**REQ-EM-codegen-004** A priority above 65535 or an interval that does not fit 64-bit microseconds is P4048: the container stores them in those widths.

**REQ-EM-codegen-005** A model recorded as not executable is reported for its reason: no `PROGRAM` is P4020, several `PROGRAM`s is P9999 with the help naming #1613, several `CONFIGURATION`s is P4076, and an instance of a type that is not a `PROGRAM` is P4075.

Today codegen compiles the only `PROGRAM` freewheeling and ignores an instance
of a function block or of an undeclared program; such a library will report
P4075 instead of compiling. Today codegen also builds the last of several
configurations without a word; that will report P4076.

**REQ-EM-codegen-006** The container header's system-uptime flag is set exactly when the model lists a system global.

A program that compiles today compiles to the same container bytes under the
model.
