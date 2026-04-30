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
dispatch loop as a tight walk over a pre-extracted cell array. Jump
targets become absolute cell indices, eliminating the
`i16`→byte-offset→`pc` arithmetic. CALL function-id lookups happen at
decode time, not on every call.

The decoded form lives in a **caller-provided buffer** (option 3 from
the design discussion) so the VM stays compatible with the project's
no_std direction. Container size on flash/disk doesn't grow; only RAM
does, and only at runtime.

**Cache-friendliness is a first-class goal.** The dispatch loop reads
one cell per iteration; if the cell stream fits in L1d (32 KB on
typical cores) the inner loop sees cache-hit latency on every fetch.
For nested or long-running FOR loops the cell-stream RAM footprint
directly translates to memory-traffic and L1 evictions, so the encoding
is sized to keep the common case ~1.6× the raw bytecode rather than
the 4-6× a naïve fixed-width design would cost. Combined with the
API-level decision to drop the original bytecode after decode (see §7),
total program-text RAM ends up at ~1.6× today's footprint, not the
~5-6× a "two copies" design would impose.

## Approach

### 1. Cell encoding

New module `compiler/vm/src/cell.rs`. Each pre-decoded instruction is a
**4-byte `u32` "cell"** with the opcode in the low byte and a 24-bit
inline operand in the high bits:

```rust
/// Pre-decoded instruction. The low 8 bits are the opcode; the upper
/// 24 bits are an inline operand (var index, const-pool index, abs
/// cell-index jump target, decoded function-table index, etc.).
#[derive(Clone, Copy)]
pub struct Cell(pub u32);

impl Cell {
    #[inline] pub fn op(self) -> u8 { self.0 as u8 }
    #[inline] pub fn operand(self) -> u32 { self.0 >> 8 }

    #[inline] pub fn new(op: u8, operand: u32) -> Self {
        debug_assert!(operand <= 0x00FF_FFFF, "operand exceeds 24 bits");
        Cell((operand << 8) | (op as u32))
    }
}
```

24 bits fit every operand that appears in a FOR-loop body:

- variable index (u16) ✓
- constant-pool index (u16) ✓
- absolute cell-index jump target — 16 M cells of headroom ✓
- function-table index (u16) ✓

**Continuation cells for wide opcodes.** The few opcodes whose source
encoding carries more than 24 bits of operand (`CALL`'s `func_id +
var_offset`, and the four 9-byte string ops `FIND_STR`, `REPLACE_STR`,
`INSERT_STR`, `CONCAT_STR` with two u32s) consume **two consecutive
cells**: the first carries the opcode + first operand fragment, the
second carries `op = OPCODE_CONTINUATION` (a sentinel) + the rest of
the operand. Handlers that need the second operand simply read
`cells[ip + 1]` and advance `ip` by 2.

The continuation sentinel is a reserved opcode value never produced by
codegen and never executed as a real instruction; the decoder asserts
that no real opcode collides.

### Size math (the headline number)

Per-iteration FOR-loop body (~13 dispatches, all 1- or 3-byte today):

| Form | Decoded size | Total RAM (keep bytecode) | Total RAM (drop bytecode, see §7) |
|---|---|---|---|
| Raw bytecode (today) | — | 1.0× | 1.0× |
| **4-byte cells** | **1.6×** | 2.6× | **1.6×** |
| 12-byte fixed (rejected) | 4.7× | 5.7× | 4.7× |

For a 10 KB container: ~16 KB decoded, replacing the original at
runtime. The RAM cost of the optimisation is therefore **+0.6× of
bytecode size** when paired with the bytecode-drop API in §7.

Where the expansion comes from, in decreasing order of impact:

1. **Padding small opcodes up to a uniform 4-byte cell** — ~80% of
   FOR-loop opcodes are 1- or 3-byte today; cell padding is the only
   inherent cost of fixed-width dispatch.
2. **Continuation cells for the 5/9-byte opcodes** — rare in FOR loops,
   negligible overall.

Operand widening is **not** a contributor: u16 indices stay u16, and
24-bit absolute jump targets fit in the same cell slot as today's i16
byte offset.

### 2. Decoder

New `pub fn decode_program(container: &Container, buf: &mut [Cell])
-> Result<DecodedProgram, DecodeError>`. Walks every function's
bytecode (one call per `FunctionId` via
`container.code.get_function_bytecode`, mirroring the existing `CALL`
handler at `vm.rs:1013`) and emits one or two `Cell`s per source
opcode into `buf`. Returns:

```rust
pub struct DecodedProgram {
    /// Per-FunctionId range into the cell buffer.
    pub functions: Vec<core::ops::Range<u32>>,
}
```

Sizing: ship a companion `pub fn decoded_cell_count(container: &Container)
-> usize` that returns the exact cell count (counting continuation
cells). Embedders allocate `[Cell; N]` of that size up-front. For std
users, expose a convenience `decode_program_owned(container) ->
(Vec<Cell>, DecodedProgram)`.

Decoder responsibilities:

- **Operand extraction.** Pre-pull the u16/u32 operand into the cell's
  24-bit inline slot (or split across a continuation cell). The
  dispatch loop no longer does any byte slicing.
- **Jump-target resolution.** `JMP`/`JMP_IF_NOT` source operand is an
  `i16` byte offset; resolve to an **absolute cell index** (u32) within
  the same function's range and store in the cell's operand slot. The
  dispatch loop just does `ip = cell.operand() as usize`. The decoder
  asserts targets land on a real cell start, never on a continuation
  cell.
- **CALL resolution.** `CALL`'s `func_id` (u16) becomes the
  function-table index; resolution at decode time means the call
  handler doesn't re-do `container.code.get_function` /
  `get_function_bytecode`. `var_offset` goes in the continuation cell.
- **Validation.** Reject malformed bytecode (truncated operand, jump
  target outside function range or onto a continuation cell, unknown
  opcode width, bad function id, operand overflows 24 bits when only
  one cell was emitted). This is not the verifier — it's the
  decode-time sanity any pre-decode pass needs anyway. Error surface:
  `DecodeError::TruncatedOperand`, `BadJumpTarget`, `UnknownOpcode`,
  `BadFunctionId`, `OperandOverflow`.

### 3. Dispatch loop

Replace the byte-driven loop in `compiler/vm/src/vm.rs:682` with:

```rust
let cells: &[Cell] = decoded.slice_for(current_function);
let mut ip: usize = 0;
while ip < cells.len() {
    let cell = cells[ip];
    let op = cell.op();
    let operand = cell.operand();
    hook.before_instruction(ip, op);
    ip += 1;
    #[cfg(feature = "profiling")]
    profile.record(op);
    match op {
        opcode::LOAD_VAR_I32 => {
            let index = VarIndex::new(operand as u16);
            let value = scope.check_access(index)?
                .and_then(|()| variables.load(index))?;
            stack.push(value)?;
        }
        opcode::JMP => {
            ip = operand as usize;
        }
        opcode::JMP_IF_NOT => {
            let cond = stack.pop()?.as_i32();
            if cond == 0 { ip = operand as usize; }
        }
        opcode::CALL => {
            let func_id = operand as u16;          // 1st cell
            let var_offset = cells[ip].operand() as u16; // 2nd cell
            ip += 1;                                // skip continuation
            // ...
        }
        // ... etc
    }
}
```

What disappears compared to today:

- The `bytecode[pc]` bounds check (subsumed by `ip < cells.len()`, but
  one bounds check per **instruction** rather than 1-3 per instruction
  today).
- Every `read_u16_le` / `read_u32_le` call in the match arms — operands
  are already in the cell.
- The `(pc as isize + offset as isize) as usize` jump arithmetic — `ip`
  is set directly from the pre-resolved absolute cell index.
- The `container.code.get_function*` lookups inside `CALL` — replaced
  by a direct function-table index.

What's added per dispatch: one `cell.0 as u8` mask + one `cell.0 >> 8`
shift. Both compile to single instructions on every target.

### 4. Function calls

The existing `CALL` arm (`vm.rs:1013`) recurses into
`execute_with_hook(func_bytecode, ...)`. Equivalent under the new
scheme: take the callee's cell slice from `DecodedProgram` and recurse
with that. No structural change to the call mechanism — just the input
it consumes.

### 5. `VmBuffers` extension

`compiler/vm/src/buffers.rs` already follows a "caller pre-allocates,
VM borrows" pattern (`Vm::load(self, container, bufs: &'a mut VmBuffers)`).
Add a sibling type `DecodedBuffer<'a>(&'a mut [Cell])` and a companion
`VmBuffers::from_container_with_decoded(container, decoded_buffer)`.
Two construction styles:

- **Strict no_std / static.** Embedder reserves
  `static mut DECODED: [Cell; N]`, hands a `&mut` slice in.
- **std convenience.** `VmBuffers::from_container(container)` (existing)
  internally calls `decode_program_owned` and stores the resulting
  `Vec<Cell>` alongside today's `Vec`s. Hot-path behaviour is identical;
  only construction differs.

This keeps the no_std path open without breaking today's `Vec`-based
construction.

### 6. Hook + profiling semantics

- **`DebugHook::before_instruction(pc, op)`** at
  `compiler/vm/src/debug_hook.rs:28`: change the first parameter's
  meaning from "byte offset of opcode" to "cell index". This is a
  **deliberate breaking change** to the hook trait. Document it in the
  doc comment and update the test hook in the same file.
  - For tools that need byte offsets (e.g. mapping back to a
    disassembly view), expose a helper on `DecodedProgram` that
    reverse-maps `(FunctionId, ip) -> byte_offset`.
- **Profiling** (`compiler/vm/src/profile.rs`): unchanged. Still records
  one count per opcode per execution.

### 7. Bytecode-release API (the second half of the RAM win)

Today `Vm::load(self, container: &'a Container, bufs)` borrows the
container, so the original bytecode stays resident in RAM for the
lifetime of the VM. After this change the decoded cell stream is the
only form the dispatch loop touches; keeping the original bytecode
around alongside it doubles the program-text footprint for no
runtime benefit.

Introduce `Container::take_bytecode() -> ContainerBytecode` (or
equivalent) that detaches the per-function bytecode slices from the
container and hands them to the decoder, after which the bytecode is
dropped. The container retains type info, function metadata, debug
maps, and constant pool — only the raw bytecode bytes go away.

Two construction paths, embedder-selectable:

- **`Vm::load_releasing(container, bufs)`** — moves the bytecode into
  the decoder, drops it once decode succeeds. Net program-text RAM:
  ~1.6× of original bytecode size. **Recommended default for
  production / embedded targets.**
- **`Vm::load(container, bufs)`** — current borrowing form. Keeps
  bytecode resident alongside decoded cells (~2.6× total). Useful for
  development, disassembler tooling
  (`compiler/project/src/disassemble.rs`), and any caller that
  inspects raw bytecode after VM construction.

Tools that today walk `container.code.get_function_bytecode(...)` keep
working under `Vm::load`; under `Vm::load_releasing` they need to
either run before VM construction or be ported to walk the decoded
cell stream via the `DecodedProgram` reverse-mapping helper from §6.

This API split gives the project two clean choices per call site
rather than forcing every embedder onto one trade-off.

## Files to change

- `compiler/vm/src/cell.rs` — **new**. `Cell` POD type with
  `op()`/`operand()`/`new()`; the continuation-cell sentinel constant.
- `compiler/vm/src/decoder.rs` — **new**. `decode_program`,
  `decoded_cell_count`, `decode_program_owned`, `DecodeError`.
- `compiler/vm/src/buffers.rs` — extend `VmBuffers` with the decoded
  cell buffer; update `from_container` to call the decoder.
- `compiler/vm/src/vm.rs` — rewrite the dispatch loop body around
  `&[Cell]`; replace every `read_u16_le`/`read_i16_le`/manual
  `bytecode[pc]` use; collapse `CALL` to use the decoded function table;
  update `JMP`/`JMP_IF_NOT` to use absolute `ip`. Add
  `Vm::load_releasing` alongside `Vm::load`. This is the largest diff.
- `compiler/vm/src/debug_hook.rs` — doc-comment change for hook
  semantics; update test hook.
- `compiler/vm/src/lib.rs` — register the new modules; export `Cell`,
  `DecodedProgram`, `DecodeError`.
- `compiler/container/src/code.rs` (or wherever the bytecode store
  lives) — add `take_bytecode` / equivalent ownership-transfer API
  used by `Vm::load_releasing`.
- `compiler/vm/Cargo.toml` — no dep changes expected.

No changes to: container *format* (only the in-memory ownership API),
codegen, opcode set, verifier (still absent), CLI, playground.

## Risks and open questions

1. **Memory cost.** ~1.6× of bytecode size for the decoded cell stream
   (when paired with `Vm::load_releasing` from §7). For a ~10 KB
   container that's ~16 KB of cells replacing the original bytecode in
   RAM. Under the borrowing `Vm::load` form, total program-text RAM is
   ~2.6× because the original bytecode stays resident. Quantify exactly
   during implementation by decoding the existing benchmark containers
   and committing actual numbers to the PR.
2. **Cache pressure.** A direct goal, not just a side-effect. Cell
   stream + dispatch table together should fit comfortably in L1d/L1i
   on common cores: 4-byte cells × ~1300 cells (a 100-iter FOR-loop
   body) = ~5 KB, well under typical 32 KB L1d. Validate by sampling
   `perf stat -e L1-dcache-load-misses` (or platform equivalent) on
   `st_for_loop` and `st_nested_loops` before/after.
3. **Decode startup cost.** Linear in bytecode size; one-shot at
   `Vm::load` / `Vm::load_releasing`. Should be sub-millisecond for any
   realistic program. Measure and report.
4. **Hook breaking change.** The `pc` parameter semantics change from
   byte offset to cell index. This is a small public-API break; call
   out in the changelog and provide the reverse-mapping helper.
5. **`Vm::load_releasing` and disassembler tooling.** Disassembler /
   debug tooling that today reads
   `container.code.get_function_bytecode(...)` needs to either run
   before VM construction or be ported to the decoded form. Inventory
   in-tree call sites
   (`compiler/project/src/disassemble.rs`, MCP tools) before flipping
   any default; the `Vm::load` (borrowing) variant continues to work
   for them in the meantime.
6. **24-bit operand cap.** The decoder asserts every single-cell
   operand fits in 24 bits. Variable indices and constant-pool indices
   are u16, well within. Cell-index jump targets cap at 16 M cells per
   function, which translates to ~64 MB of decoded text per function —
   far beyond any realistic PLC. Surface as `DecodeError::OperandOverflow`
   on the off chance some pathological input triggers it.
7. **Composes with the future verifier.** When the verifier lands, it
   validates the **decoded** form: jump targets land on real cell
   starts (not continuation cells), operands fit their expected widths,
   continuation cells follow only the wide opcodes that produce them.
   Verifier rules end up simpler than validating raw bytecode, and the
   verified-unchecked execution path can later skip even the slice
   index check on `cells[ip]`. This plan does not require the verifier;
   it just doesn't get in the verifier's way.
8. **Big PR.** Gate behind a `predecoded-dispatch` Cargo feature for
   the first PR, with the legacy byte-driven loop staying in place
   under the off setting. Flip the default after one or two release
   cycles of soak time.

## Verification

1. **Unit tests for the decoder.** Cover every opcode class: 1-byte,
   3-byte, 5-byte, 7-byte, 9-byte (the last two emit continuation
   cells). Include negative tests for `BadJumpTarget` (target onto a
   continuation cell), `OperandOverflow`, `UnknownOpcode`, and
   `BadFunctionId`. Round-trip property: for any valid container,
   `decode → re-encode by walking cells` produces the same source byte
   stream (or a documented canonical form).
2. **Behavioural equivalence.** All existing
   `compiler/codegen/tests/end_to_end_*` and `compiler/vm/` integration
   tests must pass unchanged under both `Vm::load` and
   `Vm::load_releasing`. This is the strongest signal — if any
   end-to-end test diverges, semantics drifted.
3. **FOR-loop microbenchmarks.** Re-run
   `compiler/benchmarks/benches/st_benchmark.rs::st_for_loop` and
   `::st_nested_loops`; target ≥15% on `st_for_loop` 10 000-iter and
   ≥10% on `st_nested_loops` 100×100. Lock in numbers before/after in
   the PR description.
4. **Cache-locality measurement.** On Linux x86_64, `perf stat -e
   L1-dcache-loads,L1-dcache-load-misses,instructions,cycles` over
   `st_for_loop` and `st_nested_loops`. Expect the L1d miss rate to
   stay flat or drop (cell stream is smaller and stride-1) and IPC to
   improve. Commit the raw counters to the PR.
5. **Profiler invariant.** Re-run
   `compiler/benchmarks/tests/profile_for_loop.rs`; per-opcode counts
   must match the byte-driven baseline exactly (we are not changing
   what executes, only how it dispatches).
6. **Decode size + startup cost.** Add a benchmark or test that decodes
   each benchmark container and reports `decoded_cell_count(container)`,
   the resulting RAM footprint vs. raw bytecode, and the decode wall
   time. Commit the numbers in the plan/PR for future regression
   tracking.
7. **Full CI.** `cd compiler && just` — required before any PR.

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
   In this first PR, expose **only** `Vm::load` (borrowing form) — the
   bytecode stays resident, so disassembler / debug tooling keeps
   working unchanged. Program-text RAM is ~2.6× today's (1.0× original
   bytecode + 1.6× decoded cells); full dispatch speedup is realised.
3. Run the verification suite under both feature settings; commit the
   benchmark deltas to the PR.
4. After ≥1 release cycle of soak, ship `Vm::load_releasing` as a
   follow-up. Inventory in-tree disassembler / MCP-tools call sites
   beforehand and either move them ahead of VM construction or port
   them to the decoded form via the `(FunctionId, ip) → byte_offset`
   reverse mapping.
5. After another release cycle, flip the `predecoded-dispatch` default
   to on; remove the legacy byte-driven dispatch in a final cleanup PR.
