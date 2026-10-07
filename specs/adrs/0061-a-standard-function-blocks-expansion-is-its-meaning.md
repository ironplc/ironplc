# A Standard Function Block's Expansion Is Its Meaning

status: proposed
date: 2026-10-07

## Context and Problem Statement

[ADR-0003](0003-plc-standard-function-blocks-as-intrinsics.md) made the
standard function blocks VM intrinsics: `FB_CALL` recognizes a standard type
id and runs native code. What each block does is stated only by that code
(`vm/src/intrinsic.rs`):

* **Hidden state.** The VM's `TON` keeps the time it started and whether it is
  running in fields 4 and 5 of its instance, which the analyzer's `TON` does
  not declare.
* **Time.** The timers read the time of the round, which reaches only the VM's
  intrinsics today.
* **Widths.** `CTU_INT`, `CTU_DINT`, `CTU_UDINT`, `CTU_LINT` and `CTU_ULINT`
  all run one intrinsic, which reads and writes `PV` and `CV` as signed 32-bit
  values.

A second backend would need a second implementation of all ten blocks, written
from the VM's code. That is the duplication the IR exists to remove
([ADR-0057](0057-backends-consume-a-target-neutral-lowered-program.md)).

Where is a standard function block's meaning stated?

## Decision Drivers

* **One meaning for each block**, shared by every backend.
* **A backend may keep a fast native implementation.**
* **Hidden state is visible** as named fields a debugger can show.
* **Time is read one way** on every target.

## Considered Options

* An expansion in `ironplc-ir` is the meaning; a backend implements a block
  natively or by expanding it
* Each backend implements every block natively
* Standard blocks as library source

## Decision Outcome

Chosen option: "An expansion in `ironplc-ir` is the meaning; a backend
implements a block natively or by expanding it", because it states each block
once while letting the VM keep its fast path.

* **Each `StandardBlock` has an expansion** in `ironplc-ir`: lowered
  statements over the fields of its instance.
* **Hidden state is synthesized fields** of the instance, with section
  `Internal` and a name a debugger can show, as the analyzer already declares
  the hidden `M` of `R_TRIG`.
* **Time is `RoundTime`**, the time the host gave the current round. No source
  program names it.
* **A backend chooses.** It implements `Block::Standard` natively, as the
  bytecode VM keeps doing, or calls the expansion, as an LLVM backend would,
  compiling it like a user block.
* **A native implementation must match the expansion** for the same inputs and
  round times.

This extends ADR-0003 and does not supersede it: the VM still runs standard
blocks natively through `FB_CALL`, and its code is now checked against the
expansion.

### Consequences

* Good, because a second backend gets every standard block by expanding it.
* Good, because each block's state is a named field.
* Good, because each typed variant needs an expansion of its own
  ([ADR-0062](0062-one-intrinsic-variant-per-operation-and-operand-type.md)),
  so `CTU_LINT` can no longer inherit `CTU_INT`'s width unnoticed.
* Bad, because ten expansions must be written and tested.
* Bad, because the VM's native code and the expansions must be kept in step,
  by test.

### Confirmation

The requirements of `specs/design/lowered-program.md` confirm this decision:

* `ironplc-ir` provides an expansion for every `StandardBlock`
  (REQ-LOW-ir-077).
* A test runs each block both ways, through the VM's intrinsic and through its
  expansion, over the same inputs and round times, and compares the visible
  fields (REQ-LOW-codegen-078).
* A synthesized field has section `Internal`, a unique name and a span
  (REQ-LOW-lowering-028).

## Pros and Cons of the Options

### An expansion in `ironplc-ir` is the meaning (chosen)

* Good, because the meaning is one piece of code every backend can run.
* Bad, because the VM keeps a second implementation, which only a test keeps
  equal.

### Each backend implements every block natively

* Good, because each backend is as fast as it can be.
* Bad, because each backend reads the VM's code to learn what a block does,
  and the implementations can drift apart.

### Standard blocks as library source

Written in Structured Text and shipped like a compatibility library.

* Good, because a block would be ordinary source.
* Bad, because Structured Text cannot read the time of the round or declare
  hidden fields, and [ADR-0042](0042-library-functions-over-compiler-intrinsics.md)
  keeps the IEC 61131-3 standard surface in the compiler.

## More Information

The design is `specs/design/lowered-program.md`, sections "Synthesized state"
and "Callees, arguments and intrinsics".
