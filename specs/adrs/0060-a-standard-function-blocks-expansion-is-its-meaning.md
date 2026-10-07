# A Standard Function Block's Expansion Is Its Meaning

status: proposed
date: 2026-10-07

## Context and Problem Statement

[ADR-0003](0003-plc-standard-function-blocks-as-intrinsics.md) made the
standard function blocks intrinsics of the bytecode VM: the VM recognizes a
standard block when it is called and runs native code for it. What each block
does is stated only by that code:

* **Hidden state.** The VM's on-delay timer keeps the time it started, and
  whether it is running, in fields the language does not declare.
* **Time.** The timers read the time of the current round, which only the VM's
  native code can reach.
* **Widths.** The counters of five integer widths share one implementation,
  which works at one width.

A second backend would need its own implementation of every standard block,
written from the VM's code, and the two could drift apart.

Where is a standard function block's meaning stated?

## Decision Drivers

* **One meaning for each block**, shared by every backend.
* **A backend may keep a fast native implementation.**
* **Hidden state is visible** as named fields that a debugger can show.
* **Time is read one way** on every target.

## Considered Options

* An expansion into lowered statements is the meaning, and a backend may
  implement a block natively instead
* Each backend implements every block natively
* Standard blocks as library source

## Decision Outcome

Chosen option: "An expansion into lowered statements is the meaning, and a
backend may implement a block natively instead", because it states each block
once while letting the bytecode VM keep its fast path.

* **Each standard function block has an expansion:** the block written as
  ordinary lowered statements over the fields of its instance. The expansion
  is the block's meaning.
* **State the block keeps is a field of its instance**, named, so a debugger
  can show it, even where the language declares no such field.
* **The block reads the time of the current round** from the lowered program,
  the same way on every target.
* **A backend may implement a block natively**, as the bytecode VM does. A
  native implementation must behave as the expansion does.

This extends ADR-0003 and does not supersede it: the VM still runs the standard
blocks natively.

### Consequences

* Good, because a second backend gets every standard block by expanding it.
* Good, because each block's state is a named field.
* Good, because each width of a block must be expanded on purpose, so one width
  cannot inherit another's behaviour unnoticed.
* Bad, because every standard block needs an expansion, written and tested.
* Bad, because the VM's native code and the expansions must be kept in step by
  tests.

## Pros and Cons of the Options

### An expansion into lowered statements is the meaning (chosen)

* Good, because the meaning is one piece of code that every backend can run.
* Bad, because the VM keeps a second implementation, which only tests keep
  equal.

### Each backend implements every block natively

* Good, because each backend is as fast as it can be.
* Bad, because each backend learns what a block does from another backend's
  code, and the implementations can drift apart.

### Standard blocks as library source

Written in Structured Text and shipped like a compatibility library.

* Good, because a block would be ordinary source.
* Bad, because Structured Text cannot read the time of the round or declare
  hidden fields.
