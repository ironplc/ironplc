# A Call Through an Interface Lists Its Implementers

status: proposed
date: 2026-10-07

## Context and Problem Statement

A call through an interface runs the method of whichever function block the
interface value refers to. Codegen does not compile such a call today.
[ADR-0041](0041-staged-method-and-interface-dispatch.md) decided static
dispatch and deferred dynamic dispatch, through references, pointers and
interfaces, to an ADR of its own.
[Pull request 1870](https://github.com/ironplc/ironplc/pull/1870) designs how
the bytecode VM would dispatch.

The IR must be able to express such a call. Each backend dispatches in its own
way: the VM through a table in the container, a native backend through a
switch or a table of functions. Every implementer is in the program the
compiler sees, because compatibility libraries are merged before analysis and
there is no separate compilation.

How does the IR express a call through an interface?

## Decision Drivers

* **Target neutral:** the IR does not fix a dispatch mechanism.
* **One meaning:** the method of the value's concrete type runs, with the
  value's instance as its receiver, and a null value traps.
* **A direct call where only one type is possible.**
* **No new kind of statement.**

## Considered Options

* A call lists the implementers; a value that can hold only one type is a
  reference to it
* A dispatch table in the IR
* A `Case` on a type id, with a direct call in each arm

## Decision Outcome

Chosen option: "A call lists the implementers; a value that can hold only one
type is a reference to it", because it fixes the meaning and leaves the
mechanism to each backend.

* **The analyzer records** the concrete types each interface value can hold,
  following the instances assigned or passed to it. It makes this decision
  because it decides whether a program that needs dynamic dispatch is
  reported when dynamic dispatch is not enabled.
* **One possible type.** Lowering gives the variable the type of a nullable
  reference to that function block, and a call through it is a direct method
  call on the instance it points at. A null value traps there.
* **Several possible types.** The call is a `Call` whose callee is
  `Callee::Interface`. It carries the interface value and, for every concrete
  type the value can hold, that type's method. Lowering makes that list from
  the whole program. The arguments do not include a receiver.
* **Interface values.** `InterfaceOf` makes one from an instance place, `Null`
  is the null value, and two values are equal when they refer to the same
  instance.
* **The mechanism is each backend's.** For the bytecode VM it is what pull
  request 1870 decides.

This does not decide whether or when dynamic dispatch is enabled; that stays
with ADR-0041. `__QUERYINTERFACE` and `__QUERYPOINTER` are out of scope.

### Consequences

* Good, because each backend can dispatch in the way that suits its target.
* Good, because the common case, a value that can hold only one type, is a
  direct call on every target.
* Bad, because lowering needs the whole program to list the implementers.
  Separate compilation would need another representation.

### Confirmation

The requirements of `specs/design/lowered-program.md` confirm this decision:

* Analysis records the concrete types each interface value can hold
  (REQ-LOW-analyzer-107).
* A value that can hold only one type lowers to a direct call, and any other
  call to a `Callee::Interface` that lists every implementer
  (REQ-LOW-lowering-104, REQ-LOW-lowering-105).
* A call through an interface runs the method of the value's concrete type,
  and traps when the value is null (REQ-LOW-codegen-106).

## Pros and Cons of the Options

### A call lists the implementers (chosen)

* Good, because the IR states what the call means and nothing about how it
  dispatches.
* Bad, because each backend implements the dispatch itself.

### A dispatch table in the IR

* Good, because every backend would dispatch the same way.
* Bad, because it fixes a mechanism that suits some targets and not others.

### A `Case` on a type id, with a direct call in each arm

The shape pull request 1870 proposes for the VM, written in the IR.

* Good, because it needs no new callee.
* Bad, because it fixes the VM's mechanism in the IR, and needs a type for the
  value's parts and a way for an arm to treat the instance as one implementer.

## More Information

The design is `specs/design/lowered-program.md`, section "Calls through an
interface".
