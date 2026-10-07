# Design: The Resolved Execution Model

status: implemented
date: 2026-10-07

## Overview

An IEC 61131-3 library says what runs, and when, through its `CONFIGURATION`:
the resources it declares, the `TASK`s each resource schedules, the `PROGRAM`
instances bound to those tasks, and the `VAR_GLOBAL`s they share (see
[Task Support](61131-task-support.md)). Which configuration is built, which
program runs under which task, what a task's parameters mean, and which globals
exist are language decisions.

The analyzer makes them once, in `execution_model::resolve`, and records the
answer as an **execution model** on the `SemanticContext`, next to
`reachable()`. Codegen takes it through `CleanAnalysis` and lowers it: it builds
the task table, the program instance and the global variables from the model,
and does not read the configuration from the library (ADR-0057). A second
backend reads the same model, so it cannot decide differently.

## What the model holds

**REQ-EM-analyzer-001** The model is plain data: names, type ids, durations, integers and enums. It holds no configuration, resource, task or program configuration node and no reference to one, so it outlives the library it was resolved from.

A name is an `Id`, which carries where it is written so that a backend's
diagnostic can point at it.

| Part | Contents |
|---|---|
| Configuration | its name, or none |
| Resources | each with its tasks and its program instances, in declaration order |
| Tasks | name, declared priority, interval as a duration, the `SINGLE` trigger (a global's name, or a constant), and its kind: cyclic, freewheeling or event |
| Program instances | instance name, the `PROGRAM` type it instantiates (by name), and the task it runs under |
| Programs | the names of the `PROGRAM` declarations, in source order |
| Globals | name, type id and where it was declared: system, top level, configuration or resource |
| Not executable | why no executable can be built, or nothing |

The model holds every resource, task and instance the configuration declares.
It does not know that the VM runs one program instance; codegen checks that
against it (see [Backend capability checks](#backend-capability-checks)).

## Binding programs to tasks

**REQ-EM-analyzer-010** With no `CONFIGURATION`, the only `PROGRAM` runs as an implicit instance under an implicit freewheeling task, in an implicit resource the model lists.

**REQ-EM-analyzer-011** An instance with no `WITH` clause runs under an implicit freewheeling task of its resource, listed after the declared tasks and shared by every such instance of that resource.

A `WITH` that names a task the resource does not declare is reported by
`rule_program_task_definition_exists` (P4006); the model binds that instance to
the implicit task, so every instance has a task.

A configuration whose resources instantiate no program binds the only `PROGRAM`
implicitly, as with no configuration.

## Task kind

**REQ-EM-analyzer-020** A task that declares `SINGLE` is an event task, whatever its interval.

**REQ-EM-analyzer-021** A task with a positive `INTERVAL` and no `SINGLE` is cyclic, and its interval is recorded as a duration at the precision written.

**REQ-EM-analyzer-022** A task with an absent or zero `INTERVAL` and no `SINGLE` is freewheeling. A zero interval means "as fast as possible", which is what a freewheeling task does; scheduling it as cyclic with a zero period would leave it permanently overdue.

**REQ-EM-analyzer-023** The declared priority is recorded unchanged, whatever its size; whether a backend can represent it is the backend's check.

## Several resources, tasks and instances

**REQ-EM-analyzer-030** Every resource, task and program instance of the configuration is resolved: a configuration of two resources with several tasks and several instances resolves completely.

## Globals

**REQ-EM-analyzer-040** The globals in scope are listed system first, then top-level `VAR_GLOBAL` in the order the files were given, then the configuration's, then each resource's in declaration order, each with the type id its declaration records.

**REQ-EM-analyzer-041** With `allow_system_uptime_global`, `__SYSTEM_UP_TIME` and `__SYSTEM_UP_LTIME` are the first globals, typed `TIME` and `LTIME` like any other entry; without it there are none.

The VM writes the uptime globals by slot, which is why they come first.

**Initial values are not in the model yet.** Until they are, codegen looks a
declared global's initializer up by its name and scope in the analyzed library.
That is the only place codegen still reads a declaration to build the execution
model, and the code says so where it does it.

A resource's `VAR_GLOBAL` is in the model, but codegen does not give it storage
yet: it builds the variable table from the system, top-level and configuration
globals, in the order above, so that a program that compiled before keeps its
variable indexes.

## When no executable can be built

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

## Language rules on tasks

These are errors in the library whatever backend compiles it, so they are
analyzer rules, reported by `check` and in the editor.

**REQ-EM-analyzer-060** A task whose `INTERVAL` is negative is reported as P4073.

**REQ-EM-analyzer-061** A task whose `SINGLE` names a variable that is not a declared global is reported as P4007.

**REQ-EM-analyzer-062** A task whose `SINGLE` names a global that is not `BOOL` is reported as P4074.

Before these rules, codegen rejected a negative interval as out of range
(P4048), and accepted an undeclared or non-`BOOL` `SINGLE` until it rejected
the event task.

## Backend capability checks

Codegen reports why the model is not executable, then checks the model against
what the VM can run. These checks read the model; they resolve nothing. Only
the task of the instance that runs is checked: a task no instance runs under is
never emitted.

**REQ-EM-codegen-001** The task table entry takes its priority, its type and its interval in microseconds from the task the program instance runs under: a cyclic task's interval, and 0 for a freewheeling task.

**REQ-EM-codegen-002** A model with more than one program instance, or a library with more than one `PROGRAM` declaration, is P9999 with the help naming #1613. The VM runs one program instance; lifting that limit removes this check.

**REQ-EM-codegen-003** An event task is P4047: the VM does not schedule event tasks.

**REQ-EM-codegen-004** A priority above 65535 or an interval that does not fit 64-bit microseconds is P4048: the container stores them in those widths.

**REQ-EM-codegen-005** A model recorded as not executable is reported for its reason: no `PROGRAM` is P4020, several `PROGRAM`s is P9999 with the help naming #1613, several `CONFIGURATION`s is P4076, and an instance of a type that is not a `PROGRAM` is P4075.

Before the model, codegen compiled the only `PROGRAM` freewheeling and ignored
an instance of a function block or of an undeclared program; such a library
now reports P4075 instead of compiling.

**REQ-EM-codegen-006** The container header's system-uptime flag is set exactly when the model lists a system global.
