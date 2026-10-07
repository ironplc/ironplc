# Each Lowered Operation Has One Total Meaning: the Bytecode VM's

status: proposed
date: 2026-10-07

## Context and Problem Statement

The IR ([ADR-0057](0057-backends-consume-a-target-neutral-lowered-program.md))
fixes what a program does in terms of its operations: a division, a
conversion, a dereference. The instructions of each target do not agree on
the edge cases of those operations:

* WebAssembly's `i32.div_s` traps on the most negative value divided by -1,
  and `i32.trunc_f32_s` traps on a NaN. Address 0 is readable memory, so a
  null reference does not trap by itself.
* LLVM's `sdiv` is undefined for a zero divisor and for the most negative
  value divided by -1. `fptosi` gives poison for a NaN, and so does a shift by
  the operand's width or more. LLVM's optimizer may assume an undefined case
  never happens, and miscompile a program that reaches it.

The bytecode VM already defines every one of these cases. If each backend
followed its own target's instruction, the same program would compute
different values, or trap on one target and not another. In a PLC that is a
safety defect ([ADR-0005](0005-safety-first-design-principle.md)).

What does each lowered operation mean?

## Decision Drivers

* **The same program gives the same values and the same traps** on every
  target.
* **No operation is undefined** for any operand the IR can supply.
* **Programs keep the meaning they have today.**
* **Each rule is testable** by running a program.

## Considered Options

* Each operation has one total meaning, the bytecode VM's today
* Each backend follows its target's instructions
* A new meaning, chosen now for all targets

## Decision Outcome

Chosen option: "Each operation has one total meaning, the bytecode VM's
today", because it is the only option that makes every target agree without
changing what any program does.

The meaning is stated once, in section "Meaning of operations" of
`specs/design/lowered-program.md`. In outline:

* **Integers** compute at the two widths of
  [ADR-0001](0001-bytecode-integer-arithmetic-type-strategy.md) and wrap in
  two's complement. Division rounds toward zero, `MOD` takes the dividend's
  sign, and both trap on a zero divisor. The most negative value divided by
  -1 gives the most negative value.
* **Reals** follow IEEE 754. A real division by zero does not trap, and every
  comparison with a NaN is false except `<>`.
* **A conversion from a real to an integer** truncates toward zero and
  saturates, and a NaN converts to 0. A narrowing keeps the low bits.
* **A null dereference traps**, and so does an array position outside the
  array, checked on the flat position as
  [ADR-0023](0023-array-bounds-safety.md) chose.
* **The standard functions** mean what IEC 61131-3 says, with the VM's choices
  where it leaves room: a shift count is taken modulo the width, `SHR` fills
  with zeros, and `EXPT` of an integer by a negative exponent traps.
* **A trap ends the round.** No later statement or program instance runs, and
  every write made before the trap stays.

Where a target's instruction differs, the backend emits the difference: a
guard before a division, a saturating conversion, an explicit null check. An
LLVM backend sets no `nsw`, `nuw` or fast-math flag and does not mark
functions `mustprogress`.

Real functions such as `SIN` use each target's own primitive, so two targets
can differ in the last bits. They are compared with a near check rather than
bit for bit.

The program instances of a round run one at a time, each to completion, as the
VM's scheduler runs them. Tasks that preempt each other are out of scope.

### Consequences

* Good, because a second backend has one specification and one test suite to
  meet.
* Good, because an LLVM backend has no undefined behaviour to be miscompiled
  through.
* Good, because no program changes meaning.
* Bad, because some targets pay for guards and checks their own instructions
  would not need.
* Bad, because today's choices are fixed with the rest, including a `FOR`
  whose bound is its type's largest value never ending, and a bounds check on
  the flat position only. Changing one is a new decision that changes every
  backend at once.

### Confirmation

Each requirement in section "Meaning of operations" of
`specs/design/lowered-program.md` (REQ-LOW-codegen-083 to REQ-LOW-codegen-089
and REQ-LOW-codegen-096 to REQ-LOW-codegen-099) has an end-to-end program that
exercises it, such as the most negative `DINT` divided by -1, `REAL_TO_DINT`
of a NaN and `SHL` by 33. Every backend runs the same programs. A real value
passes within the helpers' stated tolerance (REQ-LOW-codegen-132).

## Pros and Cons of the Options

### Each operation has one total meaning, the bytecode VM's today (chosen)

* Good, because the VM's behaviour is already what users rely on.
* Bad, because it records some behaviour that a fresh choice would make
  differently.

### Each backend follows its target's instructions

* Good, because each backend emits the fastest instruction.
* Bad, because the same program behaves differently by target, and LLVM may
  miscompile a program that reaches an undefined case.

### A new meaning, chosen now for all targets

For example, trapping on integer overflow.

* Good, because each choice could be made on its merits.
* Bad, because it changes what existing programs do, in the same step as a
  refactor that is meant to change nothing.

## More Information

The design is `specs/design/lowered-program.md`, section "Meaning of
operations", which also lists how WebAssembly and LLVM differ and what each
backend emits.
