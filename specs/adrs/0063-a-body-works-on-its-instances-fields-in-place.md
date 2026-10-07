# A Body Works on Its Instance's Fields in Place

status: proposed
date: 2026-10-07

## Context and Problem Statement

A program, function block or method body reads and writes the fields of its
instance. The IR reaches a field through the body's instance parameter, so
`x` in the body of `MyFb` is the place `this^.x`, the same field as `inst.x`
outside it. The IR still has to say what that field is while the body runs.

The bytecode VM copies an instance's fields into slots that belong to the
function block type before the body runs (`FB_CALL` and `METHOD_CALL` in
`vm/src/vm.rs`), and copies them back when the body returns
(`handle_frame_return`). Every instance of the type reuses those slots, and a
VM reference can only name such a slot. Reading the VM's code shows three ways
this differs from working on the instance itself:

* **References.** A reference to a field that outlives the body reads and
  writes whichever instance of the type ran last.
* **Traps.** A trap inside a body discards the writes the body made.
* **Shared slots.** A body that reached another instance of its own type would
  overwrite its own copy.

P2029, "No REF of ephemeral variables"
(`specs/design/ref-to.md`), accepts a reference to a function block's field
because the field is persistent. With the copy, the reference names the
type's shared slot, not the instance's field.

What does an instance's field mean while its body runs?

## Decision Drivers

* **Safety:** a reference must never be able to outlive what it names
  ([ADR-0005](0005-safety-first-design-principle.md)).
* **One meaning on every target.**
* **What other targets do naturally:** WebAssembly and LLVM address an
  instance's fields where they are.
* **The refactor itself changes no behaviour.**

## Considered Options

* Fields in place is the meaning, and the VM's copy is removed by a change of
  its own
* The VM's copy is the meaning
* Fields in place, with a stop-gap in the bytecode backend until the VM changes

## Decision Outcome

Chosen option: "Fields in place is the meaning, and the VM's copy is removed by
a change of its own", because it is the only option in which a reference to a
field cannot outlive what it names on any target.

* **Each instance has one storage for its fields**, which lives as long as the
  program.
* **A write is visible at once** through every path to the field.
* **A reference to a field stays valid** for the life of the program, whether
  it was taken inside the body (`REF(x)`) or outside it (`REF(inst.x)`).
* **A method called on the current instance**, through `THIS^` or `SUPER^`,
  works on the same fields.
* **When a body traps,** the writes it made before the trap stay, as writes to
  globals do.

The bytecode VM does not do this yet. Removing the copy is a VM change with a
design of its own, [issue 2120](https://github.com/ironplc/ironplc/issues/2120).
Until it lands:

* The bytecode backend keeps the copy for every statement, whichever route
  compiles it, so the refactor changes none of today's behaviour.
* Every statement of one body reaches its instance's fields the same way. A
  function block type switches to fields in place only once every statement
  of its bodies takes the lowered route.
* No stop-gap is planned.

### Consequences

* Good, because a reference to a field is safe on every target that meets the
  meaning, and P2029's premise holds.
* Good, because WebAssembly and LLVM backends meet the meaning without extra
  work.
* Bad, because the bytecode backend does not meet the meaning until issue 2120
  lands, and until then the three differences above remain as they are today.

### Confirmation

The requirements of `specs/design/lowered-program.md` confirm this decision
once issue 2120 lands:

* A body reads and writes its instance's fields in place, and a reference to a
  field stays valid for the life of the program (REQ-LOW-codegen-037).
* When a trap ends the round, every write made before it stays, including
  writes to the fields of an instance whose body was running
  (REQ-LOW-codegen-038).
* Inside a body, every access to a field of the current instance is a place
  rooted at the body's instance parameter (REQ-LOW-lowering-034).

## Pros and Cons of the Options

### Fields in place is the meaning; the VM's copy is removed by a change of its own (chosen)

* Good, because the meaning is safe and matches what other targets do.
* Bad, because the bytecode VM lags the meaning until issue 2120.

### The VM's copy is the meaning

* Good, because the VM already does it.
* Bad, because a reference to a field could outlive what it names, and every
  other backend would have to copy fields too, to match.

### Fields in place, with a stop-gap until the VM changes

For example, rejecting a reference to a field that may outlive its body.

* Good, because the unsafe case would be closed sooner.
* Bad, because the stop-gap is work that issue 2120 deletes, and it changes
  behaviour during a refactor meant to change none.

## More Information

The design is `specs/design/lowered-program.md`, sections "Places" and
"Instance fields".
