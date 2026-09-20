# Release Temporary String Buffers When They Are Consumed

status: accepted
date: 2026-09-20

## Context and Problem Statement

A string-producing operation writes its result into a temporary buffer drawn
from a pre-allocated pool and pushes that buffer's small `buf_idx` onto the
operand stack. The pool is sized once, at compile time, by two header fields
(`num_temp_bufs`, `max_temp_buf_bytes`), because the VM targets embedded
systems and allocates nothing at run time
([ADR-0010](0010-no-std-vm-for-embedded-targets.md)).

Three parts of the design each assumed a different buffer lifetime, and
nothing forced them to agree:

| Part | Lifetime it assumed |
|---|---|
| [ADR-0017](0017-unified-data-region.md), which introduced the pool | "transient within an expression" |
| The VM's allocator | released only when the call frame returns |
| Codegen's pool sizing | one buffer per *static* string-operation site |

The VM's allocator is a bump pointer rewound only by `handle_frame_return`,
which makes the pool a per-frame arena. An arena sized by a static count
cannot survive a loop: the loop body is compiled once and counted once, but
allocates once per iteration. So any string operation inside a loop exhausted
the pool and trapped `V9009` on the second iteration, after a clean compile
([#1590](https://github.com/ironplc/ironplc/issues/1590)):

```iecst
FOR i := 1 TO 2 DO
  t := CONCAT(s, 'x');
END_FOR;
```

No pool size fixes this, because no static number bounds a dynamic trip
count. The failure was reported to users as an internal compiler error with a
request to file a bug, for a five-line program that is ordinary PLC code.

The constraint's only written statement lived in an implementation plan,
which is branch-local and deleted before merge
([development-standards.md](../steering/development-standards.md)). It is why
the bundled `Tc2_Utilities` `LREAL_TO_FMTSTR` renders digits as 34 unrolled
per-weight blocks rather than a loop.

## Decision Drivers

* A program that compiles cleanly must run. A runtime trap for ordinary
  source is the worst outcome ([ADR-0005](0005-safety-first-design-principle.md)).
* The VM allocates nothing at run time, so whatever bound the compiler
  writes into the header has to be sound for every execution of the program.
* Embedded targets pay for the pool in RAM, so a bound that is merely
  sound but loose is a real cost.
* The fix must not require the bytecode verifier to do flow-sensitive
  abstract interpretation, which
  [ADR-0034](0034-string-distinction-via-operand-typing.md) and
  [ADR-0004](0004-separate-type-families-over-polymorphic-opcodes.md) both
  rule out.

## Considered Options

* **Release a buffer when the instruction that consumes it runs.** Make the
  pool a stack whose lifetime matches the operand-stack slot holding the
  `buf_idx`, and size it by live depth rather than site count.
* **Rewind the allocator at each statement boundary.** Have codegen emit a
  marker, or the VM infer one, that resets the watermark between statements.
* **Multiply loop-resident call sites by a bound.** Keep the static count
  and have codegen scale sites inside a loop by its trip count.
* **Reference-count or garbage-collect the pool.** Track liveness at run
  time.

## Decision Outcome

Chosen option: **release a buffer when the instruction that consumes it
runs**.

A temp buffer is owned by the operand-stack slot that holds its `buf_idx`.
The instructions that consume such a slot — `STR_STORE_VAR` and
`STR_STORE_ARRAY_ELEM` — release the buffer once they have copied its
contents into the data region. Every other string opcode takes its inputs as
compile-time data-region offsets, so these two are the complete set of
consumers.

Because the operand stack is LIFO, the buffer being consumed is the one most
recently allocated, so the release is a bump-pointer decrement. The allocator
declines to move for anything that is not the top allocation, which keeps it
a stack under every input: a `buf_idx` at or above the watermark is a value a
callee returned, whose frame return already rewound past it, and rewinding
*to* it would hand the same slot out twice.

The per-frame rewind stays, now as a backstop for a buffer a body leaves
live — including the one a `STRING`-returning function leaves for its caller.

Codegen sizes the pool by tracking live depth rather than counting sites: the
emitter increments at each allocating opcode and decrements at each consuming
one, beside the emission itself, and reports the per-function maximum. Since
a callee's buffers sit on top of whatever its caller holds live, the header
value is the heaviest path through the static call graph with each function
weighted by its own maximum — the same longest-path walk that already
produces `max_call_depth`.

This is the lifetime ADR-0017 described in the first place. The pool was
always the string analogue of the operand stack; it was *sized* like a stack
and *released* like an arena, and making the release match the sizing is what
makes the static bound correct.

### Consequences

* Good, because a loop over a string operation runs: the body allocates and
  releases one buffer per iteration and contributes 1 to the bound, whatever
  the trip count.
* Good, because the pool shrinks. It is now sized by nesting rather than by
  how many string statements the program contains: a program of ten
  sequential string statements went from 15 buffers to 1, and the reported
  reproducer from 3 to 1. At the default 260-byte slot that is 3,900 bytes
  down to 260 — memory returned to the embedded targets the fixed-size pool
  exists for.
* Good, because the release is local to one instruction: no verifier
  change, no flow-sensitive analysis, no runtime bookkeeping beyond the
  watermark that already existed.
* Good, because `LREAL_TO_FMTSTR` no longer *has* to be unrolled. It is left
  unrolled here and its rewrite tracked as
  [#1748](https://github.com/ironplc/ironplc/issues/1748), so this change
  stays a fix.
* Bad, because the bound is still an over-approximation: a function's peak
  is charged to every call site on the heaviest path, not the depth actually
  live at each one.
* Bad, because a codegen defect that emits an allocating opcode without
  accounting for it still surfaces as `V9009` at run time rather than at
  compile time. Moving the accounting into the emitter narrows the window to
  the emitter itself, but nothing proves the header value against the
  bytecode the way `verify_stack_balance` proves operand-stack discipline.

### Confirmation

`compiler/codegen/tests/it/end_to_end_string_loop.rs` runs the reported
program and the other loop forms, at trip counts far above any plausible
static site count. `compiler/codegen/tests/it/compile_temp_bufs.rs` pins the
header sizing, including that a loop does not enlarge the pool and that an
uncalled function does not contribute to it. The allocator's stack discipline
is unit-tested in `compiler/vm/src/string_ops.rs`.

## Pros and Cons of the Options

### Release a buffer when the instruction that consumes it runs

* Good, because it makes the static bound correct rather than merely larger.
* Good, because the consumer set is closed: two opcodes, both of which
  already copy the value out.
* Neutral, because it relies on codegen's existing invariant that a string
  expression is spilled as soon as it completes — an invariant the operand
  stack's LIFO discipline already enforces in practice.

### Rewind the allocator at each statement boundary

* Good, because it also bounds a loop.
* Bad, because a statement boundary is not visible in the bytecode. It
  needs a new opcode or a per-statement marker, which is a container format
  change for something the consuming instruction already tells us.
* Bad, because it would release a buffer that a `STRING`-returning call
  legitimately leaves live across the boundary.

### Multiply loop-resident call sites by a bound

* Bad, because it cannot work. A `WHILE` condition, a `REPEAT` until, or a
  `FOR` with non-constant bounds has no compile-time trip count, and those
  are the ordinary cases.
* Bad, because even where a bound exists it inflates the pool by the trip
  count, which is precisely the memory an embedded target does not have.

### Reference-count or garbage-collect the pool

* Bad, because it contradicts [ADR-0010](0010-no-std-vm-for-embedded-targets.md):
  the VM allocates nothing and runs in bounded time per scan.
* Bad, because it buys nothing here. The lifetime is already statically
  evident from the instruction that consumes the value.

## More Information

* [ADR-0017](0017-unified-data-region.md) introduced the pool and stated the
  "transient within an expression" lifetime this ADR restores.
* [ADR-0035](0035-length-and-encoding-prefixed-string-layout.md) and
  [ADR-0034](0034-string-distinction-via-operand-typing.md) define the
  encoding tag a temp buffer slot carries; unchanged here.
* `specs/design/bytecode-instruction-set.md` (String Operations) and
  `specs/design/runtime-execution-model.md` (String Buffer Management)
  describe the mechanism.
