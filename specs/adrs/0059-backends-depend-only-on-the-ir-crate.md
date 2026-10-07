# Backends Depend Only on the IR Crate

status: proposed
date: 2026-10-07

## Context and Problem Statement

A backend that can reach analysis or the syntax tree will, at some convenient
place, and then decide something again that lowering already decided. A rule
in a review cannot stop that for long. A crate boundary can: Rust lets a crate
use only its direct dependencies.

Built-in functions have to be named on both sides of that boundary. Analysis
knows which function a call resolved to as soon as it resolves the call, and
the operand types only once it has recorded them. The lowered program names an
operation at its operand types. If analysis used the lowered program's names,
analysis would depend on the lowered program.

How are the crates arranged?

## Decision Drivers

* **A backend cannot use analysis or the syntax tree**, and the compiler
  enforces it.
* **A backend does not build analysis.**
* **Analysis does not depend on the lowered program**, so it does not change
  when an operation gains an operand type.

## Considered Options

* The lowered program is a crate of its own, a backend depends on it alone, and
  analysis names built-in functions in its own terms
* Analysis uses the lowered program's names for built-in functions
* One crate for the lowered program and for lowering
* A backend may depend on analysis, kept apart by review

## Decision Outcome

Chosen option: "The lowered program is a crate of its own, a backend depends on
it alone, and analysis names built-in functions in its own terms", because it
is the only arrangement in which the compiler stops a backend from reaching
analysis, and analysis does not depend on the lowered program.

* **The IR, the representation of the lowered program, is a crate of its
  own.** It depends on nothing but the compiler's basic types: ids, source
  positions and diagnostics.
* **A backend depends on the IR crate alone.**
* **Analysis does not depend on it.** Analysis names a built-in function in its
  own terms, and lowering translates that name, with the operand types analysis
  recorded, into the lowered program's.

### Consequences

* Good, because the compiler, not a reviewer, keeps a backend away from
  analysis and the syntax tree.
* Good, because a backend does not build analysis at all.
* Good, because a new operand type for an operation changes the lowered program
  and lowering, not analysis.
* Bad, because each built-in function has two names, and a translation between
  them to keep in step.

## Pros and Cons of the Options

### The lowered program is a crate of its own; analysis names built-in functions in its own terms (chosen)

* Good, because each crate uses only what it needs.
* Bad, because of the second set of names and the translation.

### Analysis uses the lowered program's names for built-in functions

* Good, because there is one set of names and no translation.
* Bad, because analysis would depend on the lowered program, and would have to
  know the operand types of a call before it has recorded them.

### One crate for the lowered program and for lowering

* Good, because there is one crate fewer.
* Bad, because a backend that depends on it depends on lowering, and so on
  analysis.

### A backend may depend on analysis, kept apart by review

* Good, because it needs no new crate.
* Bad, because nothing stops the next convenient call into analysis.
