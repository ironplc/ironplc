# A Call Through an Interface Lists Its Implementers

status: proposed
date: 2026-10-07

## Context and Problem Statement

A call through an interface runs the method of whichever function block the
interface value refers to. Code generation does not compile such a call today.
[ADR-0041](0041-staged-method-and-interface-dispatch.md) decided static
dispatch, and left dynamic dispatch through interfaces to a decision of its
own.

The lowered program must be able to express such a call. Backends would
dispatch differently: the bytecode VM through a table in its container, a
native backend through a switch or a table of functions. Every function block
that implements an interface is in the program the compiler sees, because
libraries are merged before analysis and there is no separate compilation.

How does the lowered program express a call through an interface?

## Decision Drivers

* **Target neutral:** the lowered program does not fix how a call dispatches.
* **One meaning:** the method of the value's concrete type runs, with the
  value's instance as its receiver, and a call through a null value traps.
* **A direct call where only one type is possible.**

## Considered Options

* The call lists the implementers, and a value that can hold only one type is a
  reference to it
* A dispatch table in the lowered program
* A branch on the value's type, with a direct call for each type

## Decision Outcome

Chosen option: "The call lists the implementers, and a value that can hold only
one type is a reference to it", because it fixes what the call means and leaves
how it dispatches to each backend.

* **Analysis finds** the concrete types each interface value can hold.
* **Where only one type is possible,** the value is a reference to that
  function block, and a call through it is a direct method call.
* **Otherwise the call is one call in the lowered program** that lists, for
  every concrete type the value can hold, that type's method.
* **Each backend chooses how to dispatch.**

This does not decide whether or when dynamic dispatch is enabled; that stays
with ADR-0041.

### Consequences

* Good, because each backend can dispatch in the way that suits its target.
* Good, because the common case, a value that can hold only one type, is a
  direct call on every target.
* Bad, because lowering needs the whole program to list the implementers.
  Separate compilation would need another representation.

## Pros and Cons of the Options

### The call lists the implementers (chosen)

* Good, because the lowered program states what the call means and nothing
  about how it dispatches.
* Bad, because each backend implements dispatch itself.

### A dispatch table in the lowered program

* Good, because every backend would dispatch the same way.
* Bad, because it fixes a mechanism that suits some targets and not others.

### A branch on the value's type, with a direct call for each type

* Good, because it needs nothing new in the lowered program for the call.
* Bad, because it fixes one target's mechanism in the lowered program, and
  needs a way to take an interface value apart.
