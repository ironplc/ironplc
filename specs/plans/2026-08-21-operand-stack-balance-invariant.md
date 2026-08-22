# Operand-Stack Balance Invariant

## Goal

Make a codegen bug that leaves values on — or over-pops — the VM operand
stack fail **mechanically and immediately**, at the instruction responsible,
instead of surfacing later as a misattributed `Trap::StackOverflow`.

The deliverable is a guard that keeps holding as new language capabilities
land, not a fix for one defect.

## Background: what is missing today

Three facts about `main`, each verified against the source:

1. **The compiler computes the needed information and discards it.**
   `compiler/codegen/src/emit.rs` maintains `current_stack_depth` beside
   `max_stack_depth`, adjusted by `push_stack` / `pop_stack`. Nothing ever
   inspects it when a function is finalized
   (`compiler/codegen/src/compile.rs`, `finalize_function`).

2. **The VM never resets the operand stack.** It is a long-lived field on
   `VmReady` / `VmRunning`, constructed once in `Vm::load` and passed by
   `&mut` into execution. `compiler/vm/src/stack.rs` exposes
   `new/push/pop/peek/peek_at/truncate_by/dup/swap` — no `clear`, no
   `reset` — and `run_round` does not truncate. A leaked slot therefore
   survives across scan rounds and accumulates without bound.

3. **The only existing detector is calibrated by the assumption it should
   be checking.** `compiler/vm/src/buffers.rs` sizes the operand-stack
   buffer to exactly the container header's `max_stack_depth`, and that
   number comes from the emitter's own high-water mark, which presumes
   balanced call sites. An imbalance therefore both overflows the buffer
   *and* under-declares it, and the resulting `Trap::StackOverflow` fires
   roughly `max_stack_depth` calls after the defect.

The existing test suite cannot see any of this: `parse_and_run`
(`compiler/codegen/tests/it/common/mod.rs`) runs exactly one scan round and
asserts only on variable values, and a leaked stack slot changes no
variable.

## Architecture

`specs/design/bytecode-verifier-rules.md` already specifies the mechanism
this work needs — abstract interpretation over the bytecode, with

| Rule | Claim |
|------|-------|
| R0200 | Stack depth agrees at every control-flow merge point |
| R0202 | No instruction pops from an empty stack |
| R0203 | Depth never exceeds the declared `max_stack_depth` |

That verifier was designed but never implemented. This plan implements its
**stack-discipline subset** rather than inventing a parallel mechanism, and
adds the balance rule those three do not state: every path leaving a
function must leave the stack at the depth the calling convention promises.
The type rules (R0201, R0300–R0303), the control-flow rules (R0401, R0402,
R0404), and the operand-bounds rules (R0002, R0100–R0102) stay unimplemented
and out of scope.

### Layer 1 — compile-time verification

A new `ironplc_container::verify` module walks each emitted function's
control-flow graph and reports the first violation. Codegen calls it on the
finished `Container` before returning, mapping any violation to
`Diagnostic::internal_error_at` (P9998) — an imbalance is a compiler bug,
not a program error.

**Why abstract interpretation and not a cheap end-of-function assert.**
The task's own framing calls this out, and the cheap check is not merely
weaker — it is *wrong on code that exists today*. `current_stack_depth` is a
linear counter over emission order, so for this valid program:

```iecst
FUNCTION pick : DINT
  VAR_INPUT a : DINT; END_VAR
  IF a > 0 THEN pick := 1; RETURN; END_IF;
  pick := 2;
END_FUNCTION
```

codegen emits (decoded from the actual container):

```
 0  LOAD_CONST_I32 pool 0        ; pick := 0 (implicit init)
 3  STORE_VAR_I32  var 2
 6  CMP_BR_I32     a <= 0 -> 22
14  LOAD_CONST_I32 pool 1        ; pick := 1
17  DUP                          ; store-load peephole
18  STORE_VAR_I32  var 2
21  RET                          ; early RETURN
22  LOAD_CONST_I32 pool 2        ; pick := 2
25  DUP
26  STORE_VAR_I32  var 2
29  RET                          ; function end
```

The linear counter reaches `RET` at offset 21 with depth 1, then *keeps
walking* into the second arm and ends the function at **2**. A
`current_stack_depth == 0` assert fails here on correct code. Raising the
expected value does not help either, because the number depends on how many
early `RETURN`s a function happens to contain.

The counter is also unsound in the other direction: it sums both arms of an
`IF` as though they ran back to back, so a leak in one arm and an over-pop
in another cancel to zero. And `pop_stack` uses `saturating_sub`, so an
over-pop clamps at zero and is invisible even in principle.

Walking the CFG makes all three cases exact — and, the point of the
exercise, derives the answer from *the bytecode that ships*, independently
of the bookkeeping the emitter did on the way there. Where the emitter's
counter is the thing under suspicion, it cannot also be the witness.

**Model.** Each function is verified alone with an entry depth of 0. That is
sound because the calling convention isolates frames: `CALL` pops the
callee's arguments before pushing the callee's frame, so a callee never
observes slots belonging to its caller; `FB_CALL` leaves the caller's
`fb_ref` in place and runs the body as its own frame. Exit depths are 1 for
`RET` (the return value the caller's `CALL` accounts for), 0 for `RET_VOID`,
and 0 for falling off the end of a body — which the VM treats as `RET_VOID`.

**Placement.** The verifier lives in the `container` crate, beside
`opcode::instruction_size` and `opcode::builtin::arg_count` — the two tables
its stack-effect model must stay consistent with. It is invoked from
codegen, not from `ContainerBuilder::build()`: the VM's own tests hand-build
containers with deliberately malformed bytecode, and making `build()`
fallible would ripple through ~160 call sites to no benefit.

**Keeping it honest as opcodes are added.** The stack-effect match is
exhaustive over the assigned opcode space with no catch-all, and a unit test
asserts that every opcode `opcode::is_assigned` accepts has a defined
effect. A new opcode wired into `instruction_size` but not into the effect
table fails that test instead of being silently verified as a no-op.

### Layer 2 — test-harness enforcement

`parse_and_run`, `parse_and_try_run`, and `parse_and_run_rounds` assert the
operand stack is empty when a scan completes. Because the codegen e2e suite
compiles and executes hundreds of programs, every one of them becomes a
balance regression test — including tests written years from now by someone
who has never heard of this issue.

This needs a stack-depth accessor on the VM. The chosen form is a plain
`pub fn operand_stack_depth(&self) -> usize` on `VmReady` / `VmRunning`,
which costs nothing unless called and is useful to debuggers and embedders
besides. A `#[cfg(test)]` accessor would not work at all here: the codegen
tests are a separate crate and link the VM as a normal dependency, so a
test-only item in `ironplc-vm` is not visible to them.

`parse_and_run_rounds` hands a `&mut VmRunning` to a closure that may run
many rounds. Round attribution is recovered by asserting inside `drive_fb`'s
`Run` and `Pulse` steps, which is where nearly all multi-round scenarios
live. Detection does not depend on that: because nothing ever truncates the
operand stack, a leak in any round survives to the end of the scenario, so
the check after the closure catches a leak in every round of it.

### Layer 3 — runtime defense: rejected, with evidence

**No stack-balance check is added to the VM.** The first attempt put a
`debug_assert!` at the end of `run_round`, on the reasoning that
`debug_assertions` is off in release so the production scan path stays
byte-identical. Running the suite disproved the idea outright — it broke two
VM tests, and both breakages are correct:

- `proptest_robustness::execute_when_arbitrary_bytecode_then_never_panics`
  feeds up to 512 random bytes into `run_round` and pins the contract that
  arbitrary bytecode must **never panic the VM**. A `debug_assert` in
  `run_round` *is* a panic on arbitrary bytecode. The two requirements are
  irreconcilable, and the robustness contract is the more important one:
  containers reach the VM from disk, not only from this compiler.
- `execute_dup_swap::execute_when_dup_and_swap_combined_then_correct` runs a
  hand-written fixture that leaves one slot behind. It is a test fixture, not
  codegen output, and it is entitled to be unbalanced.

The mistake was a category error. "A completed scan leaves the operand stack
empty" is a property of *the compiler that produced the container*, not one
the VM can guarantee about every container it is handed. Asserting it inside
the VM asserts something the VM does not know. Enforcement belongs where
provenance is known: codegen, which verifies what it emits, and the codegen
test harness, which only ever runs containers codegen produced.

Feature-gating the check on `test-support` was considered and does not work:
`vm/Cargo.toml` self-enables that feature in its own dev-dependencies, so the
VM's robustness tests would still see it.

`Trap::StackNotEmpty` is rejected for a separate reason: it converts a
compiler bug into a runtime fault for the end user, which is the failure mode
this work exists to remove.

What the VM does gain is the read-only `operand_stack_depth()` accessor that
layer 2 needs — no branch, no cost, and useful to debuggers and embedders.

Load-time verification — calling `verify_stack_balance` when the VM loads a
container, which is what `bytecode-verifier-rules.md` actually envisages — is
the principled way to give the VM this guarantee about untrusted containers.
It is out of scope here and left as follow-up.

## Design doc reference

`specs/design/bytecode-verifier-rules.md` — rules R0200, R0202, R0203, and
the abstract-interpretation algorithm in *Verification Algorithm*.

That doc is not in any crate's `build.rs` spec-conformance list, and this
plan does not add it: enforcement is bidirectional, so listing the doc would
require a conformance test for all ~24 of its rules, not just the four this
work implements. Adding requirement IDs and wiring the doc into
`spec_requirements_gen` is left as follow-up for whoever implements the
remaining rules.

## File map

**New**

| File | Purpose |
|------|---------|
| `compiler/container/src/verify.rs` | CFG abstract interpretation, `StackImbalance` |
| `compiler/codegen/src/stack_balance.rs` | Codegen entry point plus the Emitter-driven proof tests (the `emit` module is crate-private, so the proof cannot live in `tests/it`) |

**Modified**

| File | Change |
|------|--------|
| `compiler/container/src/opcode.rs` | `arg_count_opt` — non-panicking `arg_count` for the verifier |
| `compiler/container/src/lib.rs` | Export `verify` |
| `compiler/codegen/src/compile.rs` | Verify the container before returning it |
| `compiler/codegen/tests/it/common/mod.rs` | Assert empty stack after a scan |
| `compiler/vm/src/vm.rs` | `operand_stack_depth` accessor on `VmReady`/`VmRunning` |
| `compiler/vm/src/stack.rs` | `len` accessor |

## Tasks

- [x] Add `opcode::builtin::arg_count_opt`; re-express `arg_count` on top of it
- [x] Write `container/src/verify.rs`: boundary scan, worklist interpretation,
      `StackImbalance` with `Display`
- [x] Unit-test the verifier: balanced, leak, over-pop, merge conflict,
      early-`RET`, loop back edge, unreachable code, bad jump target,
      truncated instruction, unassigned opcode, `CALL` parameter accounting
- [x] Add the exhaustiveness test tying `effect_of` to `opcode::is_assigned`
- [x] Call the verifier from `compile_program_with_functions`; map violations
      to `Diagnostic::internal_error_at`
- [x] Confirm zero false positives across the whole existing suite
      (`RETURN`, `EXIT`, `CASE`, nested calls, function blocks)
- [x] Add `OperandStack::len` and `VmReady`/`VmRunning::operand_stack_depth`
- [x] Evaluate a runtime check in `run_round`; reject it if it conflicts
      with the VM's arbitrary-bytecode robustness contract
- [x] Assert an empty stack in `parse_and_run` / `parse_and_try_run` /
      `parse_and_run_rounds`
- [x] Write the main-only reproduction in `codegen/src/stack_balance.rs`:
      drive `Emitter` + `ContainerBuilder`
      to produce a spare push and an over-pop, and assert each is caught and
      that the pre-existing `max_stack_depth` path accepted both
- [x] Run `cd compiler && just` and make every check pass

## Verification

1. `cargo test --workspace` — no regressions, no false positives.
2. The reproduction tests pin the verifier itself: each drives the real
   `Emitter` and `ContainerBuilder`, and each asserts both that the defect
   is caught now and that the pre-existing machinery accepted it. The
   `compile.rs` wiring is pinned indirectly — any real codegen leak trips
   the harness assertion in the ~1090-test e2e suite.
3. `cd compiler && just` — compile, coverage ≥ 85%, clippy, fmt, dupes.
