# Design: Functional Mock-up Interface (FMI) Co-Simulation Support

## Status

**Draft — for review.** This document is an architecture and feasibility
proposal. It contains a set of [Open Questions](#open-questions) whose answers
shape the implementation. Reviewers are asked to answer those inline as PR
feedback; the design will be finalized from the answers before any
implementation plan is written under `specs/plans/`.

## Overview

This document proposes exporting an IronPLC program as a **Functional Mock-up
Unit (FMU)** implementing the **Functional Mock-up Interface (FMI)** for
**co-simulation**, so that IronPLC can participate in the Open Industry Project
(and any FMI-compatible simulation master such as OMSimulator, or the
co-simulation environments used for virtual commissioning).

The central finding is that IronPLC's VM is unusually well-suited to FMI
co-simulation. FMI export is normally hard for a PLC runtime for two reasons:
the runtime owns its own clock (a scan thread you must fight), and it drives
physical I/O directly (nothing to redirect into a simulation). IronPLC's VM was
designed with **both** of these as *external* boundaries — the clock is a
parameter to the scan function, and the I/O boundary was deliberately left as a
pluggable "I/O driver" the VM only touches at two well-defined points. The
recent task/scheduler and time-control work supplies the co-simulation time
contract almost directly.

I/O is the one genuinely incomplete piece, but it is **not a blocker**: there is
a minimal path that maps FMI variables onto the existing flat variable table and
needs no process-image implementation at all.

### Building On

- **[Runtime Execution Model](runtime-execution-model.md)** — VM lifecycle, the
  `INPUT_FREEZE → EXECUTE → OUTPUT_FLUSH → IDLE` scan cycle, the process image,
  the `simulated` clock source, and the (read-only) diagnostic interface. FMI
  export extends this model; the FMU *is* the "I/O driver" that spec leaves out
  of scope.
- **[IEC 61131-3 Task Support](61131-task-support.md)** — the task table,
  cyclic/freewheeling scheduling, and the externally-driven `run_round` model.
- **[Debug Info in the iplc Container](debug-info-in-iplc-container.md)** and
  **[Debugger Support](debugger-support.md)** — the variable-name/scope/type
  metadata FMI needs to generate `modelDescription.xml`, and the existing
  external variable read/write path.
- **[No-std VM](no-std-vm.md)** / **ADR-0010** — the commitment that the VM core
  stays `unsafe`-free and embeddable. This constrains where the FMI C ABI lives
  (see [The FMI ABI shim](#component-3-the-fmi-abi-shim-crate)).

## Design Goals

1. **A standards-conformant FMU** — a compiled IronPLC program packages into a
   valid `.fmu` (FMI for Co-Simulation) that a standard master can instantiate,
   initialize, step, and read, with no IronPLC-specific tooling on the master
   side.
2. **The VM core stays `unsafe`-free** — the C ABI (`extern "C"`, raw pointers)
   is inherently `unsafe`. It must be quarantined in a thin, auditable boundary
   crate that lives outside the workspace `unsafe_code = "deny"` lint. The
   `ironplc-vm` crate is not modified to add `unsafe`.
3. **Deterministic co-simulation** — given the same input trajectory and the
   same sequence of communication points, the FMU produces identical outputs.
   This follows directly from the VM's `simulated` clock model.
4. **Reuse, don't fork, the execution core** — FMI is a new *driver* around the
   existing `VmRunning` state machine, exactly as the DAP debugger is. No
   changes to bytecode, codegen semantics, or the instruction set.
5. **Incremental** — a first working FMU should not require implementing the
   full process image. I/O semantics can be upgraded later without changing the
   FMU's external contract.

## Background: What FMI Co-Simulation Requires

An FMU for co-simulation is a zip archive (`.fmu`) containing:

- `modelDescription.xml` — declares every exposed variable with a stable
  integer **value reference**, a **causality** (`input` / `output` /
  `parameter` / `local`), a **variability**, a data type, and start values.
- `binaries/<platform>/<name>.{so,dll,dylib}` — a shared library exporting the
  FMI C API.
- `resources/` — arbitrary files the binary may load at instantiation.

The co-simulation C API is a small lifecycle (names shown for FMI 2.0; FMI 3.0
is analogous):

| FMI call | Meaning |
|----------|---------|
| `fmi2Instantiate` / `fmi2FreeInstance` | Create / destroy an instance |
| `fmi2SetupExperiment`, `fmi2EnterInitializationMode`, `fmi2ExitInitializationMode` | Configure start/stop time and run initialization |
| `fmi2SetReal/Integer/Boolean/String` | Write inputs (and tunable parameters) by value reference |
| `fmi2GetReal/Integer/Boolean/String` | Read outputs (and locals) by value reference |
| `fmi2DoStep(t_c, h, ...)` | Advance the model from communication point `t_c` by step size `h` |
| `fmi2Terminate`, `fmi2Reset` | End the run / return to instantiated state |

The master owns time: it decides `t_c` and `h` and calls `fmi2DoStep`
repeatedly. The FMU may internally sub-step but must not sleep or consult a
wall clock.

## Why the Architecture Fits

The four things co-simulation needs map onto capabilities that already exist.

### 1. The master owns the clock — already true

`VmRunning::run_round(current_time_us)` (`compiler/vm/src/vm.rs:339`) takes time
as an **argument**. The VM never sleeps — the sleep loop lives in the CLI
(`compiler/vm-cli/src/cli.rs:86-92`), not the VM. The scheduler is explicitly
documented as "time-agnostic: callers pass the current time as a `u64`
microsecond value … fully testable without mocking clocks"
(`compiler/vm/src/scheduler.rs:30-33`). The runtime model already specifies a
`simulated` clock source for "deterministic replay"
(`runtime-execution-model.md`, VM Configuration Parameters / Runtime Clock).

This is precisely `fmi2DoStep`'s contract. No scan thread has to be suppressed;
the time boundary is already a function parameter.

### 2. Set / get variables by index — mostly present

`VmRunning` exposes `read_variable`, `read_variable_raw`, `read_variable_i64`,
and `write_variable` (`compiler/vm/src/vm.rs:585-606`), addressing values by
`VarIndex`. This is the same external access path the DAP debugger uses. It maps
directly onto `fmi*Set*` / `fmi*Get*` keyed by value reference.

### 3. Per-instance buffers — clean multi-instantiation

`VmBuffers::from_container` allocates per-instance state while the `Container` is
read-only and shared. An FMI master that instantiates several copies of the same
FMU simply constructs several `VmBuffers` over one shared container. There is no
global mutable runtime state to untangle.

### 4. Variable metadata for `modelDescription.xml` — already persisted

The debug section's `VarNameEntry` stores `var_index`, `function_id`,
`var_section` (`VAR` / `VAR_INPUT` / `VAR_OUTPUT` / `VAR_IN_OUT` /
`VAR_GLOBAL` / …), `iec_type_tag`, `name`, and `type_name`
(`compiler/container/src/debug_section.rs:11-30`). That is enough to derive:
value reference = `var_index`, causality from `var_section` (and/or the located
`%I` / `%Q` name prefix), and FMI type from `iec_type_tag`.

## Current State of I/O

Located variables (`AT %IX0.0`, `%QX0.0`, `%MW10`) parse and type-check, but
today they compile to **ordinary flat variable-table slots**. The analyzer
renames them to symbols like `%IX0.0` (`format_address` in
`compiler/analyzer/src/xform_resolve_symbol_and_function_environment.rs:330-346`)
and they are read/written with the same `LOAD_VAR`/`STORE_VAR` instructions as
any other variable.

The full process image described in the runtime model — separate `%I` / `%Q` /
`%M` byte regions, `LOAD_INPUT` / `STORE_OUTPUT` opcodes, and the
`INPUT_FREEZE` / `OUTPUT_FLUSH` copies — is **specified but not implemented**.
Evidence:

- No `LOAD_INPUT` / `STORE_OUTPUT` / `LOAD_MEMORY` / `STORE_MEMORY` opcodes exist
  in `compiler/container/src/opcode.rs`.
- `run_round` contains literal `// Stub: INPUT_FREEZE (no-op)` and
  `// Stub: OUTPUT_FLUSH (no-op)` markers (`compiler/vm/src/vm.rs:357,410`).
- `input_image_offset` / `output_image_offset` are container fields hardcoded to
  `0` (`compiler/container/src/builder.rs:342-343`); they round-trip through the
  format but nothing consumes them.
- The runtime model explicitly lists the "I/O driver model" as **out of scope**
  and states the VM interacts with I/O "only during INPUT_FREEZE and
  OUTPUT_FLUSH" (`runtime-execution-model.md`, Out of Scope §5).

That last point is the key architectural observation: **an FMU is exactly the
pluggable "I/O driver" the runtime model deliberately left out.** The FMI
boundary has a natural home whether or not the process image is built.

## Proposed Architecture

```
   ┌─────────────────────────────────────────────────────────────┐
   │  FMI Master (OMSimulator, OIP co-sim, virtual commissioning) │
   └───────────────┬──────────────────────────────┬──────────────┘
        fmi3DoStep │ fmi3Set*/Get*                 │ loads
                   ▼                               ▼
   ╔═══════════════════════════════╗   ┌──────────────────────────┐
   ║  ironplc-fmi (cdylib, UNSAFE) ║   │  the .fmu archive        │
   ║  extern "C" fmi3* exports     ║   │   modelDescription.xml   │
   ║  · valueRef → VarIndex table  ║   │   binaries/<plat>/*.so   │
   ║  · time accumulator (µs)      ║   │   resources/program.iplc │
   ║  · lifecycle → VM states      ║   └──────────────────────────┘
   ╚═══════════════╤═══════════════╝              ▲
       safe calls  │                              │ generated at build time by
                   ▼                              │
   ┌───────────────────────────────┐   ┌──────────────────────────┐
   │  ironplc-vm (SAFE, unchanged  │   │  modeldescription gen     │
   │  core): Vm→Ready→Running→…    │   │  (reads debug/interface   │
   │  run_round(t_us), read/write  │   │   section of the container)│
   └───────────────────────────────┘   └──────────────────────────┘
```

Four new components; the VM core is reused as-is.

### Component 1: Variable model and value-reference mapping

The FMU's exposed variables are the program's interface. Proposed mapping:

| IEC source | FMI causality | Notes |
|------------|---------------|-------|
| `AT %I…` located var, or `VAR_INPUT` at program scope | `input` | Written by the master before `DoStep` |
| `AT %Q…` located var, or `VAR_OUTPUT` at program scope | `output` | Read by the master after `DoStep` |
| `VAR_GLOBAL` (non-located) | `parameter` (tunable) or `local` | See [Q3](#open-questions) |
| `AT %M…`, plain `VAR` | `local` | Observable, not part of the interface |

**Value reference = `VarIndex`.** `VarIndex` is a stable `u16` assigned at
compile time and is exactly what `read_variable`/`write_variable` already
consume. Using it as the FMI value reference means the shim needs no translation
table beyond "which value references are inputs vs outputs," which it reads from
the interface metadata.

**Interface metadata source.** The debug section already carries everything
needed ([Why the Architecture Fits §4](#4-variable-metadata-for-modeldescriptionxml--already-persisted)).
However, the debug section is *optional* and strippable
(`debugger-support.md`, Design Goal #5). An FMU must keep working with debug info
stripped. See [Q2](#open-questions): either require the debug section for FMU
export, or introduce a small **non-strippable "interface" section** that pins
the exported variables' `{value reference, name, causality, type, start}` so the
external contract is independent of debug info.

### Component 2: `modelDescription.xml` generation

A build-time generator reads the container's interface metadata and emits
`modelDescription.xml`: one scalar variable per exported `VarIndex`, with
causality, variability, declared type, and start value. Type mapping from
`iec_type_tag`:

| IEC type | FMI 2.0 | FMI 3.0 |
|----------|---------|---------|
| `BOOL` | `Boolean` | `Boolean` |
| `SINT/INT/DINT` | `Integer` (32-bit) | `Int8/Int16/Int32` |
| `LINT` | `Integer` (**lossy** — truncates) | `Int64` |
| `USINT/UINT/UDINT/ULINT` | `Integer` (lossy for large/64-bit) | `UInt8/16/32/64` |
| `REAL` | `Real` | `Float32` |
| `LREAL` | `Real` | `Float64` |
| `STRING/WSTRING` | `String` | `String` (binary/clock types available) |
| enumerations | `Enumeration` | `Enumeration` |
| `TIME`, date/time types | see [Q4](#open-questions) | see [Q4](#open-questions) |

The lossy rows are the main argument for targeting FMI 3.0 (see
[FMI 2.0 vs 3.0](#fmi-20-vs-30)).

### Component 3: The FMI ABI shim crate

A new crate — proposed name **`ironplc-fmi`**, built as a `cdylib` — implements
the `extern "C"` FMI entry points and holds all `unsafe`. It calls only the safe
`ironplc-vm` API.

**Why it must be separate.** The workspace sets `unsafe_code = "deny"`
(`compiler/Cargo.toml`) and development standards forbid
`#[allow(unsafe_code)]` escapes. The FMI C API cannot be implemented without
`unsafe` (raw `extern "C"` functions, pointer arguments, opaque instance
handles). Therefore the shim must be **outside** that lint scope — either a
workspace member that overrides `[lints]` locally, or a separate crate outside
the compiler workspace. This preserves Design Goal #2: only a thin, reviewable
boundary is `unsafe`; the VM stays fully safe and no-std-compatible. This
warrants an ADR (see [Q1](#open-questions)).

Responsibilities of the shim:

- Own an opaque instance struct: the loaded `Container` (from `resources/`), its
  `VmBuffers`, and the current `Vm*` state.
- Translate `fmi*Set*`/`fmi*Get*` value references to `VarIndex` and call the
  typed accessors.
- Own the microsecond time accumulator and drive `run_round` (Component 4).
- Map VM lifecycle and faults to `fmi*Status`.

### Component 4: `DoStep` ↔ scan mapping (time)

FMI time is `double` seconds; VM time is `u64` microseconds. The shim keeps an
**integer microsecond accumulator** to avoid floating-point drift across many
steps:

1. On `SetupExperiment`, record start time; initialize the accumulator.
2. `DoStep(t_c, h)`: advance a target time `t_end = accumulator + round(h ×
   1e6)`. Write pending inputs into the VM, then call `run_round(t)` at each
   scan boundary from the accumulator up to `t_end`, then read outputs.
3. The relationship between `h` and the PLC scan interval is the key semantic
   decision ([Q5](#open-questions)). The cleanest default: **communication step
   = task interval**, so one `DoStep` is one `run_round`. For a variable or
   larger `h`, the shim iterates `run_round` across the interval. Because the VM
   consumes an injected clock, this is deterministic.

### Component 5: Lifecycle mapping

| FMI | IronPLC VM |
|-----|-----------|
| `Instantiate` | `Vm::new().load(container, bufs)` → `VmReady` |
| `EnterInitializationMode` / apply start values | hold `VmReady`; write `parameter`/`input` start values |
| `ExitInitializationMode` | `VmReady::start()` (runs each instance's init function) → `VmRunning` |
| `SetReal/…` (inputs) | typed `write_variable_*` |
| `GetReal/…` (outputs) | `read_variable_raw` / `read_variable_i64` + reinterpret by type |
| `DoStep` | Component 4 |
| trap during a step | `VmRunning::fault(ctx)` → `VmFaulted`; return `fmi*Error`/`Fatal` |
| `Terminate` | `stop()` → `VmStopped` |
| `Reset` | re-initialize to `VmReady` and re-run init — needs a reset path exposed to the shim ([Q6](#open-questions)) |

### Component 6: FMU packaging

A build step (proposed: an `ironplcc` subcommand, e.g. `ironplcc fmu <project>`,
or a `just` recipe) that:

1. Compiles the project to a `.iplc` container.
2. Generates `modelDescription.xml` (Component 2).
3. Builds `ironplc-fmi` as a `cdylib` for the target platform(s).
4. Zips container (into `resources/`), binary (into `binaries/<platform>/`), and
   XML into a `.fmu`. The shim loads the embedded container at `Instantiate`.

## The I/O Decision: Minimal vs Process Image

Two paths, and the FMU's *external* contract is identical for both — so this can
be deferred.

- **Path A — minimal (recommended first).** Map each FMI variable directly to
  its `VarIndex` in the existing flat variable table. Inputs are written with
  `write_variable_*` before the step; outputs are read after. **No VM execution
  changes, no new opcodes, no process image.** Sufficient for real
  co-simulation. Limitation: no hardware-accurate "inputs frozen for the whole
  scan / outputs staged atomically" guarantee — but for a program modeled as one
  cyclic task stepped once per communication point, the shim's write-before /
  read-after ordering already provides that at the step boundary.

- **Path B — process image (later, independent).** Implement the designed `%I` /
  `%Q` / `%M` regions, the `LOAD_INPUT` / `STORE_OUTPUT` opcodes, and wire the
  two `run_round` stubs. The FMU then writes the `%I` snapshot before the step
  and reads the `%Q` staging buffer after — giving crisp, hardware-accurate
  freeze/flush semantics and multi-task correctness. This is a general I/O
  improvement that stands on its own regardless of FMI.

Recommendation: build Path A to land a working FMU; treat Path B as a separate
process-image work item that upgrades semantics without changing the FMU
contract. See [Q7](#open-questions).

## FMI 2.0 vs 3.0

FMI 2.0 for Co-Simulation has the broadest master/tool support but only
`Real` / `Integer` (32-bit) / `Boolean` / `String` / `Enumeration`, so it
narrows `LINT`/`ULINT`/64-bit types (the lossy rows above). FMI 3.0 added
`int8…int64`, `uint8…uint64`, and `float32/64`, mapping IEC types faithfully.
Given the Open Industry Project's co-simulation focus and IEC's 64-bit types,
**FMI 3.0 CS** is the type-faithful target; **FMI 2.0 CS** is the
maximum-compatibility target. This is a primary decision for reviewers
([Q8](#open-questions)).

## Proposed Phased Implementation

Each phase is independently valuable and testable. Detailed plans will follow in
`specs/plans/` once the open questions are resolved.

1. **Typed variable access.** Add typed/raw setters to `VmRunning`
   (`write_variable` currently only accepts `i32`, `compiler/vm/src/vm.rs:603`)
   to mirror the existing read side (BOOL, integer widths, REAL, LREAL). Pure
   addition to the safe VM.
2. **Interface metadata + `modelDescription.xml` generator.** Decide the
   metadata source ([Q2](#open-questions)); emit the XML from a container.
3. **`ironplc-fmi` shim (Path A) + FMU packaging.** The `cdylib`, lifecycle,
   time accumulator, and `.fmu` builder. First end-to-end FMU.
4. **Conformance + a golden vertical slice.** A trivial `%I`-in / `%Q`-out
   program exported as an FMU, stepped by a reference master (e.g. the FMI
   cross-check / OMSimulator), with outputs asserted against a known trajectory.
5. **(Optional, independent) Path B process image.** Implement `%I`/`%Q`/`%M`,
   the input/output opcodes, and wire the `INPUT_FREEZE`/`OUTPUT_FLUSH` stubs.

## Out of Scope (this document)

- **FMI Model Exchange** (as opposed to Co-Simulation). PLC scan semantics fit
  co-simulation; model exchange (exposing continuous state derivatives) does not
  apply.
- **Importing** FMUs *into* an IronPLC program (the reverse direction).
- **FMI clocks / event mode / hybrid co-simulation** (FMI 3.0 advanced
  features) beyond fixed/variable communication steps.
- The full process-image implementation (Path B) — scoped here only as it
  relates to the FMI boundary; its own design belongs with the runtime model.
- Real-time / hardware-in-the-loop execution. Co-simulation here is
  logical-time, master-driven.

## Open Questions

Please answer these inline as PR feedback; they drive the plan.

- **Q1 — Unsafe-boundary crate.** Confirm the approach of quarantining all FMI
  `unsafe` FFI in a separate `ironplc-fmi` crate outside the
  `unsafe_code = "deny"` lint, leaving `ironplc-vm` unchanged and safe. Should
  this be recorded as an ADR? Should the crate be a workspace member with a
  local `[lints]` override, or live outside the compiler workspace entirely?

- **Q2 — Interface metadata source.** For the exported-variable contract, do we
  (a) require the (optional, strippable) debug section to be present for FMU
  export, or (b) add a dedicated non-strippable "interface" section to the
  container that pins `{value reference, name, causality, type, start}`
  independently of debug info? (b) is more robust but adds container surface.

- **Q3 — Causality of globals.** Should non-located `VAR_GLOBAL` map to FMI
  `parameter` (tunable at init), to `local` (observable only), or be
  configurable per variable (e.g. via a pragma/attribute)? Is there appetite for
  a source-level annotation to mark FMI causality explicitly, rather than
  inferring it from section/address?

- **Q4 — TIME / date-time types.** How should `TIME`/`LTIME`/date-time IEC types
  be exposed? As FMI `Real` seconds, as scaled integers, or excluded from the
  interface (kept `local`)?

- **Q5 — Step ↔ scan semantics.** Is "communication step size = task interval,
  one `DoStep` = one scan" the right default? How should the FMU behave when the
  master's `h` is not a multiple of the task interval (sub-step and run partial,
  reject, or snap)? Should the task interval be fixed by the program or
  overridable by the master at instantiation?

- **Q6 — Reset semantics.** `fmi*Reset` returns to the instantiated state. Is
  re-running the init functions (as on a FAULTED restart) the intended behavior,
  and should the VM expose an explicit `VmRunning`/`VmFaulted → VmReady` reset,
  or should the shim rebuild the instance from the container each time?

- **Q7 — I/O path ordering.** Agree to ship Path A (direct variable mapping,
  no process image) first, with Path B (process image) as a later, independent
  improvement? Or is hardware-accurate freeze/flush a hard requirement for the
  first FMU?

- **Q8 — FMI version.** Target **FMI 3.0 CS** (faithful 64-bit type mapping),
  **FMI 2.0 CS** (widest tool compatibility, lossy on 64-bit), or both? Which
  master(s) must the first release interoperate with (this pins the required
  version and the conformance target in Phase 4)?

- **Q9 — Multi-task programs.** For a program with multiple tasks at different
  intervals, is the FMU a single unit stepping all tasks per its own internal
  schedule within a `DoStep`, or is multi-task FMI export out of scope for v1
  (single-task/single-program only, matching the runtime model's current
  single-instance focus)?

- **Q10 — Packaging entry point.** Preferred build surface for producing a
  `.fmu`: a new `ironplcc fmu …` subcommand, a dedicated tool/crate, or a `just`
  recipe? And which target platforms must the first release build binaries for
  (Linux `x86_64` only, or also Windows / macOS)?
