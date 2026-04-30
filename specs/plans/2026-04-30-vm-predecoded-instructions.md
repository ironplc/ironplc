# Plan: Pre-Decoded Instruction Stream for the VM Dispatch Loop

## Context

FOR loops are roughly 200x slower than native code. The roadmap's biggest
near-term levers fall outside our current constraint set:

- **Verifier-gated unchecked execution** (10-30%) requires `unsafe` paths
  in the VM and is being deferred until after the verifier lands.
- **Superinstructions / register translation / fused INC_VAR** all
  require new opcodes; the team is mid-flight on the opcode set and
  doesn't want to disturb that work with parallel opcode additions.
- **Tail-call threaded dispatch** needs nightly Rust or unsafe trampoline
  code on stable.

Two codegen-only wins have already shipped on top of those constraints:

- TRUNC elision for in-range constant bounds
  (`specs/plans/2026-04-30-elide-for-loop-trunc.md`).
- Per-iteration exit-`JMP` elision via inverted predicate
  (`specs/plans/2026-04-30-elide-for-loop-exit-jmp.md`).

Both saved one dispatch per iteration. The remaining FOR-loop cost is
dominated by **per-instruction overhead inside the dispatch loop
itself** — see `compiler/vm/src/vm.rs:682`:

```rust
while pc < bytecode.len() {
    let op = bytecode[pc];          // bounds check on pc
    hook.before_instruction(pc, op);
    pc += 1;
    match op {
        opcode::LOAD_VAR_I32 => {
            let index = VarIndex::new(read_u16_le(bytecode, &mut pc)?);
            // ... 2 more bounds checks inside read_u16_le
        }
        // ~96 arms ...
    }
}
```

For a typical FOR-loop iteration (~13 dispatches after the recent wins),
each iteration pays:

- 1 bounds check on `bytecode[pc]` per dispatch.
- 2 bounds checks per `read_u16_le` (one per byte) for opcodes with
  operands — most FOR-loop opcodes have a u16 operand.
- Branch on a 96-arm `match` whose generated jump table is ~18 KB
  (`vm-performance.md` §4b notes this exceeds typical L1i capacity).

**Pre-decoding the bytecode once at VM load** removes the byte-level
bounds checks and the per-instruction operand decode work, leaving the
dispatch loop as a tight `for ins in &instructions { match ins.op }`
walk over a pre-extracted instruction array. Jump targets become
absolute instruction indices, eliminating the `i16`→byte-offset→`pc`
arithmetic.

The decoded form lives in a **caller-provided buffer** (option 3 from
the design discussion) so the VM stays compatible with the project's
no_std direction. Container size on flash/disk doesn't grow; only RAM
does, and only at runtime.

## Approach

### 1. `Instruction` value type

New module `compiler/vm/src/instruction.rs`. Plain-old-data, `Copy`,
fixed size to keep dispatch trivial:

```rust
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Instruction {
    pub op: u8,
    _pad: [u8; 3],
    pub operand_a: u32,   // var index, const-pool index, abs jump target,
                          //   decoded function id, byte offset into data
                          //   region — narrow operand for the common case
    pub operand_b: u32,   // second operand for the few opcodes that need
                          //   one (CALL var_offset, string ops second u32);
                          //   zero for everyone else
}
```

Size: 12 bytes; aligned to 16 in practice. For typical FOR-loop bytecode
(~2 bytes/opcode average) this is a **6-8x RAM expansion of the program
text at runtime**, with no change to container/flash size. Concrete
example: a 10 KB bytecode container decodes to ~60-80 KB of
`Instruction`s in RAM. This is the price of the optimisation — call out
explicitly during review.

Open question worth resolving early: drop `operand_b` and emit an
extension cell (sentinel `op`) for the four 9-byte string opcodes
(`FIND_STR`, `REPLACE_STR`, `INSERT_STR`, `CONCAT_STR`)? That would cut
the common case to 8 bytes/instruction and only the four string opcodes
pay double. Recommendation: **start with the 12-byte fixed layout**;
revisit if benchmarks show the size matters more than dispatch
simplicity.

### 2. Decoder

New `pub fn decode_program(container: &Container, buf: &mut [Instruction])
-> Result<DecodedProgram, DecodeError>`. Walks every function's
bytecode (one call per `FunctionId` via `container.code.get_function_bytecode`,
mirroring the existing `CALL` handler at `vm.rs:1013`) and emits one
`Instruction` per source opcode into `buf`. Returns:

```rust
pub struct DecodedProgram {
    /// Per-FunctionId range into the buffer.
    pub functions: Vec<core::ops::Range<u32>>,
}
```

Sizing: ship a companion `pub fn decoded_size_for(container: &Container)
-> usize` that returns the exact instruction count. Embedders allocate
`[Instruction; N]` of that size up-front. For std users, expose a
convenience `decode_program_owned(container) -> (Vec<Instruction>,
DecodedProgram)`.

Decoder responsibilities:

- **Operand extraction.** Pre-pull the u16/u32 operand into
  `operand_a` (and `operand_b` where applicable). The dispatch loop no
  longer does any byte slicing.
- **Jump-target resolution.** `JMP`/`JMP_IF_NOT` source operand is an
  `i16` byte offset; resolve to an **absolute instruction index** (u32)
  in the same function's range and store in `operand_a`. The dispatch
  loop just does `pc = ins.operand_a as usize`.
- **CALL resolution.** `CALL`'s `func_id` (u16) becomes the function
  table index; resolution at decode time means the call handler doesn't
  re-do `container.code.get_function`/`get_function_bytecode`.
  `var_offset` goes in `operand_b`.
- **Validation.** Reject malformed bytecode (truncated operand, jump
  target outside function range, unknown opcode width, bad function id).
  This is not the verifier — it's just the decode-time sanity that any
  pre-decode pass needs anyway. Keep the error surface narrow:
  `DecodeError::TruncatedOperand`, `BadJumpTarget`, `UnknownOpcode`,
  `BadFunctionId`.

### 3. Dispatch loop

Replace the byte-driven loop in `compiler/vm/src/vm.rs:682` with:

```rust
let instructions: &[Instruction] = decoded.slice_for(current_function);
let mut ip: usize = 0;
while ip < instructions.len() {
    let ins = instructions[ip];
    hook.before_instruction(ip, ins.op);
    ip += 1;
    #[cfg(feature = "profiling")]
    profile.record(ins.op);
    match ins.op {
        opcode::LOAD_VAR_I32 => {
            let index = VarIndex::new(ins.operand_a as u16);
            let value = scope.check_access(index)?
                .and_then(|()| variables.load(index))?;
            stack.push(value)?;
        }
        opcode::JMP => {
            ip = ins.operand_a as usize;
        }
        opcode::JMP_IF_NOT => {
            let cond = stack.pop()?.as_i32();
            if cond == 0 { ip = ins.operand_a as usize; }
        }
        // ... etc
    }
}
```

What disappears compared to today:

- The `bytecode[pc]` bounds check (subsumed by `ip < instructions.len()`,
  but the slice indexer is one bounds check vs. several today).
- Every `read_u16_le` / `read_u32_le` call in the match arms — operands
  are already in the `Instruction`.
- The `(pc as isize + offset as isize) as usize` jump arithmetic.
- The `container.code.get_function*` lookups inside `CALL`.

### 4. Function calls

The existing `CALL` arm (`vm.rs:1013`) recurses into
`execute_with_hook(func_bytecode, ...)`. Equivalent under the new
scheme: take the callee's instruction slice from `DecodedProgram` and
recurse with that. No structural change to the call mechanism — just
the input it consumes.

### 5. `VmBuffers` extension

`compiler/vm/src/buffers.rs` already follows a "caller pre-allocates,
VM borrows" pattern (`Vm::load(self, container, bufs: &'a mut VmBuffers)`).
Add a sibling type `DecodedBuffer<'a>(&'a mut [Instruction])` and a
companion `VmBuffers::from_container_with_decoded(container,
decoded_buffer)`. Two construction styles:

- **Strict no_std / static.** Embedder reserves
  `static mut DECODED: [Instruction; N]`, hands a `&mut` slice in.
- **std convenience.** `VmBuffers::from_container(container)` (existing)
  internally calls `decode_program_owned` and stores the resulting
  `Vec<Instruction>` alongside today's `Vec`s. Hot-path behaviour is
  identical; only construction differs.

This keeps the no_std path open without breaking today's `Vec`-based
construction.

### 6. Hook + profiling semantics

- **`DebugHook::before_instruction(pc, op)`** at
  `compiler/vm/src/debug_hook.rs:28`: change the first parameter's
  meaning from "byte offset of opcode" to "instruction index". This is
  a **deliberate breaking change** to the hook trait. Document it in
  the doc comment and update the test hook in the same file.
  - For tools that need byte offsets (e.g. mapping back to a disassembly
    view), expose a helper on `DecodedProgram` that reverse-maps
    `(FunctionId, ip) -> byte_offset`.
- **Profiling** (`compiler/vm/src/profile.rs`): unchanged. Still records
  one count per opcode per execution.

## Files to change

- `compiler/vm/src/instruction.rs` — **new**. `Instruction` POD type,
  helpers.
- `compiler/vm/src/decoder.rs` — **new**. `decode_program`,
  `decoded_size_for`, `decode_program_owned`, `DecodeError`.
- `compiler/vm/src/buffers.rs` — extend `VmBuffers` with the decoded
  buffer; update `from_container` to call the decoder.
- `compiler/vm/src/vm.rs` — rewrite the dispatch loop body around
  `&[Instruction]`; replace every `read_u16_le`/`read_i16_le`/manual
  `bytecode[pc]` use; collapse `CALL` to use the decoded function table;
  update `JMP`/`JMP_IF_NOT` to use absolute `ip`. This is the largest
  diff.
- `compiler/vm/src/debug_hook.rs` — doc-comment change for hook
  semantics; update test hook.
- `compiler/vm/src/lib.rs` — register the new modules.
- `compiler/vm/Cargo.toml` — no dep changes expected.

No changes to: container format, codegen, opcode set, verifier (still
absent), CLI, playground.

## Risks and open questions

1. **Memory cost.** 6-8x runtime growth of the program text. For a
   ~10 KB container, that's ~60-80 KB of decoded instructions in RAM.
   Acceptable for desktop and most embedded PLC targets; document the
   number for users with very tight RAM. Quantify exactly during
   implementation by decoding the existing benchmark containers and
   reporting actual sizes.
2. **Decode startup cost.** Linear in bytecode size; one-shot at
   `Vm::load`. Should be sub-millisecond for any realistic program.
   Measure and report.
3. **Hook breaking change.** The `pc` parameter semantics change from
   byte offset to instruction index. This is a small public-API break;
   call out in the changelog and provide the reverse-mapping helper.
4. **Worst-case opcode width.** The 9-byte string ops need both
   `operand_a` and `operand_b`. Confirmed they fit in the proposed
   12-byte `Instruction`. Revisit only if measurement shows the
   instruction size matters more than dispatch simplicity (see §1).
5. **Composes with the future verifier.** When the verifier lands, it
   should validate the **decoded** form (jump targets are valid `ip`s,
   operand_a fits its expected width, etc.). This makes verifier rules
   simpler than validating raw bytecode and lets the verified-unchecked
   execution path skip even the slice index check on `instructions[ip]`.
   This plan does not require the verifier; it just doesn't get in
   verifier's way.
6. **Big PR.** Suggest gating behind a `predecoded-dispatch` Cargo
   feature for the first PR, with the legacy byte-driven loop staying
   in place under the off setting. Flip the default after one or two
   release cycles of soak time. (If the team prefers atomic switchover,
   skip the feature gate — but the diff will be large and harder to
   review.)

## Verification

1. **Unit tests for the decoder.** Cover every opcode class: 1-byte,
   3-byte, 5-byte, 7-byte, 9-byte. Round-trip property: for any valid
   container, `decode → re-encode by walking instructions` produces the
   same byte stream (or a documented canonical form).
2. **Behavioural equivalence.** All existing
   `compiler/codegen/tests/end_to_end_*` and `compiler/vm/` integration
   tests must pass unchanged. This is the strongest signal — if any
   end-to-end test diverges, semantics drifted.
3. **FOR-loop microbenchmarks.** Re-run
   `compiler/benchmarks/benches/st_benchmark.rs::st_for_loop` and
   `::st_nested_loops`; expect a measurable improvement (target: ≥15%
   on `st_for_loop` 10 000-iter, ≥10% on
   `st_nested_loops` 100×100). Lock in the numbers before/after in the
   PR description.
4. **Profiler invariant.** Re-run
   `compiler/benchmarks/tests/profile_for_loop.rs`; per-opcode counts
   must match the byte-driven baseline exactly (we are not changing
   what executes, only how it dispatches).
5. **Decode size + startup cost.** Add a benchmark or test that decodes
   each benchmark container and reports `decoded_size_for(container)`
   and the decode wall time. Commit the numbers in the plan/PR for
   future regression tracking.
6. **Full CI.** `cd compiler && just` — required before any PR.

## Out of scope

- String `copy_from_slice` (separate, smaller, independent win;
  scheduled separately).
- The bytecode verifier (separate plan,
  `specs/plans/2026-04-06-bytecode-verifier.md`).
- Verifier-gated unchecked stack/var access (blocked on verifier and
  on adopting `unsafe`).
- Superinstructions / fused INC_VAR / register translation (folded into
  the in-flight opcode redesign, not addressed here).
- New opcodes of any kind.

## Rollout

1. Land this plan on `main`.
2. Implement behind a `predecoded-dispatch` Cargo feature, default off.
3. Run the verification suite under both feature settings; commit the
   benchmark deltas to the PR.
4. After ≥1 release cycle with the feature exercised in CI, flip the
   default to on; remove the legacy byte-driven dispatch in a follow-up
   PR.
