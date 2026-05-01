# Plan: Elide Per-Iteration Exit `JMP` in FOR Loops

## Context

FOR-loop bodies are roughly 200x slower than native code in IronPLC. The
`vm-performance.md` roadmap lists several Tier-1 levers (verifier-gated
unchecked execution, superinstructions, register translation), but each
either requires `unsafe` paths in the VM or new opcodes. The opcode set
is currently in flux, and we want to defer `unsafe` execution paths
until the bytecode verifier lands. That filter narrows the next win to
**codegen-level FOR-loop emission**, where the recent TRUNC-elision
work (`specs/plans/2026-04-30-elide-for-loop-trunc.md`) already cut one
opcode per iteration.

Today `compile_for` (`compiler/codegen/src/compile_stmt.rs:891-992`)
emits at the top of every iteration:

```text
LOAD_VAR control
compile(to)
GT  (or LT for negative step)
JMP_IF_NOT body_label
JMP end_label
body_label:                  ; bound on the very next instruction
  ...body...
```

The `JMP_IF_NOT body_label` jumps 3 bytes forward to the instruction
that already comes next — a near-no-op branch — and the unconditional
`JMP end_label` after it executes on every successful iteration. For a
100-iteration loop that's 99 wasted `JMP` dispatches through the VM's
~96-arm match (`compiler/vm/src/vm.rs`); for nested loops the cost
compounds.

## Approach

Invert the loop's continuation predicate so the *survival* condition is
on the stack, then exit the loop in a single conditional branch and let
the body fall through. The new emission is:

```text
LOAD_VAR control
compile(to)
LE  (or GE for negative step)   ; continue while i <= to
JMP_IF_NOT end_label            ; exit when continuation fails
  ...body...
```

Per iteration this removes one `JMP` dispatch, cuts 3 bytes of bytecode
per FOR loop, and does not change observable behavior: `LE` is the
exact logical negation of `GT` over IEC integer/real types, and likewise
`GE` for `LT`. (Both `from`-then-`to` and `step` are already required
to be constant, so no surprises around side-effecting bounds.)

### Reused helpers

All required infrastructure already exists:

- Comparison emitters: `emit_le`, `emit_ge` in
  `compiler/codegen/src/compile_expr.rs:1740,1762`.
- Comparison opcodes for every integer/real width:
  `LE_I32`/`GE_I32` (`compiler/container/src/opcode.rs:64,72`),
  `LE_I64`/`GE_I64` (`:467,475`), `LE_U32`/`GE_U32` (`:485,493`),
  `LE_U64`/`GE_U64` (`:501,509`), `LE_F32`/`GE_F32` (`:527,535`),
  `LE_F64`/`GE_F64` (`:553,561`).
- `JMP_IF_NOT` opcode: `compiler/container/src/opcode.rs:130`.

No new opcodes. No VM changes. No verifier work. No `unsafe`.

## Changes

### `compiler/codegen/src/compile_for` (lines ~935-988)

1. Remove `let body_label = emitter.create_label();` (line 936) and the
   matching `emitter.bind_label(body_label);` call (line 951).
2. In the loop-head emission (lines 943-948):
   - Change `StepSign::Positive` arm from `emit_gt` to `emit_le`.
   - Change `StepSign::Negative` arm from `emit_lt` to `emit_ge`.
   - Replace the `emit_jmp_if_not(body_label)` + `emit_jmp(end_label)`
     pair with a single `emit_jmp_if_not(end_label)`.
3. Update the docstring at lines 873-889 to reflect the new shape.

The increment block (lines 957-986) and the END label binding (line
989) are unchanged.

### Tests

- New unit test alongside the existing `for_loop_trunc_*` tests in
  `compiler/codegen/`: assert that for a representative
  `FOR i:DINT := 1 TO 10 DO i := i; END_FOR`, the emitted bytecode
  contains:
  - exactly **one** `JMP` (the back-edge),
  - exactly **one** `JMP_IF_NOT` (targeting END),
  - `LE_I32` (not `GT_I32`) at the loop head.
- Symmetric test for negative step asserting `GE_I32` at the loop head.

### Behavior verification

- The existing FOR-loop integration tests and the `profile_for_loop`
  benchmark in `compiler/benchmarks/tests/` already cover positive-step
  loops, zero-iteration loops (`from > to`), and the boundary case
  `FOR i:INT := 32760 TO 32767`. They must continue to pass unchanged.

## Verification

1. `cd compiler && just compile` — must succeed.
2. `cd compiler && just test` — full test suite (including any added
   asserts above) green.
3. `cd compiler && just` — full CI pipeline (clippy, fmt, coverage)
   green per `CLAUDE.md` requirement before PR.
4. Re-run `compiler/benchmarks/tests/profile_for_loop.rs` with
   profiling: total `JMP` opcode count for a 100-iteration loop should
   drop by ~99.
5. `compiler/benchmarks/benches/st_benchmark.rs` — `st_for_loop` and
   `st_nested_loops` should show a small but measurable improvement.

## Out of scope

- Verifier + verified-unchecked execution (Tier 1, blocked on `unsafe`).
- Superinstructions / fused increment / register translation (require
  new opcodes).
- String-op `copy_from_slice` (high ROI, but unrelated to FOR loops;
  natural follow-up).
