# Every String Has an Explicit Capacity

status: proposed
date: 2026-10-07

## Context and Problem Statement

A string has a capacity: the most characters its value can hold. Today that
capacity is not always stated:

* **The default.** A string declared without a length holds 254 characters,
  and that default is applied in more than one place. CODESYS and TwinCAT use
  80, so a dialect would want to set it.
* **Intermediate results.** The capacity of a result such as that of `CONCAT`
  is a bound code generation works out, and where it knows none it uses the
  default.
* **Temporary storage.** The bytecode VM gives every temporary string buffer
  the size of the largest string in the program.

A second backend would have to apply the same default and the same rules, or
cut values at different lengths.

Who decides each capacity?

## Decision Drivers

* **No backend applies a default**, or sizes a string from other strings in the
  program.
* **A value is cut only** where it goes into something smaller than itself.
* **The default can become a compiler option** that a dialect sets.
* **The move to the lowered program changes no behaviour.**

## Considered Options

* Every string in the lowered program states its capacity, analysis applies the
  default, and lowering decides each intermediate result's
* The capacity is optional in the lowered program, and each backend applies the
  default
* Each backend sizes intermediate results itself

## Decision Outcome

Chosen option: "Every string in the lowered program states its capacity,
analysis applies the default, and lowering decides each intermediate
result's", because each capacity is then decided once, before any backend.

* **Every string in the lowered program states its capacity.**
* **Analysis applies the default** when it resolves a string type declared
  without one. A different default changes only what analysis records.
* **Lowering decides the capacity** of every intermediate result.
* **A backend sizes storage** for a string from the capacity the lowered
  program gives it.
* **A value is cut** only when it goes into a place or a result with a smaller
  capacity. It keeps its first characters.

The bytecode VM cannot size each temporary buffer from its value until the VM
and its container format change. That change has a design of its own
([issue 2118](https://github.com/ironplc/ironplc/issues/2118)). Until then the
bytecode backend gives every temporary buffer one size, as it does today.

### Consequences

* Good, because a dialect's default changes analysis alone.
* Good, because no two backends can disagree on how long a string can be.
* Good, because a better rule for intermediate results is a change in one
  place.
* Bad, because the bytecode VM does not size its temporary buffers from each
  value until issue 2118 is done.

## Pros and Cons of the Options

### Every string in the lowered program states its capacity (chosen)

* Good, because no backend needs a default or a sizing rule.
* Bad, because lowering must give every intermediate result a capacity.

### The capacity is optional, and each backend applies the default

* Good, because it is closer to what code generation does today.
* Bad, because every backend must apply the same default, and a dialect's
  default would reach every backend.

### Each backend sizes intermediate results itself

* Good, because nothing changes today.
* Bad, because each backend repeats the rule, and two can cut a value at
  different lengths.
