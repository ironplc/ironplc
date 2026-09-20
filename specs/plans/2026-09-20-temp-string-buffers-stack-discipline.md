# Release temporary string buffers when they are consumed

Fixes [#1590](https://github.com/ironplc/ironplc/issues/1590): any string
operation inside a loop traps `V9009` on the second iteration.

## Goal

A string operation inside a loop runs as many times as the loop does, using
the same one or two temporary buffers on every iteration. The container
header's `num_temp_bufs` becomes a true bound: the deepest nesting of live
string values along any call chain, not a count of source-level call sites.

## Why the current design fails

Three parts of the string design each assumed a different buffer lifetime,
and nothing tied them together:

| Part | Lifetime it assumed |
|---|---|
| ADR-0017 (the design intent) | "transient within an expression" |
| VM (`TempBufAllocator`, `handle_frame_return`) | released only when the call frame returns |
| Codegen (`CompileContext::num_temp_bufs`) | one buffer per *static* string-operation site |

A bump allocator that is rewound only on frame return is an arena. An arena
sized by a static count cannot survive a loop: a loop body allocates per
iteration but was counted once. Every `CONCAT`/`MID`/`LEFT`/literal in a
loop body exhausts the pool, regardless of how the pool is sized, because
no static count bounds a dynamic iteration count.

The scratch-pad pool itself is not the mistake — it is the string analogue
of the operand stack, and it is what lets a string value flow through the
operand stack as a small `buf_idx` exactly as a scalar does. The mistake is
that the pool is *sized* like a stack (a static bound on depth) but
*released* like an arena. Making it release like a stack makes the static
bound correct.

## Architecture

**A temp buffer is owned by the operand-stack slot that holds its
`buf_idx`, and is released when that slot is consumed.** The consumers are
`STR_STORE_VAR` and `STR_STORE_ARRAY_ELEM`; every other string opcode takes
its inputs as data-region offsets (a nested expression or literal is first
spilled to a hidden data-region slot by `STR_STORE_VAR`). Because the
operand stack is LIFO, so is consumption: when slot `k` is consumed, every
slot above `k` is already dead, so the allocator rewinds `next` to `k`.

A `buf_idx` at or above `next` is left alone: it is the value a callee
returned, whose frame return already rewound past it. That value is
"valid until the next allocation" exactly as today.

The frame-return rewind stays as a safety net for the operand-stack
imbalance the verifier already forbids.

**Codegen tracks live depth, not site count.** The `Emitter` gains a
temp-depth counter beside `current_stack_depth`/`max_stack_depth`: every
emit function for an allocating opcode pushes one, every consuming emit
function pops one. The per-function maximum flows out through
`FinalizedFunction`. The program-wide `num_temp_bufs` is the longest
weighted path through the static call graph from `SCAN` (and the init
function on its own), where each function's weight is its own maximum depth
— the same walk that already produces `max_call_depth`, with a weight other
than 1.

Consequences worth stating: a loop over strings needs one buffer; the pool
for a typical program shrinks from "number of string statements" to one or
two buffers, which matters for the no-std targets the pool exists for; and
the bundled `LREAL_TO_FMTSTR` no longer has to be unrolled (left as-is here,
follow-up issue).

## Prefactoring

Both prefactors land as their own commits, behaviour-preserving, before the
fix.

1. **Emitter-owned temp accounting.** Fifteen `ctx.num_temp_bufs += 1`
   sites across six modules each remember to count next to an emit call.
   Move the count into the emit functions themselves (the emitter is the
   one place that knows which opcodes touch the pool) and return the
   per-function total through `FinalizedFunction`, summed in `compile.rs`.
   Every allocating emit call today has a matching increment, so the header
   value is identical for every program. The signal from the development
   standards: "a similar bug could occur rather than being prevented at
   compile time" — a new string opcode can no longer be emitted without
   being accounted for.

2. **Weighted longest path over the call graph.** `compute_max_call_depth`
   is a longest-path walk with every node weighted 1. Generalize it to
   `longest_path(graph, entry, weight)` and express the call-depth
   computation through it. The fix then adds a second call with the
   per-function temp depth as the weight, rather than a second DFS.

## Design doc references

- `specs/design/bytecode-instruction-set.md` — String Operations (buffer
  lifecycle, pool sizing)
- `specs/design/runtime-execution-model.md` — String Buffer Management
- `specs/adrs/0017-unified-data-region.md` — the original lifetime intent
- New: `specs/adrs/0052-temp-string-buffers-released-on-consume.md`

## File map

| File | Change |
|---|---|
| `compiler/codegen/src/emit.rs` | Temp-depth tracking beside stack-depth tracking; `emit_call` learns whether the callee returns a string |
| `compiler/codegen/src/compile.rs` | Drop `CompileContext::num_temp_bufs`; `FinalizedFunction`/`CompiledFunction` carry `max_temp_depth`; size the pool from the call graph |
| `compiler/codegen/src/compile_{stmt,expr,call,string,fn}.rs` | Delete the scattered `+= 1` sites |
| `compiler/codegen/src/call_graph.rs` | Weighted longest path |
| `compiler/vm/src/string_ops.rs` | `TempBufAllocator::release` |
| `compiler/vm/src/vm.rs` | `STR_STORE_VAR` / `STR_STORE_ARRAY_ELEM` release the consumed buffer |
| `compiler/vm/tests/it/execute_string_ops.rs` | Pool-of-one reuse tests (`REQ-RT-vm-*`) |
| `compiler/codegen/tests/it/end_to_end_string_loop.rs` | The issue's program and variants, run end to end |
| `compiler/codegen/tests/it/compile_temp_bufs.rs` | Pins the header sizing model (`REQ-RT-codegen-*`) |
| `compiler/codegen/build.rs` | Register `runtime-execution-model.md` for the codegen-slugged requirement |
| `specs/design/bytecode-instruction-set.md`, `specs/design/runtime-execution-model.md` | Reconcile the lifecycle sections with the new discipline |
| `specs/adrs/0052-temp-string-buffers-released-on-consume.md` | The decision and the alternatives |
| `docs/reference/runtime/problems/V9009.rst` | Describe the sizing model the trap guards |

## Tasks

- [x] Plan
- [ ] Prefactor 1: emitter-owned temp accounting (count-neutral)
- [ ] Prefactor 2: weighted longest path in `call_graph.rs`
- [ ] VM: `release` on consume, unit and integration tests
- [ ] Codegen: depth tracking, call-graph sizing, tests
- [ ] Design docs, ADR-0052, V9009 page
- [ ] `cd compiler && just`, `cd specs && just`
- [ ] Delete this plan
