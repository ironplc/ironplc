# One Name per Built-in Operation and Operand Type

status: proposed
date: 2026-10-07

## Context and Problem Statement

The lowered program names each operation the compiler implements itself, such
as the standard functions, and each standard function block.

A name for each operation alone leaves the operand types to the call. Then the
square root of an integer can be written, something has to refuse it, and a
backend that handles every name does not thereby handle every type of every
operation.

Today the counters of five integer widths share one implementation in the
bytecode VM, which works at one width. Nothing showed that the wider counters
inherited it.

How fine are the names?

## Decision Drivers

* **A combination the language does not allow cannot be written.**
* **A backend that handles every name handles every combination**, and the
  compiler checks that it does.
* **Consistent with the bytecode VM**, whose instructions and built-in
  functions are already separate for each operand type.

## Considered Options

* One name for each operation at each set of operand types
* One name for each operation, with the operand types taken from the call

## Decision Outcome

Chosen option: "One name for each operation at each set of operand types",
because a combination the language does not allow then has no name, and the
compiler can check that a backend handles every combination.

* **Each built-in operation is named once for each set of operand types.** The
  square root of a `REAL` and the square root of an `LREAL` are two
  operations.
* **Each standard function block is named once for each set of field types.**
  A counter of `INT` and a counter of `LINT` are two blocks.
* **A call's arguments have exactly the types its operation names.**

### Consequences

* Good, because the square root of an integer cannot be written.
* Good, because a new operation, or a new operand type for one, fails to
  compile in every backend until the backend handles it.
* Good, because each counter width has to be implemented on purpose.
* Bad, because there are many more names, and lowering's translation from
  analysis's names grows with each operand type an operation gains.

## Pros and Cons of the Options

### One name for each operation at each set of operand types (chosen)

* Good, because invalid combinations cannot be written.
* Bad, because the list of names is long.

### One name for each operation, with the operand types taken from the call

* Good, because the list is short, about the length of the standard's list of
  functions.
* Bad, because invalid combinations can be written and must be refused when the
  program is built.
* Bad, because handling every name no longer shows which types a backend
  handles.
