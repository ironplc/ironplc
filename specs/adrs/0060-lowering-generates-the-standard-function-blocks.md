# Lowering Generates the Standard Function Blocks

status: proposed
date: 2026-10-09

## Context and Problem Statement

[ADR-0003](0003-plc-standard-function-blocks-as-intrinsics.md) made the
standard function blocks intrinsics of the bytecode VM: the timers, the
counters, the bistables and the edge detectors. The VM recognizes a standard
block when it is called and runs native code for it. What each block does is
stated only by that code.

A second backend would need its own implementation of every standard block,
written from the VM's code, and the two could drift apart. Counting each
counter's form for every integer width, there are 25 standard blocks.

What kept these blocks native was time. A timer reads the time of the current
round, which no source program can name, so a timer could not be written as an
ordinary function block. The lowered program can carry that time.

Who implements the standard function blocks?

## Decision Drivers

* **One implementation of each block**, shared by every backend.
* **No backend implements a standard block.**
* **Hidden state is visible** as named fields that a debugger can show.
* **Time is read one way** on every target.

## Considered Options

* Lowering generates each standard block as an ordinary function block
* Each block has an expansion that is its meaning, and a backend may implement
  it natively instead
* Each backend implements every block natively
* Standard blocks as library source

## Decision Outcome

Chosen option: "Lowering generates each standard block as an ordinary function
block", because then no backend implements, or even sees, a standard block.

* **Lowering generates each standard function block a program uses** as an
  ordinary function block of the lowered program.
* **Each block has one definition in lowering.** A counter's forms for the
  different integer widths come from that one definition.
* **The state a block keeps is fields of its instance**, named, even where the
  language declares no such field.
* **A timer reads the time of the current round**, which the lowered program
  carries.
* **Every backend compiles a standard block** as it compiles any other function
  block.

When the work lands, this supersedes ADR-0003: the bytecode VM no longer runs
the standard blocks natively.

### Consequences

* Good, because a backend has nothing to implement for the standard blocks.
* Good, because the lowered program needs no name for any standard block.
* Good, because each block's state is a named field.
* Good, because each counter computes at its own width. Today every width runs
  on the VM's 32-bit counter, so a wide counter past the 32-bit range will
  behave differently. That is a correction.
* Bad, because the bytecode VM runs the standard blocks as bytecode rather than
  native code, which is slower by an amount not yet measured.
* Bad, because the generated blocks must do what the VM's native code does
  today, and tests must show that they do.

## Pros and Cons of the Options

### Lowering generates each standard block as an ordinary function block (chosen)

* Good, because one definition serves every backend, and no backend has
  anything to choose.
* Bad, because the bytecode VM loses its native fast path.

### Each block has an expansion that is its meaning, and a backend may implement it natively instead

* Good, because the bytecode VM can keep its fast path.
* Bad, because a backend can choose native code only if it knows which block it
  calls, so the lowered program needs a name for every standard block at every
  width, and every backend must handle each name.
* Bad, because native code and expansions must be kept in step by tests.

### Each backend implements every block natively

* Good, because each backend is as fast as it can be.
* Bad, because each backend learns what a block does from another backend's
  code, and the implementations can drift apart.

### Standard blocks as library source

Written in Structured Text and shipped like a compatibility library.

* Good, because a block would be ordinary source.
* Bad, because Structured Text cannot read the time of the round or declare
  hidden fields without extending the language.
