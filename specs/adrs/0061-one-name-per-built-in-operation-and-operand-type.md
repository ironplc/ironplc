# One Name per Built-in Operation and Operand Type

status: proposed
date: 2026-10-09

## Context and Problem Statement

The lowered program names each function the compiler implements itself, such
as the standard functions `SQRT` and `CONCAT`.

A name for each function alone leaves the operand types to the call. Then the
square root of an integer can be written, something has to refuse it, and a
backend that handles every name does not thereby handle every type of every
function.

The bytecode VM already tells them apart: each built-in function it implements
has a separate identity for each operand type.

This concerns functions only. The standard function blocks have no names in
the lowered program, because lowering generates them as ordinary function
blocks.

How fine are the names?

## Decision Drivers

* **A combination the language does not allow cannot be written.**
* **A backend that handles every name handles every combination**, and the
  compiler checks that it does.
* **Consistent with the bytecode VM**, whose built-in functions are already
  separate for each operand type.

## Considered Options

* One name for each function at each set of operand types
* One name for each function, with the operand types taken from the call

## Decision Outcome

Chosen option: "One name for each function at each set of operand types",
because a combination the language does not allow then has no name, and the
compiler can check that a backend handles every combination.

* **Each built-in function is named once for each set of operand types.** The
  square root of a `REAL` and the square root of an `LREAL` are two
  operations.
* **A call's arguments have exactly the types its name says.**

### Consequences

* Good, because the square root of an integer cannot be written.
* Good, because a new function, or a new operand type for one, fails to compile
  in every backend until the backend handles it.
* Bad, because there are many more names, and lowering's translation from
  analysis's names grows with each operand type a function gains.

## Pros and Cons of the Options

### One name for each function at each set of operand types (chosen)

* Good, because invalid combinations cannot be written.
* Bad, because the list of names is long.

### One name for each function, with the operand types taken from the call

* Good, because the list is short, about the length of the standard's list of
  functions.
* Bad, because invalid combinations can be written and must be refused when the
  program is built.
* Bad, because handling every name no longer shows which types a backend
  handles.
