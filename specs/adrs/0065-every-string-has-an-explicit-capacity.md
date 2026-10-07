# Every String Has an Explicit Capacity

status: proposed
date: 2026-10-07

## Context and Problem Statement

A string has a capacity: the most code units its value can hold. Today that
capacity is not always stated:

* **The default.** A string declared without a length has 254 code units
  (`DEFAULT_STRING_MAX_LENGTH`), applied by codegen and by the analyzer's
  `slot_count`. CODESYS and TwinCAT use 80, so a dialect would want to set
  it.
* **Intermediate results.** Codegen computes a bound for each string
  expression (`string_width.rs`): a declared capacity, a literal's length,
  `m + n` for `CONCAT` of a `STRING[m]` and a `STRING[n]`. The bound is
  optional, and where codegen knows none it uses 254.
* **Temporary buffers.** The VM's pool of temporary buffers is sized once, by
  two container header fields
  ([ADR-0052](0052-temp-string-buffers-released-on-consume.md)), so every
  buffer has the size of the largest string in the program.

A second backend would have to apply the same default and the same sizing
rules, or cut values at different lengths.

Who decides each capacity?

## Decision Drivers

* **No backend applies a default** or sizes a string from other declarations.
* **A value is cut only** where it goes into a place or a result smaller than
  itself.
* **The default can become a compiler option** that a dialect sets.
* **The move changes no behaviour.**

## Considered Options

* Every string in the IR states its capacity; the analyzer applies the
  default; lowering decides each intermediate result's
* The capacity is optional in the IR, and each backend applies the default
* Each backend sizes intermediate results itself, as codegen does today

## Decision Outcome

Chosen option: "Every string in the IR states its capacity; the analyzer
applies the default; lowering decides each intermediate result's", because
each capacity is then decided once, before any backend.

* **A declared string** has the capacity its declaration gives. The analyzer
  applies the default when it resolves a type declared without one, so every
  string type it records has a capacity.
* **The default** is a compile-time choice. Changing it changes only what the
  analyzer records. Making it a compiler option is out of scope, but stays
  possible.
* **An intermediate result** has the capacity lowering gives it. At first that
  is the bound codegen computes for it today, so the move is behaviour
  preserving. A later change to that rule changes lowering and no backend.
* **A backend sizes storage** for a string value from the capacity the IR
  gives that value.
* **A value is cut** only when it goes into a place or an intermediate result
  whose capacity is smaller. It keeps its first code units.

The bytecode VM cannot size its temporary buffers this way yet: its pool gives
every buffer one size. Sizing each buffer from the value it holds is a change
to the container format and the VM, with a design of its own,
[issue 2118](https://github.com/ironplc/ironplc/issues/2118). Until then the
bytecode backend keeps one size for every buffer, as it does today.

### Consequences

* Good, because a dialect's default changes the analyzer alone.
* Good, because no two backends can disagree on how long a string can be.
* Good, because a better rule for intermediate results is a change in one
  place.
* Bad, because the bytecode backend does not size its temporary buffers from
  each value until issue 2118's container change lands.

### Confirmation

The requirements of `specs/design/lowered-program.md` confirm this decision:

* Every string type the analyzer records has a capacity
  (REQ-LOW-analyzer-053).
* Every `StringShape` in the IR states a capacity (REQ-LOW-lowering-054).
* A value that goes into something smaller keeps its first code units
  (REQ-LOW-codegen-056).
* A backend sizes storage for a value from that value's capacity
  (REQ-LOW-codegen-055), which the bytecode backend meets once issue 2118
  lands.

## Pros and Cons of the Options

### Every string in the IR states its capacity (chosen)

* Good, because no backend needs a default or a sizing rule.
* Bad, because lowering must give every intermediate result a capacity.

### The capacity is optional in the IR, and each backend applies the default

* Good, because the IR stays closer to codegen's `StringShape` today.
* Bad, because every backend must apply the same default, and a dialect's
  default would reach every backend.

### Each backend sizes intermediate results itself

* Good, because nothing changes today.
* Bad, because each backend repeats the rule, and two can cut a value at
  different lengths.

## More Information

The design is `specs/design/lowered-program.md`, section "String capacity".
