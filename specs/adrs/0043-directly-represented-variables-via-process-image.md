# ADR-0043: Directly-Represented Variables via the Process Image

## Status

Proposed

## Context

IEC 61131-3 lets a program name hardware directly with **directly-represented
(located) variables** — `%I…` inputs, `%Q…` outputs, and `%M…` memory — using a
location prefix (`I`/`Q`/`M`), a size prefix (`X`/`B`/`W`/`D`/`L`), and an
address (`%IX0.0`, `%QW1`, `%MW10`). These appear two ways in source:

1. **Declared and named** — `sensor AT %IW0 : INT;` — the address is bound to a
   symbol.
2. **Anonymous / inline** — the raw `%IW0` written directly in an expression or
   statement, e.g. `x := %IW0;`, `REF(%IW0)`, `fb(OUT => %QX0.0)`.

**Current state.** Codegen has no model for located memory:

- A *named* located variable gets an ordinary variable-table slot keyed by its
  symbol (`compile_fn.rs`, via `identifier.symbolic_id()`), so it compiles and
  runs — but only as scratch memory. There is no input source and no output
  sink, so `%IW0` reads whatever was last written and `%QX0.0` goes nowhere. The
  I/O semantics are *absent*, not merely incomplete.
- An *anonymous* located address has no symbol, so `resolve_variable()` cannot
  map it to a slot and returns a `NotImplemented` diagnostic
  (`compiler/codegen/src/compile_expr.rs:864-868`, the "Not implemented at
  compile_expr.rs#L867" error users hit). The same gap exists for reads,
  `REF()`, and function-block output targets.

**Specified but unbuilt.** [Runtime Execution Model](../design/runtime-execution-model.md)
already fully specifies a **process image**: three regions (`%I` read-only during
EXECUTE, `%Q` write-only/staged, `%M` read-write), byte-offset formulas per size
prefix, `LOAD_INPUT`/`STORE_OUTPUT`/`LOAD_MEMORY`/`STORE_MEMORY` opcodes, and the
`INPUT_FREEZE → EXECUTE → OUTPUT_FLUSH → IDLE` scan cycle. Today the phases are
`// Stub` no-ops (`compiler/vm/src/vm.rs`), the task-table image offsets are
hardcoded to `0` (`compiler/container/src/builder.rs:342-343`), and no located
opcodes exist. The container header already reserves `input_image_bytes` /
`output_image_bytes` / `memory_image_bytes` (`compiler/container/src/header.rs:79-81`),
all currently `0`.

**Already committed elsewhere.** The [FMI Co-Simulation design](../design/fmi-co-simulation-support.md)
(PR #1258) records, under *Decisions (settled in review)*: *"Build the process
image — IronPLC implements the `%I`/`%Q`/`%M` regions and wires the
`INPUT_FREEZE`/`OUTPUT_FLUSH` phases, rather than mapping I/O straight to flat
variable slots."* An FMU is precisely the "I/O driver" the runtime model leaves
out of scope; it writes `%I` before a step and reads `%Q` after. FMI export
therefore depends on the process image being real.

How should directly-represented variables be implemented?

## Decision Drivers

* **Correct IEC I/O semantics** — inputs frozen for the whole scan, outputs
  staged and flushed atomically, "last-known-good" on fault. Flat slots cannot
  express this.
* **Enable the committed roadmap** — FMI co-simulation (PR #1258) requires the
  process image; a real I/O driver has nothing to bind to without it.
* **Avoid throwaway work** — flat slots for `%I`/`%Q` are the approach the FMI
  design explicitly rejected; building them now is effort we would tear out.
* **Safety and verifiability** — located access must be bounds-checked against
  the region sizes at verification and/or runtime, consistent with
  [ADR-0023 (array bounds safety)](0023-array-bounds-safety.md) and
  [ADR-0006 (bytecode verification)](0006-bytecode-verification-requirement.md).
* **Consistency with the existing spec** — the runtime model already dictates
  the region layout and opcodes; implementation should realize that spec, not
  invent a parallel one.

## Considered Options

### Option A — Flat variable-table slots

Intern each distinct `AddressAssignment` into a synthetic variable-table slot
(the same representation named located variables already get). `resolve_variable`
returns that slot; the anonymous-inline case (L867) is fixed with a small change.

* Good, because it is small and immediately stops the L867 error.
* Good, because the compiler-facing resolution work (interning addresses,
  deriving width from the size prefix, seeding analyzer types) is reusable.
* Bad, because it delivers **no real I/O semantics** — no input snapshot, no
  output staging, no atomicity, no fault last-value guarantee.
* Bad, because there is no I/O driver, so the values are meaningless (reads
  return stale scratch, writes are never observed externally).
* Bad, because it is the approach the FMI design rejected; the `%I`/`%Q`/`%M`
  slot handling would be replaced by the process image later.

### Option B — Process image (chosen)

Realize the process image exactly as [runtime-execution-model.md](../design/runtime-execution-model.md)
specifies: allocate `%I`/`%Q`/`%M` regions, add located-access opcodes, compile
every located/direct variable (named or anonymous) to a region+offset access,
and wire `INPUT_FREEZE`/`OUTPUT_FLUSH` in `run_round`.

* Good, because it gives correct, hardware-faithful I/O semantics.
* Good, because it unblocks FMI co-simulation and any future I/O driver with one
  shared substrate.
* Good, because the L867 anonymous-inline case falls out naturally — a
  `Variable::Direct` resolves to `(region, offset, width)`, no special case.
* Good, because it matches the already-reserved container fields and the
  settled FMI decision — no throwaway.
* Bad, because it is larger and cross-cutting: DSL/analyzer type resolution,
  codegen offset computation, new opcodes, VM regions and phase wiring, verifier
  bounds rules, and container offset population.

### Option C — Hybrid: shared front-end now, image later

Do only the reusable compiler/analyzer resolution now (intern addresses, derive
width, seed types, make `resolve_variable` return an index), deferring the
container/VM/opcode work.

* Good, because it kills L867 quickly with forward-compatible code.
* Bad, because "returns an index" implies flat-slot storage, re-introducing
  Option A's thin semantics as the visible behavior until the image lands.
* Bad, because it splits one coherent feature across two loosely-coupled efforts
  and risks the second half being deprioritized, leaving located variables
  semantically broken indefinitely.

## Decision Outcome

**Chosen: Option B — build the process image.** Directly-represented variables
are implemented against the `%I`/`%Q`/`%M` process image specified by the runtime
execution model, not against flat variable-table slots.

### How located access maps to the image

Each `AddressAssignment` resolves at compile time to a `(region, byte_offset,
access_width)` triple:

| Source part | Resolves to |
|-------------|-------------|
| `LocationPrefix` (`I`/`Q`/`M`) | region: input / output / memory |
| `SizePrefix` (`X`/`B`/`W`/`D`/`L`) | access width: 1 bit / 8 / 16 / 32 / 64 bits |
| `address` vector (e.g. `0.0`, `10`) | byte offset via the runtime-model formula (bit: `idx/8` byte + `idx%8` LSB-first bit; byte: `idx`; word: `idx*2`; dword: `idx*4`; lword: `idx*8`) |

The compiler places located variables into their regions, computes each region's
total byte size, and writes `input_image_bytes` / `output_image_bytes` /
`memory_image_bytes` into the header and the per-task
`input_image_offset` / `output_image_offset` (replacing the hardcoded `0`s).

### Codegen

Located reads/writes compile to the located-access opcodes
(`LOAD_INPUT`/`STORE_OUTPUT`/`LOAD_MEMORY`/`STORE_MEMORY`, encoded per
[ADR-0033](0033-opcode-encoding-by-class-and-type.md)) carrying the region
offset and width — **not** to the scalar-slot load/store opcodes. Both the named
path (`ctx.var_index`) and the anonymous path (`resolve_variable`'s
`Variable::Direct` arm, today L867) route through the same offset resolution, so
the two source forms share one implementation and the L867 TODO is removed.

### Semantics (from the runtime model)

- `%I` is read-only during EXECUTE and is a **frozen snapshot** taken at
  `INPUT_FREEZE`.
- `%Q` writes accumulate in a **staging buffer** during EXECUTE and are handed to
  the I/O driver atomically at `OUTPUT_FLUSH`; on fault, `OUTPUT_FLUSH` is
  skipped, preserving last-known-good.
- `%M` is directly read-write with no double buffering; it persists across scans.
- Writing to `%I` and reading an uninitialized `%Q` are handled per the standard
  (compile-time rejection of `%I` as an assignment target).

### Safety

Located offsets are bounds-checked against the region sizes. Static bounds are
verified by the bytecode verifier
([ADR-0006](0006-bytecode-verification-requirement.md)); this is analogous to
[ADR-0023](0023-array-bounds-safety.md).

### The I/O driver stays out of scope

This ADR implements the process image and the located-access instruction
semantics inside the VM. *Populating* `%I` from hardware and *draining* `%Q` to
hardware remains the platform-specific "I/O driver" the runtime model leaves out
of scope. The first real driver is the FMI shim (PR #1258); a bare CLI run leaves
inputs zero-filled and outputs observable only via the diagnostic interface.

## Consequences

* Good — directly-represented variables (both declared and anonymous inline) work
  with correct IEC I/O semantics; the L867 `NotImplemented` gap closes.
* Good — FMI co-simulation and any future hardware/simulation I/O driver bind to
  one shared, spec-conformant substrate.
* Good — no throwaway: the reserved container fields, the runtime-model regions,
  and the FMI decision all converge on this implementation.
* Neutral — named located variables change storage from a variable-table slot to
  a process-image offset; behavior for existing programs that used them as
  scratch becomes *more* correct (real region semantics) but differs at the
  fault/scan boundary.
* Bad — larger, cross-cutting change (DSL/analyzer/codegen/VM/verifier/container)
  delivered in phases; interim phases compile the regions before the VM fully
  wires the scan phases.
* Bad — new opcodes are a bytecode surface addition governed by the verifier and
  the container format version.

## More Information

- [Runtime Execution Model](../design/runtime-execution-model.md) — the
  authoritative spec for the process image, scan phases, and located opcodes this
  ADR implements.
- [FMI Co-Simulation Support](../design/fmi-co-simulation-support.md) (PR #1258)
  — the committed consumer of the process image; its phase 2 is this work.
- [ADR-0017](0017-unified-data-region.md) — the process image is **distinct**
  from the unified data region: the data region holds variable-length values
  (strings, arrays, structs, FB instances) addressed by `data_offset`, while the
  process image holds fixed I/O regions addressed by located offsets.
- [ADR-0033](0033-opcode-encoding-by-class-and-type.md) — encoding scheme the new
  located-access opcodes follow.
- [ADR-0023](0023-array-bounds-safety.md) / [ADR-0006](0006-bytecode-verification-requirement.md)
  — the bounds-safety and verification model located access conforms to.
- Implementation plan: `specs/plans/2026-07-31-directly-represented-variables-process-image.md`.
