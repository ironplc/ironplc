# Codegen Consumes a Resolved Execution Model

status: accepted
date: 2026-10-07

## Context and Problem Statement

Which `CONFIGURATION` is built, which `PROGRAM` runs under which `TASK`, what
a task's `INTERVAL`, `PRIORITY` and `SINGLE` mean, and which global variables
exist are decisions of the language. Codegen's driver made all of them while
it compiled, reading the configuration's declarations from the library:

* it took the first `CONFIGURATION` in the toposorted library and ignored the
  others without a word, which in practice was the *last* one in the file;
* it bound the program to a task by matching instance type names, and decided
  from the task's parameters whether it was cyclic or freewheeling;
* it synthesized the system uptime globals as declarations of its own while
  the analyzer separately registered the same names as symbols;
* it rejected a negative `INTERVAL` as out of the container's range, so
  `check` and the editor accepted it.

A second backend would have had to make the same decisions the same way, and
`check` could not report the ones that are errors in the program. ADR-0056
moved implicit conversions out of codegen for the same reason. Where should
the execution decisions live?

## Decision Drivers

* **Backends lower; they do not decide** what the language means.
* **The answer is plain data.** A backend that reads the configuration's
  declarations can make the decision again, differently.
* **The language allows several resources, tasks and instances.** The VM's
  limit of one program instance is the VM's, not the language's.
* **`check` accepts libraries**: a file of functions, or a configuration kept
  apart from its programs, is valid on its own.

## Considered Options

* An analyzer step resolves an execution model that codegen lowers
* Codegen keeps the decisions, behind shared helper functions

## Decision Outcome

Chosen option: "An analyzer step resolves an execution model that codegen
lowers", because it makes one answer that every backend, and `check`, reads.

* **`execution_model::resolve`** runs at the end of `stages::resolve_types`
  and stores an `ExecutionModel` on the `SemanticContext`, so `CleanAnalysis`
  hands it to codegen. It holds the configuration's name, every resource with
  its tasks and program instances, each task's priority, interval, `SINGLE`
  trigger and kind (cyclic, freewheeling or event), and every global in scope
  with its type id and where it was declared. It holds no configuration,
  resource, task or program configuration node.
* **A program nothing binds runs under an implicit freewheeling task** that
  the model lists, so every instance has a task.
* **Ambiguity is recorded, not resolved.** Several configurations, several
  programs that no configuration binds, no program, and an instance of
  something that is not a program are reasons the model is not executable.
  They are not analysis diagnostics, so `check` still accepts the library;
  codegen reports them.
* **Errors in the program are analyzer rules**: a negative interval, and a
  `SINGLE` that names no `BOOL` global.
* **Codegen keeps only capability checks** against the model: one program
  instance (#1613), no event tasks, and a priority and interval that fit the
  container. Lifting the VM's limit removes a check; the model does not
  change.

Initial values are not in the model yet. Until they are, codegen looks a
declared global's initializer up by name in the analyzed library. See
`specs/design/execution-model.md`.

### Consequences

* Good, because a second backend reads the same answer and cannot choose a
  different configuration, task kind or global order.
* Good, because a library with two configurations is now refused with a
  diagnostic (P4076) instead of building the last one silently.
* Good, because `check` reports a negative interval (P4073) and a `SINGLE`
  that is undeclared (P4007) or not `BOOL` (P4074).
* Good, because an instance of a function block or of an undeclared program is
  now refused (P4075). Codegen used to ignore it and run the only program
  freewheeling.
* Bad, because the model carries `Id`s, not only strings, so that a backend
  can label a diagnostic; the source spans come with them.
* Bad, because until initial values are resolved codegen still reads each
  global's declaration, so the model is not yet the whole of what codegen
  needs about globals.

### Confirmation

The model's rules are tested in `analyzer/src/execution_model/tests.rs`, and
the capability checks in `codegen/src/execution/tests.rs`, each tied to a
`REQ-EM-*` requirement in `specs/design/execution-model.md`. The codegen
integration tests, including `wire_format.rs` and the end-to-end tests, pass
unchanged: a program that compiled before compiles to the same bytes.

## Pros and Cons of the Options

### An analyzer step resolves an execution model that codegen lowers (chosen)

* Good, because the decisions are made once and are visible to every
  consumer, including the language server.
* Good, because language errors in a task move to `check`.
* Bad, because there is one more structure to keep in step with the
  configuration syntax.

### Codegen keeps the decisions, behind shared helper functions

* Good, because nothing moves.
* Bad, because a helper that reads declarations can be bypassed, and
  `check` still cannot report the decisions that are errors.
