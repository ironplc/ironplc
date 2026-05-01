# Plan: Cell-Form Bytecode in Codegen and Container

## Context

FOR loops are roughly 200x slower than native code. The roadmap's biggest
near-term levers fall outside our current constraint set:

- **Verifier-gated unchecked execution** (10-30%) requires `unsafe` paths
  in the VM and is being deferred until after the verifier lands.
- **Tail-call threaded dispatch** needs nightly Rust or unsafe trampoline
  code on stable.

Two codegen-only wins have already shipped:

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
  operands — most FOR-loop opcodes have a u16 operand. There are 30+
  `read_u16_le` / `read_i16_le` / `read_u32_le` callsites in `vm.rs`
  (`compiler/vm/src/vm.rs:2084-2119`).
- Branch on a 96-arm `match` whose generated jump table is ~18 KB
  (`vm-performance.md` §4b notes this exceeds typical L1i capacity).
- For `JMP` / `JMP_IF_NOT`: arithmetic to convert the i16 byte offset
  back into an absolute `pc`.
- For `CALL`: a `container.code.get_function*` lookup on every call
  (`compiler/vm/src/vm.rs:1013`).

### Why this plan supersedes the predecode design

A previous draft of this plan — "Pre-Decoded Instruction Stream for the
VM Dispatch Loop" — proposed a one-shot decode pass at `Vm::load` that
expanded the byte stream into a `[Cell]` buffer in RAM, then dispatched
on cells. That design was shaped by a constraint that no longer holds:
**we couldn't change codegen or the on-disk container format**, so the
only way to get cell-form dispatch was to translate at load time.

That constraint cost us:

- A second program-text representation in the VM (bytes + cells).
- A whole-section `Vm::load_releasing` API to drop the original
  bytecode after decode, plus a "borrowing" variant for tooling that
  still needed the bytes — the design's entire §7.
- A reverse-mapping helper (`(FunctionId, ip) → byte_offset`) so
  hooks/disassembler tooling could keep speaking byte offsets.
- Two-form invariants for the future verifier ("verify the bytes" vs.
  "verify the decoded cells").
- Subtle edge cases around continuation cells (jump-target-onto-
  continuation, operand-overflow on a single-cell op, etc.) that exist
  only because the byte form was the source of truth.

With the constraint relaxed, **codegen emits cells directly and the
container stores cells**. There is exactly one form. The decoder
disappears, the bytecode-release API disappears, the reverse-mapping
helper disappears, the two-form verifier question disappears.

The cost is a one-time **container format break** (FORMAT_VERSION 1 →
2): containers compiled by older versions of the toolchain will not
load. For a project that ships the compiler and the VM together this
is acceptable — there are no third-party container producers, no
long-lived archive of compiled programs to migrate. Existing tests
recompile their fixtures on every build.

Container size on flash grows ~1.6× (see size table below). For PLC
programs measured in KB, this is negligible; flash is typically the
abundant resource on PLC targets.

**Cache-friendliness is a first-class goal.** The dispatch loop reads
one cell per iteration; if the cell stream fits in L1d (32 KB on
typical cores) the inner loop sees cache-hit latency on every fetch.
4-byte cells × ~1300 cells (a 100-iter FOR-loop body) = ~5 KB, well
under typical 32 KB L1d.

## Approach

### 1. Cell encoding

New module `compiler/container/src/cell.rs` (lives in `container`, not
`vm`, because the container stores cells now). Each instruction is a
**4-byte `u32` "cell"** with the opcode in the low byte and a 24-bit
inline operand in the high bits:

```rust
/// One bytecode cell. The low 8 bits are the opcode; the upper 24 bits
/// are an inline operand (var index, const-pool index, abs cell-index
/// jump target, function-table index, etc.). Wide opcodes are followed
/// by raw u32 trailing cells (see below).
#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(transparent)]
pub struct Cell(pub u32);

impl Cell {
    #[inline] pub fn op(self) -> u8 { self.0 as u8 }
    #[inline] pub fn operand(self) -> u32 { self.0 >> 8 }

    #[inline] pub fn new(op: u8, operand: u32) -> Self {
        debug_assert!(operand <= 0x00FF_FFFF, "operand exceeds 24 bits");
        Cell((operand << 8) | (op as u32))
    }

    #[inline] pub fn raw(value: u32) -> Self { Cell(value) }
}
```

24 bits fit every operand that appears in a FOR-loop body:

- variable index (u16) ✓
- constant-pool index (u16) ✓
- absolute cell-index jump target — 16 M cells of headroom ✓
- function-table index (u16) ✓

**Trailing raw cells for wide opcodes.** The opcodes whose operand
exceeds 24 bits (`CALL` with `u16 + u16`, `STR_INIT` with `u32 + u16`,
and the four 9-byte string ops `FIND_STR`, `REPLACE_STR`, `INSERT_STR`,
`CONCAT_STR` with `u32 + u32`) are followed by **raw `u32` trailing
cells** (full 32-bit slots, not opcode+operand cells). The dispatch
handler reads `cells[ip + N]` directly. Sizing per opcode is fixed and
known from `opcode::cell_width(op)`:

| Opcode class | Source bytes | Cells | Layout |
|---|---|---|---|
| 1-byte (e.g. `ADD_I32`) | 1 | 1 | opcode-cell, operand unused |
| 3-byte (e.g. `LOAD_VAR_I32`) | 3 | 1 | opcode-cell, u16 operand |
| 3-byte `JMP` / `JMP_IF_NOT` | 3 | 1 | opcode-cell, **abs cell index** |
| 5-byte `CALL` (u16+u16) | 5 | 2 | opcode-cell w/ func_id; raw u32 var_offset |
| 7-byte `STR_INIT` (u32+u16) | 7 | 2 | opcode-cell w/ max_length; raw u32 data_offset |
| 9-byte string ops (u32+u32) | 9 | 3 | opcode-cell unused; raw u32; raw u32 |

This is cleaner than the previous plan's `OPCODE_CONTINUATION` sentinel
scheme. Trailing cells carry no opcode field, so there is no risk of a
jump landing "on" one and being interpreted as an instruction — the
verifier (when it lands) only needs to validate that jump targets
appear in the per-function start-cell set, which is computed by a
single forward walk.

> The previous plan's two-cell continuation scheme also undercounted:
> two cells provide 48 operand bits, but the 9-byte string ops carry
> 64 bits of operand. Trailing raw cells fix that natively.

### Size math (the headline number)

Per-iteration FOR-loop body (~13 dispatches, all 1- or 3-byte today):

| Form | Container size | Notes |
|---|---|---|
| Raw bytecode (today) | 1.0× | baseline |
| **4-byte cells (this plan)** | **~1.6×** | single form, on flash and in RAM |
| 12-byte fixed (rejected) | 4.7× | uniform-width strawman |

There is **no separate "decoded" form in RAM**. The container's cell
slice is the dispatch loop's input. Total program-text RAM is the same
as flash: ~1.6× of today's bytecode footprint.

Where the expansion comes from:

1. **Padding small opcodes up to a uniform 4-byte cell** — ~80% of
   FOR-loop opcodes are 1- or 3-byte today; cell padding is the only
   inherent cost of fixed-width dispatch.
2. **Trailing cells for wide ops** — rare in FOR loops, negligible
   overall.

Operand widening is **not** a contributor: u16 indices stay u16, and
24-bit absolute jump targets fit in the same cell slot as today's i16
byte offset.

### 2. Codegen emits cells

Rewrite `compiler/codegen/src/emit.rs`. The `Emitter` becomes a
`Vec<Cell>` instead of `Vec<u8>`. The shape of the emit API stays
similar — call sites pass the same logical operands — but the helpers
pack into cells:

- `emit_op(op)` → push a 1-cell instruction with `operand = 0`.
- `emit_op_u16(op, x)` → push `Cell::new(op, x as u32)`.
- `emit_call(func_id, var_offset)` → push `Cell::new(CALL, func_id as
  u32)` then `Cell::raw(var_offset as u32)`.
- `emit_str_init(data_offset, max_length)` → push
  `Cell::new(STR_INIT, max_length as u32)` then `Cell::raw(data_offset)`.
- `emit_find_str(in1, in2)` → push `Cell::new(FIND_STR, 0)` then
  `Cell::raw(in1)` then `Cell::raw(in2)` (and the same for
  `REPLACE_STR`, `INSERT_STR`, `CONCAT_STR`).

**Label fixup becomes simpler.** The existing `PendingPatch` design
(`compiler/codegen/src/emit.rs:13-17`, `:695-707`) stores the byte
offset of the i16 operand and patches it post-emission. Under cells:

- `bind_label()` records the **cell index** of the next instruction
  (`self.cells.len() as u32`).
- `emit_jmp(label)` / `emit_jmp_if_not(label)` push
  `Cell::new(JMP, 0)` and record `PendingPatch { patch_cell: index,
  target_label: label }`.
- `patch_jumps()` walks pending patches and rewrites
  `cells[patch_cell] = Cell::new(JMP, target_cell_index)`.

Forward jumps need no offset arithmetic — the label resolves to an
absolute cell index, which is what the dispatch loop consumes
directly. Backward jumps work the same way (label was bound earlier
with a known cell index).

The `bytecode()` accessor becomes `cells() -> &[Cell]`.

### 3. Container stores cells

`compiler/container/src/code_section.rs` changes:

- `CodeSection::bytecode: Vec<u8>` → `cells: Vec<Cell>`.
- `FuncEntry::bytecode_offset: u32` (in cells); `length: u32` (in
  cells). The directory entry remains 16 bytes.
- `get_function_bytecode(function_id) -> &[u8]` → `get_function_cells
  (function_id) -> &[Cell]`.
- `Builder::add_function(bytecode: &[u8])`
  (`compiler/container/src/builder.rs:138-161`) →
  `add_function(cells: &[Cell])`. Internally appends to the flat cell
  buffer and records offset/length in cells.

**Format break.** Bump `compiler/container/src/header.rs:10`:
`FORMAT_VERSION: u32 = 2`. The serialization layout changes from
"function directory + flat byte buffer" to "function directory + flat
4-byte-aligned cell buffer". Endianness is little-endian (matches
existing serialization), and on disk a `Cell` is one `u32`. Containers
written under FORMAT_VERSION 1 are rejected at load with a clear
error pointing at the new toolchain.

The `CodeSection` reader/writer (`compiler/container/src/code_section.rs:54-65`)
is replaced; this is the single point where the format change is
visible. The old reader code is removed in this PR — no two-format
support, no migration helper.

### 4. Dispatch loop

Replace `compiler/vm/src/vm.rs:682-688` with:

```rust
let cells: &[Cell] = container.code.get_function_cells(function_id);
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
            let func_id = operand as u16;
            let var_offset = cells[ip].0 as u16;     // raw trailing u32
            ip += 1;
            // ... call setup ...
        }
        opcode::FIND_STR => {
            let in1 = cells[ip].0;                    // raw trailing u32
            let in2 = cells[ip + 1].0;                // raw trailing u32
            ip += 2;
            // ...
        }
        // ... etc
    }
}
```

What disappears compared to today:

- Every `read_u16_le` / `read_i16_le` / `read_u32_le` call in the match
  arms (~30 callsites at `compiler/vm/src/vm.rs:2084-2119`). Operands
  are already in the cell.
- The `(pc as isize + offset as isize) as usize` jump arithmetic — `ip`
  is set directly from the absolute cell index emitted by codegen.
- The `container.code.get_function*` lookup inside `CALL` — replaced
  by a direct function-table index resolved at codegen time.
- The byte-level bounds checks inside `read_*_le` — one slice index
  per **instruction**, not per byte.

What's added per dispatch: one `cell.0 as u8` mask + one `cell.0 >> 8`
shift. Both compile to single instructions on every target.

The helper functions `read_u16_le`, `read_i16_le`, `read_u32_le` are
deleted from `vm.rs`.

### 5. Function calls

The existing `CALL` arm at `compiler/vm/src/vm.rs:1013-1059` recurses
into `execute_with_hook(func_bytecode, ...)`. Under cells: take the
callee's cell slice via `container.code.get_function_cells(func_id)`
and recurse with that. No structural change to the call mechanism —
just the input it consumes. `MAX_CALL_DEPTH = 32` and the
`VariableScope` setup are unchanged.

### 6. `VmBuffers`

`compiler/vm/src/buffers.rs` (`VmBuffers` at lines 16-24) is
**unchanged**. There is no separate decoded buffer to allocate, no
caller-provided `[Cell; N]`, no two construction styles. `Vm::load`
keeps its existing signature — the container already holds cells.

This is the half of the previous plan's complexity that disappears
entirely.

### 7. Hook + profiling semantics

- **`DebugHook::before_instruction(pc, op)`**
  (`compiler/vm/src/debug_hook.rs:21-46`): the first parameter's
  meaning changes from "byte offset of opcode within function" to
  "cell index within function". Document in the doc comment and update
  the test hook.
  - There is **no reverse-mapping helper** to provide. The byte form
    no longer exists. Tooling that wants a human-readable "PC" uses
    the cell index directly.
- **Profiling** (`compiler/vm/src/profile.rs`): unchanged. Still
  records one count per opcode per execution.

### 8. Disassembler and debug tooling

`compiler/project/src/disassemble.rs:231-577` (`decode_instructions`)
is rewritten to walk a `&[Cell]` slice instead of `&[u8]`. The shape
is simpler: per opcode, look up cell width, read inline operand from
the opcode-cell, read trailing cells as raw u32s. The four 9-byte
string ops (FIND_STR, REPLACE_STR, INSERT_STR, CONCAT_STR) currently
fall through to the `unknown` case at line 564; they should be
explicitly decoded in the same PR (small win, easy to do alongside).

VS Code custom-editor and CLI consumers of `disassemble()` and
`disassemble_file()` see no API change — only the internal walker
changes.

The MCP tools and any other in-tree byte-offset consumer follow the
same pattern. Inventory before writing the PR; expect a small handful
of touch points.

### 9. Codegen test fixtures

The codegen tests (`compiler/codegen/tests/compile_*`,
`compile_loops.rs` notably) assert exact byte sequences in many places.
Each such assertion needs to be rewritten in cell form. Two tactics
to keep this manageable:

- Add an `assert_cells!` test helper in
  `compiler/codegen/tests/common/` that prints a cell slice in a
  stable, readable form (`OP_NAME(operand)` or `OP_NAME / raw u32`)
  and diffs against a literal. This is the new equivalent of the
  byte-stream assertion idiom.
- For tests where exact emission isn't the property under test, switch
  to behavioural / round-trip assertions (already the dominant pattern
  in `end_to_end_*.rs`).

Plan to land the helper first, then sweep the assertions in a single
mechanical pass on the same branch.

## Files to change

- `compiler/container/src/cell.rs` — **new**. `Cell` POD type;
  serialization helpers (LE u32 read/write).
- `compiler/container/src/opcode.rs` — add `cell_width(op) -> u8`
  helper (1 / 2 / 3 cells per opcode). Opcode bytes themselves are
  unchanged.
- `compiler/container/src/code_section.rs` — store `Vec<Cell>`;
  rewrite reader/writer for FORMAT_VERSION 2; rename
  `get_function_bytecode` → `get_function_cells`.
- `compiler/container/src/header.rs` — bump `FORMAT_VERSION` 1 → 2.
- `compiler/container/src/builder.rs` — `add_function(cells: &[Cell])`.
- `compiler/codegen/src/emit.rs` — major rewrite. `Emitter` over
  `Vec<Cell>`; new `emit_*` helpers; rework `PendingPatch` to operate
  on cell indices; `JMP` / `JMP_IF_NOT` patch absolute cell indices.
- `compiler/codegen/src/compile_*.rs` — call sites use the new
  emit helpers (mechanical sweep).
- `compiler/codegen/tests/common/mod.rs` (or equivalent) —
  `assert_cells!` helper.
- `compiler/codegen/tests/compile_*.rs` — port byte-literal
  assertions to cell-literal assertions.
- `compiler/vm/src/vm.rs` — rewrite the dispatch loop body around
  `&[Cell]`; delete `read_u16_le` / `read_i16_le` / `read_u32_le`;
  collapse `CALL` to use the function-table index from the cell
  operand; update `JMP` / `JMP_IF_NOT` to assign `ip` directly.
- `compiler/vm/src/debug_hook.rs` — doc-comment change for
  `before_instruction` parameter semantics; update test hook.
- `compiler/vm/src/lib.rs` — re-export `Cell` from `container` for
  callers that build VMs.
- `compiler/project/src/disassemble.rs` — rewrite `decode_instructions`
  over `&[Cell]`; explicit handling for the four 9-byte string ops.

No changes to: opcode set, the verifier (still absent), CLI, playground
public surfaces. `VmBuffers` is unchanged. There is no `Vm::load_releasing`,
no `decode_program`, no `decoded_cell_count`, no `(FunctionId, ip) →
byte_offset` reverse map.

## Risks and open questions

1. **Container format break.** FORMAT_VERSION 1 → 2 means containers
   produced by older toolchains do not load. Acceptable: the project
   ships compiler and VM together, no third-party container producers
   exist. Integration test fixtures recompile from source on every
   build.
2. **Codegen test sweep size.** Many codegen tests assert exact byte
   output. The `assert_cells!` helper plus a mechanical sweep keeps
   this contained. Expect a sizeable diff in the test files; the
   *behaviour* asserted should be unchanged.
3. **Memory cost on flash.** ~1.6× of today's bytecode size. For PLC
   programs measured in KB this is well within budget. Quantify
   exactly during implementation by re-encoding the existing benchmark
   containers and committing actual numbers to the PR.
4. **Cache pressure.** A direct goal, not just a side-effect. 4-byte
   cells × ~1300 cells (a 100-iter FOR-loop body) = ~5 KB, well under
   typical 32 KB L1d. Validate by sampling
   `perf stat -e L1-dcache-load-misses` (or platform equivalent) on
   `st_for_loop` and `st_nested_loops` before/after.
5. **Hook breaking change.** `pc` parameter semantics change from
   byte offset to cell index. Smaller break than the predecode
   design's two-form story (no reverse-map helper exists or is
   needed). Call out in the changelog.
6. **24-bit operand cap.** The emit helpers debug-assert every
   inline operand fits in 24 bits. Variable indices and constant-pool
   indices are u16, well within. Cell-index jump targets cap at 16 M
   cells per function (~64 MB of program text per function) — far
   beyond any realistic PLC. Wide operands go in trailing raw u32
   cells, so the cap doesn't bind on them.
7. **Composes with the future verifier.** When the verifier lands, it
   validates the cell stream directly: jump targets land on opcode
   cells (in the per-function start-cell set computed by one forward
   walk), trailing-cell counts match each opcode's declared width.
   Verifier rules are simpler than they would have been for raw
   bytecode, with no two-form ambiguity. The verified-unchecked
   execution path can later skip the slice index check on
   `cells[ip]`. This plan does not require the verifier; it just
   doesn't get in the verifier's way.
8. **Big PR.** Land the cell type, container format change, codegen
   rewrite, dispatch rewrite, disassembler rewrite, and test sweep
   together. There is no clean intermediate state — the format change
   forces atomicity. Stage internally as a sequence of commits on the
   feature branch, but PR as one. No Cargo feature flag (the previous
   plan needed one because the byte form persisted; here, only one
   form exists at any commit).

## Verification

1. **Behavioural equivalence.** All existing
   `compiler/codegen/tests/end_to_end_*` and `compiler/vm/` integration
   tests must pass unchanged in semantics (assertion *form* changes
   for byte-literal tests; *outcomes* don't). This is the strongest
   signal — if any end-to-end test diverges, semantics drifted.
2. **Codegen unit tests.** Every emit helper has a unit test that
   round-trips a small program through `Emitter` → `cells()` and
   asserts the cell sequence with `assert_cells!`. Cover label
   patching forward and backward; cover all wide opcodes
   (CALL, STR_INIT, the four 9-byte string ops).
3. **Container format round-trip.** Serialize / deserialize each
   benchmark container; assert structural equality and byte-for-byte
   equality of the cell buffer. Reject FORMAT_VERSION 1 with a clear
   error; add a unit test for that path.
4. **FOR-loop microbenchmarks.** Re-run
   `compiler/benchmarks/benches/st_benchmark.rs::st_for_loop` and
   `::st_nested_loops`; target ≥15% on `st_for_loop` 10 000-iter and
   ≥10% on `st_nested_loops` 100×100. Lock in numbers before/after
   in the PR description.
5. **Cache-locality measurement.** On Linux x86_64, `perf stat -e
   L1-dcache-loads,L1-dcache-load-misses,instructions,cycles` over
   `st_for_loop` and `st_nested_loops`. Expect L1d miss rate flat or
   down (cell stream is smaller and stride-1) and IPC up. Commit raw
   counters to the PR.
6. **Profiler invariant.** Re-run
   `compiler/benchmarks/tests/profile_for_loop.rs`; per-opcode counts
   must match the byte-form baseline exactly (we are not changing
   what executes, only how it is encoded and dispatched).
7. **Container size on flash.** Add a benchmark/test that reports
   per-benchmark container size in cell-form vs. recorded byte-form
   baseline. Commit the numbers in the PR for future regression
   tracking.
8. **Disassembler round-trip.** For each benchmark container,
   `disassemble()` produces a sensible textual form covering all
   opcodes including the four 9-byte string ops (which fall through
   to `unknown` today).
9. **Full CI.** `cd compiler && just` — required before any PR.

## Out of scope

- String `copy_from_slice` (separate, smaller, independent win;
  scheduled separately).
- The bytecode verifier (separate plan,
  `specs/plans/2026-04-06-bytecode-verifier.md`).
- Verifier-gated unchecked stack/var access (blocked on verifier and
  on adopting `unsafe`).
- Superinstructions / fused INC_VAR / register translation (folded into
  the in-flight opcode redesign, not addressed here).
- New opcodes of any kind. The opcode set is unchanged; only the
  encoding changes.
- Backward-compatibility for FORMAT_VERSION 1 containers. Hard break.

## Rollout

1. Land this plan on `main`.
2. Implement on a single feature branch as a sequence of commits:
   (a) `Cell` type + opcode width metadata; (b) container reader/writer
   for FORMAT_VERSION 2; (c) `Emitter` rewrite + `assert_cells!` helper;
   (d) codegen test sweep; (e) VM dispatch loop rewrite; (f)
   disassembler rewrite; (g) hook doc/test updates.
3. PR atomically. The format change forces atomicity — there is no
   intermediate commit at which the toolchain end-to-end works in
   both forms.
4. Run the verification suite; commit benchmark deltas, container
   size deltas, and `perf` counters to the PR description.
5. Merge. No follow-up "drop bytecode" PR is needed — there is no
   bytecode to drop. No `predecoded-dispatch` Cargo feature exists or
   needs flipping.
