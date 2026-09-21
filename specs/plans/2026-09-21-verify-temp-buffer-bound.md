# Verify the temp buffer pool bound against the shipped bytecode

Closes the residual risk [ADR-0052](../adrs/0052-temp-string-buffers-released-on-consume.md)
records: the container header's `num_temp_bufs` is derived from the
emitter's own bookkeeping, and nothing checks it against the bytecode that
actually ships.

## Goal

A container whose declared temp buffer pool is too small for its own
bytecode is rejected at compile time, with the function and byte offset of
the instruction that would have overrun it — instead of running and trapping
`V9009` in the field.

## Why this is a backstop, not the answer

The pool exists because the VM allocates nothing at run time
([ADR-0010](../adrs/0010-no-std-vm-for-embedded-targets.md)), so its size
must be decided at compile time. Any compile-time number can be wrong, and
this change only makes a wrong one loud.

**The design that removes the risk entirely is to have no pool.** String
operations already spill nested results into compile-time-assigned
data-region scratch slots — that is how `CONCAT(CONCAT(a,b),c)` works today,
and why nesting does not stack buffers. If a string operation's *result*
also went to a scratch slot chosen at compile time, rather than to a slot
bump-allocated from a shared pool at run time, then `num_temp_bufs`,
`max_temp_buf_bytes`, the allocator, the watermark, `V9009` and this
verifier rule all stop existing. Every buffer would be addressed the way
every other piece of data-region storage is: by an offset fixed when the
instruction was emitted.

That is a larger change than this one — it alters the meaning of `buf_idx`
in the instruction set and touches every string opcode — so it is not
attempted here. The rule this plan adds must carry that statement in its
own doc comment, so the next reader knows the check is a guard around a
design we would rather not need, not a feature to build on.

## Architecture

A new verifier rule, R0204, decided the way R0200–R0203 already are: by
abstract interpretation over each function's control-flow graph, reading
the bytecode in the container rather than any compiler-side state.

**Per function.** Walk the CFG tracking *temp buffer depth* instead of
operand-stack depth. The allocating opcodes (`LOAD_CONST_STR`,
`STR_LOAD_VAR`, `STR_LOAD_ARRAY_ELEM`, `CONCAT_STR`, `LEFT_STR`,
`RIGHT_STR`, `MID_STR`, `INSERT_STR`, `DELETE_STR`, `REPLACE_STR`, and the
`*_TO_STRING` built-ins) are +1; the consuming opcodes (`STR_STORE_VAR`,
`STR_STORE_ARRAY_ELEM`) are −1, saturating at zero as the VM's allocator
does.

Two differences from the operand-stack walk, both deliberate:

- **A merge takes the maximum, it does not conflict.** Stack depth must
  agree at a merge because the calling convention depends on it. Temp depth
  legitimately differs between arms — one may return a string and the other
  not — and what this rule needs is the worst case, so a merge that raises
  the recorded depth re-enqueues the successor.
- **Termination comes from the bound, not from convergence.** A loop body
  with net-positive temp depth has no fixpoint; depth grows each time round.
  That is exactly the defect class this rule exists to catch, and the walk
  errors as soon as depth exceeds the declared pool, so it terminates on
  such bytecode rather than looping.

**Across the call graph.** A callee's buffers sit on top of whatever its
caller holds live, so the per-function maxima are composed along the call
graph — derived here from the `CALL` / `FB_CALL` / `METHOD_CALL`
instructions in the bytecode, not from codegen's recorded edges — and the
heaviest path is compared against `num_temp_bufs`.

Deriving the graph from the bytecode is the point. Codegen already computes
this number from its own call graph; a check that reused that graph would
re-confirm codegen's arithmetic rather than its output.

## Prefactoring

The CFG machinery a second analysis needs is private to
`container/src/verify.rs`: `instruction_boundaries`, `branch_target`,
`flow_of`, `Flow`, `u16_at`, `i16_at`. Without extracting it the new rule
would duplicate the walk, which the development standards forbid.

Extract it into its own module, leaving `verify_stack_balance`
behaviourally identical. The existing verifier tests must pass unchanged;
that is the check that the extraction changed shape and not behaviour.

Size is a secondary motive and only partly served. `verify.rs` is 1258
lines, of which roughly 580 are tests, so its code is already inside the
1000-line limit and the extraction moves about 100 lines out. The reason to
do it is reuse, not the line count.

## Design doc reference

`specs/design/bytecode-verifier-rules.md`. R0204 is added to the rule index
and given a section beside R0203, which it resembles: both are "this
resource's declared bound is not exceeded". The R0200–R0299 category
description widens from the operand stack to stack-disciplined resources.

## File map

| File | Change |
|---|---|
| `compiler/container/src/cfg.rs` | New — the shared CFG walk machinery, extracted |
| `compiler/container/src/verify.rs` | Uses the extracted module; behaviour unchanged |
| `compiler/container/src/verify_temp_bufs.rs` | New — R0204 |
| `compiler/container/src/lib.rs` | Export the new entry point and error type |
| `compiler/codegen/src/stack_balance.rs` | Run the new check beside the existing one |
| `specs/design/bytecode-verifier-rules.md` | R0204 |
| `specs/adrs/0052-...md` | Amend: the residual risk it records is now covered |

## Tasks

- [x] Plan
- [ ] Prefactor: extract the CFG walk; existing tests pass unchanged
- [ ] R0204: per-function walk, call-graph composition, error type
- [ ] Wire into codegen's pre-container verification
- [ ] Tests: a hand-built container that under-declares is rejected; every
      compiled program in the corpus is accepted; the loop shape that
      caused #1590 is rejected when the pool is forced to 1 too few
- [ ] Design doc rule entry, ADR amendment
- [ ] `cd compiler && just`, `cd specs && just`
- [ ] Delete this plan
