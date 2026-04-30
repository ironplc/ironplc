# Plan: Cell-Encoded Bytecode (4-Byte u32 Cells in Codegen Output)

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

**Encode the bytecode as fixed-width 4-byte u32 cells directly in
codegen output**, with operands inline in the cell, jump targets
emitted as absolute cell indices, and `CALL` targets emitted as
function-table indices. The container stores cells; the VM dispatches
on cells; there is no decode step. The byte-level bounds checks
disappear because there are no per-byte reads, and the per-arm
`read_u16_le`/`read_i16_le`/`read_u32_le` calls disappear because the
operand is already in the cell.

This supersedes an earlier draft of this plan that introduced a runtime
decoder translating today's variable-width bytecode into the cell form
at `Vm::load`. The runtime-decoder approach was strictly worse than
this one: it introduced a two-form architecture (raw bytes + decoded
cells), required a bytecode-release API to avoid carrying both copies
in RAM, and added decode-time validation that the codegen path can
satisfy structurally. Pushing the encoding upstream into codegen
eliminates the duplicate-form problem, simplifies the verifier later
(it validates cells directly), and is one fewer translation stage where
bugs can hide.

**Cache-friendliness is a first-class goal.** The dispatch loop reads
one cell per iteration; if the cell stream fits in L1d (32 KB on
typical cores) the inner loop sees cache-hit latency on every fetch.
The encoding is sized to keep the common case ~1.6× the raw bytecode
rather than the ~4-5× a naïve fixed-width design would cost. There is
no second copy of the program text — the container *is* the runtime
form — so total program-text RAM lands at ~1.6× today's bytecode size,
not the ~5-6× a "two copies" design would impose.

**Constraint preserved:** no new opcodes. This plan changes how
existing opcodes are *encoded*, not what opcodes exist. It composes
with the team's in-flight opcode work — when superinstructions land on
the opcode set, they slot into the cell encoding the same way every
other opcode does.

## Approach

### 1. Cell encoding

New module `compiler/container/src/cell.rs` (lives in `container` so
both codegen and vm depend on it without a cycle). Each instruction is
a **4-byte `u32` "cell"** with the opcode in the low byte and a 24-bit
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

| Encoding | Container (flash) | Runtime program-text RAM |
|---|---|---|
| Variable-width bytecode (today) | 1.0× | 1.0× |
| **4-byte cells (this plan)** | **1.6×** | **1.6×** |
| 12-byte fixed cells (rejected) | 4.7× | 4.7× |

For a 10 KB container today: ~16 KB on flash and ~16 KB in RAM after
this change. Container *is* the runtime form, so flash and RAM scale
identically. The growth is **+0.6× of bytecode size**, end-to-end, with
no decode pass and no two-form duplication.

Where the expansion comes from, in decreasing order of impact:

1. **Padding small opcodes up to a uniform 4-byte cell** — ~80% of
   FOR-loop opcodes are 1- or 3-byte today; cell padding is the only
   inherent cost of fixed-width dispatch.
2. **Continuation cells for the 5/9-byte opcodes** — rare in FOR loops,
   negligible overall.

Operand widening is **not** a contributor: u16 indices stay u16, and
24-bit absolute jump targets fit in the same cell slot as today's i16
byte offset.

### 2. Codegen emits cells

`compiler/codegen/src/emit.rs` is the single point where bytes get
written today. The change:

- **Buffer type.** `Emitter.bytecode: Vec<u8>` becomes
  `Emitter.cells: Vec<u32>` (or `Vec<Cell>`). Every `emit_*` helper
  switches from byte writes to cell writes. The big win is that the
  current pattern of "write opcode byte, then little-endian-write a
  u16 operand" collapses to a single `cells.push(Cell::new(op,
  operand))`.
- **Label resolution.** `Emitter`'s existing fixup machinery
  (`PendingPatch` at `emit.rs:13-17`, `create_label`/`bind_label` at
  `emit.rs:435,444`, fixups in `finish()` at `emit.rs:587+`) keeps the
  same shape but operates on **cell indices**. Forward-reference jumps
  emit a placeholder cell with the opcode set and the operand left
  zero; on `bind_label`, the patcher writes the absolute target cell
  index back into the high 24 bits of the placeholder cell. The
  source-side `i16` byte-offset arithmetic disappears entirely.
- **Continuation cells.** The four wide opcodes (`CALL`, `FIND_STR`,
  `REPLACE_STR`, `INSERT_STR`, `CONCAT_STR`) emit two cells: the lead
  cell carries the opcode plus first operand fragment; the continuation
  cell uses `op = OPCODE_CONTINUATION` plus the rest of the operand in
  its high 24 bits.
- **`CALL` target encoding.** Codegen already knows the callee's
  `FunctionId`; it emits that as the lead cell's operand. No further
  pre-resolution is needed — the runtime function table is the
  container's existing function directory (`code_section.rs:46-51`).
- **Existing optimisations port over unchanged.**
  - TRUNC elision (`compile_stmt.rs:821-981`): emits or doesn't emit a
    cell, same logic, no offset math.
  - FOR-loop exit-`JMP` elision (already shipped): same logic, just
    one cell saved instead of three bytes.
  - DUP-based load-elimination peephole (`emit.rs:30,156`): tracks the
    last emitted cell instead of the last byte/operand pair; logic
    survives intact.
  - Post-emission identity optimiser (`optimize.rs`): walks cells with
    a stride of 1 instead of `instruction_size(op)`. Jump-offset
    recalculation after a deletion becomes "subtract 1 from any cell
    index pointing past the deletion" — strictly simpler than today's
    byte-offset shift logic. The `jump_targets: HashSet<usize>` now
    holds cell indices rather than byte offsets.

The Emitter's outward contract (the `emit_*` helpers compile_stmt.rs
calls) doesn't change shape; only the internal buffer type and the
underlying byte-vs-cell write does.

### 3. Dispatch loop

Replace the byte-driven loop in `compiler/vm/src/vm.rs:682` with:

```rust
let cells: &[Cell] = container.code.get_function_cells(current_function)?;
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
            let func_id = operand as u16;                 // lead cell
            let var_offset = cells[ip].operand() as u16;  // continuation
            ip += 1;
            // ...
        }
        // ... etc
    }
}
```

What disappears compared to today:

- Per-byte bounds checks: `bytecode[pc]` plus the 2-3 byte reads inside
  every `read_u16_le`/`read_u32_le` collapse into one bounds check per
  cell (`cells[ip]`).
- Every `read_u16_le` / `read_u32_le` call in the match arms — operands
  are already in the cell.
- The `(pc as isize + offset as isize) as usize` jump arithmetic — `ip`
  is set directly from the absolute cell index codegen emitted.

What's added per dispatch: one `cell.0 as u8` mask + one `cell.0 >> 8`
shift. Both compile to single instructions on every target.

### 4. Function calls

The existing `CALL` arm (`vm.rs:1013`) recurses into
`execute_with_hook(func_bytecode, ...)`. Under the new scheme: same
recursion, but the callee's slice comes from
`container.code.get_function_cells(FunctionId)` — a renamed/retyped
sibling of the current `get_function_bytecode`. The function directory
in the container (`code_section.rs:13-20`) keeps the same shape; only
the units of `bytecode_offset`/`bytecode_length` change from byte
counts to cell counts.

### 5. Container storage

`compiler/container/src/code_section.rs` is the on-disk and in-memory
home of bytecode today. The change:

- **In-memory type.** `CodeSection.bytecode: Vec<u8>` becomes
  `CodeSection.cells: Vec<u32>`. The accessor
  `get_function_bytecode(function_id) -> Option<&[u8]>` becomes
  `get_function_cells(function_id) -> Option<&[Cell]>`. The bulk
  buffer + per-function directory pattern stays.
- **`FuncEntry` semantics.** `bytecode_offset` and `bytecode_length`
  reinterpret as **cell offset** and **cell count**. The fields stay
  u32 on disk; only their meaning changes. (Note: this means a v1
  container's `bytecode_length` is in bytes; a v2 container's is in
  cells. The version bump below disambiguates.)
- **On-disk format.** The container header at
  `bytecode-container-format.md:60-97` carries `format_version` (REQ-CF-003).
  Bump from `1` to `2`. Loaders refuse v1 containers under the new
  codepath; a separate `iplc-migrate v1-to-v2` tool (out of scope here,
  trivial follow-up) can re-emit existing artefacts.
- **Endianness.** REQ-CF-004 already fixes little-endian for
  multi-byte values; cells extend that — each cell is a u32 LE.
- **`instruction_size()` retires.** `compiler/container/src/opcode.rs:567`
  becomes a constant `1` (or just deleted; callers know the stride).

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

### 7. Disassembler and tooling

Three call sites consume the raw bytecode today and need updates to
walk cells:

- **`compiler/project/src/disassemble.rs`** — the JSON disassembler.
  Replaces its `instruction_size`-driven walk with a stride-1 cell walk;
  switches its operand-extraction from `read_u16_le`/etc. to
  `cell.operand()`. The output schema (JSON shape) stays the same;
  source-byte-offset fields, where present, become cell-index fields.
- **MCP tools that surface bytecode/disassembly** (e.g.
  `compiler/mcp/src/tools/compile.rs`, `tools/run.rs`) — port to
  whatever disassemble.rs exposes.
- **CLI `iplc disassemble` (or equivalent)** — same rename: byte
  offsets become cell indices in the output.

There is no two-form ownership question to resolve (the container *is*
the runtime form), so no `Vm::load` / `Vm::load_releasing` API split is
needed. `Vm::load(container, bufs)` keeps its current shape; only the
underlying type held by the container changes.

## Files to change

**Container:**
- `compiler/container/src/cell.rs` — **new**. `Cell` POD type with
  `op()`/`operand()`/`new()`; the `OPCODE_CONTINUATION` sentinel
  constant.
- `compiler/container/src/code_section.rs` — `bytecode: Vec<u8>` →
  `cells: Vec<u32>`; rename `get_function_bytecode` to
  `get_function_cells`; reinterpret `FuncEntry.bytecode_offset` /
  `bytecode_length` as cell counts; bump `format_version` from 1 to 2;
  update `write_to`/`read_from` accordingly.
- `compiler/container/src/opcode.rs` — retire `instruction_size`
  (becomes constant 1 for non-continuation cells, or delete and inline);
  reserve `OPCODE_CONTINUATION` value.
- `compiler/container/src/lib.rs` — export `Cell`.

**Codegen:**
- `compiler/codegen/src/emit.rs` — `Emitter.bytecode: Vec<u8>` →
  `Emitter.cells: Vec<u32>`; rewrite every `emit_*` helper; update
  `PendingPatch`/`create_label`/`bind_label`/finish-fixups to operate
  on cell indices; emit continuation cells for `CALL` and the wide
  string opcodes; the DUP/store-load peephole (`emit.rs:30,156`)
  tracks the last-emitted cell.
- `compiler/codegen/src/optimize.rs` — replace `instruction_size`-driven
  walk with stride-1 cell walk; rewrite jump-offset adjustment in cell
  units (becomes simpler); `jump_targets: HashSet<usize>` holds cell
  indices.
- `compiler/codegen/src/compile_stmt.rs`, `compile_expr.rs`, etc. — no
  changes to the surface they call (Emitter helpers); pre-existing
  optimisations (TRUNC elision, FOR-loop exit-`JMP` elision) keep
  working.

**VM:**
- `compiler/vm/src/vm.rs` — rewrite the dispatch loop around `&[Cell]`;
  delete every `read_u16_le`/`read_i16_le`/`read_u32_le` call site;
  collapse `JMP`/`JMP_IF_NOT` to direct `ip` assignment; rewrite
  `CALL`'s function lookup against `get_function_cells`. Largest diff.
- `compiler/vm/src/debug_hook.rs` — doc-comment change: hook
  parameter is now cell index; update test hook.
- `compiler/vm/Cargo.toml` — no dep changes expected.

**Tooling:**
- `compiler/project/src/disassemble.rs` — stride-1 cell walk; operand
  extraction via `cell.operand()`; rename byte-offset fields to
  cell-index fields in JSON output.
- `compiler/mcp/src/tools/compile.rs`, `tools/run.rs`, and any other
  MCP tool that surfaces bytecode/disassembly — port to the new
  disassembler interface.
- `compiler/ironplc-cli/` — any disassemble subcommand likewise.

**Tests** (see also "Spec tests" below):
- `compiler/codegen/tests/compile_loops.rs`, `compile_if.rs`,
  `compile_case.rs`, `compile_dup.rs`, `compile_array.rs` — 15 byte-
  level `assert_eq!(bytecode, &[...])` assertions across these 5 files
  need rewriting against the cell encoding (assertions become
  `assert_eq!(cells, &[Cell::new(op, operand), ...])` or equivalent).

No changes to: opcode set itself (the values `0x10`, `0x6B`, `0xB2`
etc. stay), verifier (still absent — landing it on top is a separate
plan), playground (consumes containers; the format_version bump is
transparent once it loads v2 containers).

## Design docs to update

The following live in `specs/design/` and `specs/adrs/` and either
encode assumptions this plan invalidates or describe the format
this plan reshapes. Edits are part of the implementation PR(s); each
edit also gets a REQ-level audit (see "Spec tests" below).

- **`specs/design/bytecode-instruction-set.md`**
  - **§Encoding (lines 15-26)** — full rewrite. Today's table of
    operand types (u8/u16/i16/i32/u32 immediately following the opcode
    byte) is replaced by a single description: every instruction is a
    4-byte u32 cell, opcode in low 8 bits, 24-bit inline operand in
    high bits, with a continuation-cell rule for the wide opcodes.
  - **§Opcode Summary (line 625)** — keep the opcode byte values; add a
    column noting which opcodes take a continuation cell.
  - **§Compilation Examples (line 656)** — re-render the worked
    examples in cell form rather than byte form.
  - **§Out of Scope for Version 1 (line 850)** — note the cell encoding
    landed in container `format_version = 2`.
- **`specs/design/bytecode-container-format.md`**
  - **REQ-CF-003 (line 65)** — `format_version` flips from `1` to `2`.
  - **REQ-CF-004 (line 23)** — extend the little-endian rule explicitly
    to cells.
  - **Per-function bytecode section** — `bytecode_offset` /
    `bytecode_length` are now cell counts, not byte counts.
  - Add a short "Migration from v1" subsection pointing at the
    `iplc-migrate` follow-up.
- **`specs/design/runtime-execution-model.md`**
  - **§VM Lifecycle** — clarify that load is now zero-copy with respect
    to bytecode (no decode pass); the container's cell array is the
    runtime form.
  - **§Design Goals item 2 ("Bounded resource usage")** — restate: no
    runtime allocation for program text; the decoded form *is* the
    on-disk form.
- **`specs/design/vm-performance.md`**
  - **§Tier 1 §1 (verification-gated unchecked ops)** — note that the
    cell encoding makes the verifier's structural rules simpler (cell
    index bounds replace byte-offset arithmetic; jump-target validity
    becomes a simple cell-index range check).
  - **§Tier 1 §4b (opcode consolidation for icache)** — partly
    superseded by the cell-stream cache discussion in this plan; cross-
    reference and trim.
  - Add a short forward-pointer: superinstructions, when they land,
    encode as plain cells like every other opcode.
- **`specs/design/bytecode-verifier-rules.md`**
  - Rewrite rules that reference "byte offsets" to reference "cell
    indices". `R0001` (valid opcodes), `R0400` (jump-target validity),
    `R0001` neighbours that talk about operand bounds become naturally
    simpler under cells.
- **`specs/design/no-std-vm.md`**
  - No content change needed (cells are no_std-friendly by
    construction), but cross-reference: the encoding makes the load
    path zero-allocation by default.
- **ADR (new)** — add `specs/adrs/0009-fixed-width-cell-encoding.md`
  capturing the decision: superseded variable-width byte encoding
  with fixed 4-byte u32 cells; alternatives considered (12-byte fixed,
  runtime decode, status-quo); chosen for cache locality and verifier
  simplicity.

## Spec tests

The project links tests to numbered requirements via the
`#[spec_test(REQ_XX_NNN)]` proc-macro
(`compiler/spec_test_macro/`, registry in
`compiler/container/src/spec_conformance.rs`). The build fails if a
declared `REQ_*` constant has no matching `#[spec_test]`. So every new
or rewritten REQ in the design docs above needs a paired test.

**New requirements to introduce** (proposed REQ IDs; finalise during
PR review):

| Req | Where it's defined | What the test asserts |
|---|---|---|
| REQ-IS-100 | `bytecode-instruction-set.md` §Encoding | Round-trip: encode a sample program, decode each cell, recover `(op, operand)` exactly. |
| REQ-IS-101 | same | `OPCODE_CONTINUATION` is reserved (no real opcode collides). |
| REQ-IS-102 | same | Wide opcodes (`CALL`, `FIND_STR`, `REPLACE_STR`, `INSERT_STR`, `CONCAT_STR`) emit exactly one continuation cell each. |
| REQ-IS-103 | same | Codegen never emits a `JMP`/`JMP_IF_NOT` whose target lands on a continuation cell. |
| REQ-IS-104 | same | Single-cell operands fit in 24 bits; codegen rejects overflow. |
| REQ-CF-100 | `bytecode-container-format.md` | `format_version == 2` for cell-encoded containers; v1 containers are rejected. |
| REQ-CF-101 | same | `FuncEntry.bytecode_offset` and `bytecode_length` are cell counts in v2 containers. |
| REQ-CF-102 | same | Each cell on disk is a u32 little-endian. |
| REQ-EM-100 | `runtime-execution-model.md` §VM Lifecycle | `Vm::load(container_v2)` performs no allocation for the program text. |

**Existing tests that need updating** (not requirements changes; just
adapting assertions):

- `compiler/codegen/tests/compile_loops.rs` (FOR/WHILE/REPEAT layout
  asserts at byte level, including the FOR-loop tests added in the
  recent exit-`JMP` elision).
- `compiler/codegen/tests/compile_if.rs`, `compile_case.rs`,
  `compile_dup.rs`, `compile_array.rs` — same pattern.

Total: 15 byte-level `assert_eq!(bytecode, &[0x10, 0x00, 0x00, ...])`
sites. They become `assert_eq!(cells, &[Cell::new(LOAD_VAR_I32, 0),
...])`. The rewrite is mechanical but bulky — budget time accordingly.

**Existing spec_conformance.rs registrations** — extend with the new
`REQ_IS_100`-`REQ_IS_104`, `REQ_CF_100`-`REQ_CF_102`, `REQ_EM_100`
constants. The build break if a `REQ_*` lacks a `#[spec_test]` is the
project's enforcement mechanism; rely on it.

## Risks and open questions

1. **Container format version bump.** v2 containers can't be loaded
   by v1 runtimes and vice versa. For a pre-1.0 project this is fine,
   but call it out in the release notes. Provide a one-shot
   `iplc-migrate v1-to-v2` tool (or an `iplc compile --reemit`
   command) so anyone with stored v1 artefacts can upgrade.
2. **Coordination with in-flight opcode work.** Whoever is mid-flight
   on opcode changes will want to land their changes against either
   the v1 or v2 encoding, not both. Sequencing options: (a) freeze
   opcode changes for the duration of the cell-encoding PR, or (b)
   land opcode changes first and rebase the cell encoding on top.
   Picking (b) is usually less painful: the cell PR doesn't care which
   opcodes exist, only how they're encoded.
3. **Test rewrite blast radius.** ~15 byte-level assertions across 5
   codegen test files plus any indirect golden files. Mechanical, but
   the diff is big enough that it dwarfs the actual logic change in
   review. Mitigation: structure the codegen PR so the test rewrite
   is a separate commit immediately after the encoding switch.
4. **Hook breaking change.** `DebugHook::before_instruction(pc, op)`
   parameter semantics change from byte offset to cell index. Public-
   API break; callout in changelog plus a `(FunctionId, ip) ->
   byte_offset_in_v1_container` helper for tools that need to
   cross-reference v1 artefacts.
5. **24-bit operand cap.** Variable indices and constant-pool indices
   are u16 (well under 24 bits). Cell-index jump targets cap at 16 M
   cells per function — ~64 MB of decoded text per function, far
   beyond any realistic PLC. Codegen surfaces a hard error
   (`Problem::CellOperandOverflow` or similar) on the off chance some
   pathological input triggers it.
6. **Composes with the future verifier.** Verifier validates cells:
   jump targets land on real cell starts (not continuation cells),
   operands fit expected widths, continuation cells follow only the
   wide opcodes that produce them. Verifier rules end up simpler
   under cells than under variable-width bytes. This plan doesn't
   require the verifier; it just shapes the surface the verifier will
   target.
7. **Big PR.** Single coherent change — cells in codegen, container
   storage, VM dispatch, disassembler, tests, design docs. Cargo
   feature-gating doesn't cleanly split this: the container format
   itself differs. Plan to land in 2-3 sequenced PRs (encoding
   primitive + codegen; VM dispatch + tooling; design docs + spec
   test wiring), not as one mega-PR.

## Verification

1. **Spec tests for the new REQs** (see "Spec tests" above). Each
   `REQ_IS_*` / `REQ_CF_*` / `REQ_EM_*` constant gets a paired
   `#[spec_test(...)]`. Cover every opcode class: 1-byte, 3-byte,
   5-byte, 7-byte, 9-byte (the last two emit continuation cells).
   Negative tests for jump-onto-continuation-cell, operand overflow,
   unknown opcode, and v1-container-rejection.
2. **Behavioural equivalence.** All existing
   `compiler/codegen/tests/end_to_end_*` and `compiler/vm/` integration
   tests pass unchanged. This is the strongest signal — if any
   end-to-end test diverges, semantics drifted somewhere in the
   codegen→VM path.
3. **Container roundtrip.** Compile a representative program, write
   the container to disk, read it back, run it; bit-identical compared
   to in-memory execution. Asserts the v2 serialisation/deserialisation
   path is symmetric.
4. **FOR-loop microbenchmarks.** Re-run
   `compiler/benchmarks/benches/st_benchmark.rs::st_for_loop` and
   `::st_nested_loops`; target ≥15% on `st_for_loop` 10 000-iter and
   ≥10% on `st_nested_loops` 100×100. Lock in numbers before/after in
   the PR description.
5. **Cache-locality measurement.** On Linux x86_64, `perf stat -e
   L1-dcache-loads,L1-dcache-load-misses,instructions,cycles` over
   `st_for_loop` and `st_nested_loops`. Expect the L1d miss rate to
   stay flat or drop and IPC to improve. Commit the raw counters to
   the PR.
6. **Profiler invariant.** Re-run
   `compiler/benchmarks/tests/profile_for_loop.rs`; per-opcode counts
   match the byte-driven baseline exactly (we are not changing what
   executes, only how it's encoded).
7. **Container size measurement.** For each benchmark program, record
   v1 byte-encoded container size and v2 cell-encoded container size.
   Expect ~1.6×; treat anything outside [1.4×, 1.8×] as a signal to
   investigate. Commit the numbers in the plan/PR for regression
   tracking.
8. **Full CI.** `cd compiler && just` — required before any PR per
   `CLAUDE.md`.

## Out of scope

- **String `copy_from_slice`** — separate, smaller, independent win.
  Scheduled separately.
- **The bytecode verifier** — separate plan,
  `specs/plans/2026-04-06-bytecode-verifier.md`. Cell encoding makes
  verifier rules simpler but doesn't require the verifier.
- **Verifier-gated unchecked stack/var access** — blocked on verifier
  and on adopting `unsafe`.
- **Superinstructions / fused INC_VAR / register translation** —
  belong in the in-flight opcode redesign. They encode as plain cells
  like every other opcode; this plan doesn't constrain them.
- **New opcodes** — not introduced by this plan. Existing opcode
  values (`0x10`, `0x6B`, `0xB2`, …) retain their meaning.
- **`iplc-migrate v1-to-v2`** — sketched in Rollout PR 3 but the tool
  itself is a follow-up.

## Rollout

The container format change makes Cargo-feature gating impractical (the
bytes on disk differ). Sequence as a small chain of focused PRs rather
than one mega-PR:

1. **Land this plan on `main`.** Coordinate with the in-flight opcode
   work to pick a sequencing window — recommend they land first, this
   rebases on top.
2. **PR 1: Encoding primitive + codegen.** Add
   `compiler/container/src/cell.rs`. Switch
   `compiler/codegen/src/emit.rs` and `optimize.rs` to cells. Update
   the 15 byte-level codegen test assertions in the same commit so the
   suite stays green. Container side: `code_section.rs` switches its
   in-memory type and bumps `format_version`. The VM side temporarily
   adapts by reading the cell stream as a byte stream during this PR,
   or feature-gates dispatch — whichever is shorter.
3. **PR 2: VM dispatch + tooling.** Rewrite `vm.rs:682` dispatch loop;
   port `disassemble.rs` and the MCP tools; update `debug_hook.rs`
   semantics. After this PR, the cell encoding is end-to-end live.
4. **PR 3: Design docs + spec test wiring.** Edit the design docs
   listed above; add `REQ_IS_*`, `REQ_CF_*`, `REQ_EM_*` constants in
   `spec_conformance.rs` with paired `#[spec_test]`s. Add the
   `specs/adrs/0009-fixed-width-cell-encoding.md` ADR. Provide an
   `iplc-migrate v1-to-v2` (or `iplc compile --reemit`) tool for any
   stored v1 artefacts.
5. **Each PR runs `cd compiler && just` clean before merge.**
