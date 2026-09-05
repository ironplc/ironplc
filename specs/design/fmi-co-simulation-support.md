# Design: Functional Mock-up Interface (FMI) Co-Simulation Support

## Status

**Draft — for review (revision 2).** This revision incorporates first-round
review feedback. Points that are now settled are recorded under
[Decisions](#decisions-settled-in-review); the genuinely unresolved items are in
[Open Questions](#open-questions). The design will be finalized from the
remaining answers before an implementation plan is written under `specs/plans/`.

## Overview

This document proposes exporting an IronPLC program as a **Functional Mock-up
Unit (FMU)** implementing **FMI 3.0 for Co-Simulation**, so that IronPLC can
participate in the **Open Industry Project (OIP)** and any FMI 3.0-compatible
simulation master (e.g. OMSimulator, FMPy, and the co-simulation environments
used for virtual commissioning).

The central finding is that IronPLC's VM is unusually well-suited to FMI
co-simulation. FMI export is normally hard for a PLC runtime for two reasons:
the runtime owns its own clock (a scan thread you must fight), and it drives
physical I/O directly (nothing to redirect into a simulation). IronPLC's VM was
designed with **both** as *external* boundaries — the clock is a parameter to
the scan function (`run_round(current_time_us)`), and the I/O boundary was
deliberately left as a pluggable "I/O driver" the VM only touches at two
well-defined points. The recent task/scheduler and time-control work supplies
the co-simulation time contract almost directly.

The FMU's interface is the program's **I/O**: located input variables (`%I`)
become FMU inputs and located output variables (`%Q`) become FMU outputs. This
is the correct interaction pattern for co-simulation — the simulation drives the
PLC's sensors and reads its actuators, exactly as physical hardware would.
Delivering this cleanly depends on one piece of IronPLC that is specified but
not yet built: the **process image** (the `%I`/`%Q`/`%M` regions). This design
commits to building it.

## For the Open Industry Project Maintainer

This section is written so the OIP maintainer can evaluate fit **without** having
to read the IronPLC internals below. If you maintain a co-simulation master or an
OIP scenario, this is what IronPLC would hand you and what it would expect back.

**What IronPLC would ship.** A standard **FMI 3.0 Co-Simulation FMU** — a single
`.fmu` file (a zip archive; see [What a `.fmu` is](#what-a-fmu-is)). It contains
a `modelDescription.xml`, a platform shared library implementing the FMI 3.0 C
API, and the compiled PLC program as an embedded resource. No IronPLC-specific
software is needed on the master side; any conformant FMI 3.0 CS importer loads
it.

**What the interface looks like.** The FMU's variables **are the PLC's I/O
points**:

| PLC declaration | FMU variable | Causality |
|-----------------|--------------|-----------|
| `sensor AT %IW0 : INT;` | `sensor` | `input` (you write it before a step) |
| `valve AT %QX0.0 : BOOL;` | `valve` | `output` (you read it after a step) |
| tunable setpoints (see [Open Questions](#open-questions)) | as declared | `parameter` |

Internal **memory** state (`%M`) is exposed as **read-only** FMI `local`
variables: the master can *read* them (via `fmi3Get*`) for observability/logging,
but cannot *write* them — FMI only allows the master to `Set` `input`s and
`parameter`s, never `local`s. So the interaction pattern stays I/O in, I/O out;
`%M` is visible but not drivable. **Other internal variables (plain, non-located
locals) are not exposed in v1** — `%M` is the only internal state on the
interface. A follow-up version could expose selected locals the same read-only
way if a use case appears.

**How time works.** A PLC executes on a fixed **scan cycle** (e.g. every 10 ms).
The FMU advertises that period to the master via FMI 3.0's
`fixedInternalStepSize`. The recommended usage is **one communication step = one
scan**: call `fmi3DoStep` with a step equal to the scan period. Set the inputs,
`DoStep`, read the outputs. The FMU is deterministic — same inputs and same step
sequence produce identical outputs — because internally the VM runs on a
simulated clock, never wall-clock time. What happens when a master requests a
larger, smaller, or misaligned step is covered in
[Time and Stepping](#time-and-stepping); the short version is that the FMU tells
you its required period and will report the actual time it reached.

**Type mapping.** IEC 61131-3 types map onto FMI 3.0's typed variables with no
loss (this is the main reason for targeting 3.0 rather than 2.0). `BOOL→Boolean`,
`SINT/INT/DINT/LINT→Int8/16/32/64`, the unsigned variants to `UInt*`,
`REAL→Float32`, `LREAL→Float64`, `STRING→String`, and `TIME`/date-time values as
**integer** counts (see [Type Mapping](#type-mapping)).

**Platform constraint.** IronPLC ships the runtime (`ironplc-fmi`) **only for the
target hardware** — the platform the PLC program is built for. The FMU's shared
library therefore matches that one platform, so **the co-simulation must run on
that same platform**. If your OIP scenarios run the master on a different OS/CPU
than the PLC's target hardware, the FMU binary won't match and we'd need a
cross-compilation/installer story. We're treating this as an **open question and
want your input** ([Q6](#open-questions)): tell us
what platform(s) your co-simulation host runs on.

**What we would want from you.** (1) Which FMI 3.0 master(s) must the first
release interoperate with, so we target the right conformance suite
([Q1](#open-questions))? (2) v1 exposes I/O plus read-only `%M`; do any scenarios
need tunable `parameter`s (setpoints/limits) set at initialization, or is that
out of scope for now ([Q4](#open-questions))? (3) Do you
ever step FMUs at a rate other than their native period, and does your master
honor `fixedInternalStepSize`/early-return ([Q2](#open-questions))? (4) What
platform does your co-simulation host run on — is it the same as the PLC's target
hardware ([Q6](#open-questions))?

**Automated testing.** FMU export is covered by an **automated CI integration
test**: it compiles a PLC program to an FMU, loads it in a reference FMI 3.0
importer (e.g. FMPy or the rust-fmi importer), runs a multi-step scenario, and
asserts inputs→outputs — so exports can't silently regress
(see [Implementation Phases](#implementation-phases), phase 6).

**Known v1 limitations.** Single program / single scan task (multi-task programs
deferred, [Q3](#open-questions)); FMI **Co-Simulation** only (not Model
Exchange); no importing of external FMUs into a PLC program; the FMU binary is
built for the target hardware, so the co-simulation runs on that platform
([Q6](#open-questions)); calendar date/time types await a storage ADR before they
can appear in an FMU interface ([Q5](#open-questions)).

## Background

### What a `.fmu` is

A `.fmu` is **not** an executable you run directly. It is a **zip archive** with
a defined layout that a *simulation master* (importer) opens and calls into:

```
example.fmu  (a zip file)
├── modelDescription.xml          — the machine-readable interface contract
├── binaries/
│   └── x86_64-linux/example.so   — shared library exporting the FMI 3.0 C API
└── resources/
    └── program.iplc              — (IronPLC-specific) the compiled PLC program
```

The master reads `modelDescription.xml` to learn the variables (names, types,
causality, value references, start values), loads the shared library, and then
drives it through the FMI lifecycle: instantiate → initialize → repeatedly
`Set` inputs / `DoStep` / `Get` outputs → terminate. The FMU never owns the
main loop and never sleeps; the master owns time. This is why the model fits a
clock-injected VM so well.

### The FMI 3.0 Co-Simulation lifecycle

| FMI 3.0 call | Meaning |
|--------------|---------|
| `fmi3InstantiateCoSimulation` / `fmi3FreeInstance` | Create / destroy an instance |
| `fmi3EnterInitializationMode` / `fmi3ExitInitializationMode` | Configure start/stop time; run initialization |
| `fmi3SetBoolean/Int*/UInt*/Float*/String` | Write inputs and parameters by value reference |
| `fmi3GetBoolean/Int*/UInt*/Float*/String` | Read outputs (and any exposed locals) by value reference |
| `fmi3DoStep(t_c, h, …)` → `lastSuccessfulTime`, `eventEncountered`, `earlyReturn` | Advance from communication point `t_c` by step size `h` |
| `fmi3Terminate`, `fmi3Reset` | End the run / return to instantiated state |

### Building On

- **[Runtime Execution Model](runtime-execution-model.md)** — VM lifecycle, the
  `INPUT_FREEZE → EXECUTE → OUTPUT_FLUSH → IDLE` scan cycle, the process image,
  and the `simulated` clock source. FMI export realizes the "I/O driver" that
  spec leaves out of scope, and this design implements the process image it
  defines.
- **[IEC 61131-3 Task Support](61131-task-support.md)** — the task table,
  cyclic/freewheeling scheduling, and the externally-driven `run_round` model.
- **[Bytecode Container Format](bytecode-container-format.md)** — the compiled
  container. FMI export needs **no format change now**: the model description is
  generated at compile time (see
  [Interface Metadata](#interface-metadata-generated-at-compile-time)).
- **[No-std VM](no-std-vm.md)** / **ADR-0010** — the commitment that the VM core
  stays `unsafe`-free and embeddable, which dictates where the FMI C ABI lives.

## User Experience

End to end, from writing a program to running it in a co-simulation.

### 1. Author the program with located I/O

The engineer writes ordinary Structured Text and declares the points that should
be visible to the simulation as **located variables**. There is nothing
FMI-specific in the source — the same program runs on hardware.

```iecst
PROGRAM main
  VAR
    tank_level  AT %IW0  : INT;   (* sensor  → becomes FMU input  *)
    pump_on     AT %QX0.0 : BOOL;  (* actuator → becomes FMU output *)
    high_mark   AT %MW10 : INT;   (* internal → read-only FMU local *)
  END_VAR
  pump_on := tank_level < high_mark;
END_PROGRAM
```

The `%I`/`%Q`/`%M` declarations are the entire contract: `%I`→input, `%Q`→output,
`%M`→read-only local. No annotations, no separate interface file.

### 2. Build the FMU

One command, an output format of the existing compiler:

```console
$ ironplcc compile --format fmu ./my-project
  → my-project.fmu
```

The compiler produces the `.iplc` container, generates `modelDescription.xml` at
compile time from the located-variable (IOM) info, and zips both together with the
**prebuilt** `ironplc-fmi` runtime for the target platform. The result is a
single self-contained `my-project.fmu` (see [Packaging](#packaging-ironplcc-compile---format-fmu)).
Because the runtime binary is built for the PLC's target hardware, the FMU runs
on that platform ([Q6](#open-questions)).

### 3. Use it in a co-simulation master (generic FMI 3.0)

`my-project.fmu` is a standard FMI 3.0 CS FMU. Any conformant master loads it —
no IronPLC tooling required. For example, with FMPy in Python:

```python
from fmpy import simulate_fmu
# Drive tank_level from a plant model / signal, read pump_on back.
result = simulate_fmu('my-project.fmu',
                      start_time=0.0, stop_time=10.0,
                      output=['pump_on'])
```

In a graphical master (OMSimulator, OSP/libcosim), the FMU appears as a block
with `tank_level` as an input pin and `pump_on` as an output pin; the user wires
those to the rest of the scenario and presses run. The master calls
`Set`(inputs) → `DoStep`(scan period) → `Get`(outputs) each cycle.

### 4. Integrate with the Open Industry Project

The closed loop is the point: the PLC FMU controls a **plant/process model** FMU.
The user connects the PLC's `%I` inputs to the plant model's outputs (sensors)
and the PLC's `%Q` outputs to the plant model's inputs (actuators), forming a
control loop the OIP scenario runs forward in time:

```
        ┌────────────────┐   pump_on (%Q) ──▶ actuator   ┌─────────────────┐
        │  IronPLC FMU   │                                │  Plant/Process  │
        │  (controller)  │   tank_level (%I) ◀── sensor   │   model (FMU)   │
        └────────────────┘                                └─────────────────┘
                     ▲                                             ▲
                     └───────────── OIP master steps both ─────────┘
```

Because the FMU is standards-conformant, integrating with OIP is "drop it into
the scenario as a component and wire the pins." The **exact** OIP integration
surface — the scenario/config format (e.g. an OSP-style `SystemStructure` file),
how components are registered, and any OIP-specific connection metadata — depends
on OIP's tooling and is the one piece this design cannot specify from the IronPLC
side. That is [Q7](#open-questions), for the OIP maintainer.

## Design Goals

1. **A standards-conformant FMI 3.0 CS FMU** — a compiled IronPLC program
   packages into a valid `.fmu` that any conformant FMI 3.0 master can
   instantiate, initialize, step, and read, with no IronPLC-specific tooling on
   the master side.
2. **The VM core stays `unsafe`-free** — the C ABI (`extern "C"`, raw pointers)
   is inherently `unsafe`; it is quarantined in a boundary crate outside the
   workspace `unsafe_code = "deny"` lint. `ironplc-vm` is not modified to add
   `unsafe`.
3. **Deterministic co-simulation** — same input trajectory and same sequence of
   communication points ⇒ identical outputs (follows from the `simulated`
   clock).
4. **I/O is the interface** — FMU variables are the program's located `%I`/`%Q`
   points, driven through the process image, not arbitrary internal variables.
5. **Reuse, don't fork, the execution core** — FMI is a new *driver* around the
   existing `VmRunning` state machine, like the DAP debugger. No changes to
   bytecode or codegen semantics.

## Decisions (settled in review)

These were open questions in revision 1; review resolved them.

- **FMI 3.0 only.** FMI 2.0 is not a supported target (it would narrow 64-bit
  IEC types). The type table and API names below are 3.0.
- **The interface is I/O; `%M` is read-only-visible.** Located `%I` → FMU
  `input`, `%Q` → FMU `output`. `%M` memory is exposed as read-only FMI `local`
  variables — the master can read them but cannot write them (FMI only lets the
  master `Set` `input`s/`parameter`s). The master never drives internal state.
- **No other internal variables in v1.** Plain, non-located locals are not
  exposed on the FMU interface; `%M` is the only internal state surfaced. A later
  version may add opt-in read-only exposure of selected locals.
- **Build the process image.** IronPLC implements the `%I`/`%Q`/`%M` regions and
  wires the `INPUT_FREEZE`/`OUTPUT_FLUSH` phases, rather than mapping I/O straight
  to flat variable slots. This is what makes the I/O semantics real.
- **`modelDescription.xml` is generated at compile time — no format change now.**
  The compiler emits the model description directly from the located-variable
  (IOM) information it already has, so the FMI path needs neither the strippable
  debug section nor a new FMI-specific container section. The runtime's IOM layout
  is supplied by the process-image work already planned.
- **Packaging is `ironplcc compile` with an output-format flag**, not a `just`
  recipe and not a new subcommand.
- **The `.fmu` build does not compile `ironplc-fmi`.** That shared library is a
  prebuilt artifact shipped with IronPLC; `compile` produces the container,
  generates `modelDescription.xml`, and zips them together with the
  already-built binary.
- **`fmi3Reset` rebuilds the instance** in the shim (drop and reconstruct from
  the container) to guarantee state is cleanly wiped.
- **Time/date IEC types map to integers**, not `Float`.
- **TIME → integer.** `TIME`/`LTIME` and date-time types are exported as integer
  counts (see [Type Mapping](#type-mapping)).
- **Same-host binary for v1.** The FMU runs on the platform it was compiled for.

## Why the Architecture Fits

The four things co-simulation needs already exist in the VM.

1. **The master owns the clock — already true.**
   `VmRunning::run_round(current_time_us)` (`compiler/vm/src/vm.rs:339`) takes
   time as an argument; the VM never sleeps (the sleep loop is in the CLI,
   `compiler/vm-cli/src/cli.rs:86-92`). The scheduler is "time-agnostic: callers
   pass the current time" (`compiler/vm/src/scheduler.rs:30-33`), and the runtime
   model already specifies a `simulated` clock for deterministic replay. This is
   `fmi3DoStep`'s contract directly.
2. **Set/get variables by index — present.** `VmRunning` exposes
   `read_variable`, `read_variable_raw`, `read_variable_i64`, and
   `write_variable` (`compiler/vm/src/vm.rs:585-606`), the same external-access
   path the DAP debugger uses. (Typed setters need widening — see
   [phase 1](#implementation-phases).)
3. **Per-instance buffers — clean multi-instantiation.**
   `VmBuffers::from_container` allocates per-instance state over a read-only
   shared `Container`; multiple FMU instances are independent.
4. **Interface metadata already exists at compile time.** The compiler knows each
   located variable's `var_section`, `iec_type_tag`, name, and type (the debug
   section records the same, `compiler/container/src/debug_section.rs:11-30`) — so
   the model description is emitted at compile time without relying on the
   strippable debug section
   ([below](#interface-metadata-generated-at-compile-time)).

## The Interface: Mapping PLC I/O to FMI Variables

The FMU exposes the program's **directly-represented (located) variables** as its
FMI variables:

| IEC source | FMI 3.0 causality | Direction at a step boundary |
|------------|-------------------|------------------------------|
| `AT %I…` (input) | `input` | Master writes before `DoStep` |
| `AT %Q…` (output) | `output` | Master reads after `DoStep` |
| tunable parameter (marked; [Q4](#open-questions)) | `parameter` | Master writes at init |
| `AT %M…` | `local` | read-only: master `Get`s, cannot `Set` |
| plain `VAR` (non-located) | not exported | deferred to a follow-up version |

The input/output causality is not inferred heuristically — it comes
straight from the `%I`/`%Q` region the variable is located in. Before a step the
shim writes FMU inputs into the `%I` image; after the step it reads FMU outputs
from the `%Q` image.

**Value references.** Each exported I/O variable gets a stable FMI
`valueReference` (an opaque `uint32`) assigned by the compiler and written into
`modelDescription.xml` (see [next section](#interface-metadata-generated-at-compile-time)).
The shim maps `valueReference → (region, offset, width)` in the process image.
Because references are derived from the located variables, they are stable across
recompiles that don't
change the I/O map.

### Type Mapping

FMI 3.0 has typed variables that cover the IEC elementary types with no
narrowing (the reason FMI 2.0 is rejected):

| IEC type | FMI 3.0 type |
|----------|--------------|
| `BOOL` | `Boolean` |
| `SINT` / `INT` / `DINT` / `LINT` | `Int8` / `Int16` / `Int32` / `Int64` |
| `USINT` / `UINT` / `UDINT` / `ULINT` | `UInt8` / `UInt16` / `UInt32` / `UInt64` |
| `BYTE` / `WORD` / `DWORD` / `LWORD` | `UInt8` / `UInt16` / `UInt32` / `UInt64` |
| `REAL` / `LREAL` | `Float32` / `Float64` |
| `STRING` / `WSTRING` | `String` |
| enumerations | `Enumeration` (or the underlying `Int*`) |
| date/time types | integer — see the [dedicated table](#dateduration-types) below |

#### Date/duration types

Every IEC date and duration type is exported as an **integer** (never `Float`),
so no precision is lost and the master reads an exact count. Each variable's
`modelDescription.xml` entry carries a `unit`/annotation naming the tick and base
so the master can interpret the integer. The eight IEC date/time type tags
(`compiler/container/src/debug_section.rs:51-58`) map as follows:

| IEC type | Meaning | FMI 3.0 type | Encoding (unit · base) | Status |
|----------|---------|--------------|------------------------|--------|
| `TIME` | duration | `Int32` | signed **milliseconds** | **Settled** — ADR-0021 (32-bit, ms) |
| `LTIME` | duration | `Int64` | signed **milliseconds** | **Settled** — ADR-0021 (64-bit, ms) |
| `DATE` | calendar date | `Int32` | **days** since 1970-01-01 | Proposed — needs storage ADR |
| `LDATE` | calendar date | `Int64` | **nanoseconds** since 1970-01-01 00:00:00 | Proposed — needs storage ADR |
| `TIME_OF_DAY` (`TOD`) | time within a day | `Int32` | **milliseconds** since 00:00:00 | Proposed — needs storage ADR |
| `LTOD` | time within a day | `Int64` | **nanoseconds** since 00:00:00 | Proposed — needs storage ADR |
| `DATE_AND_TIME` (`DT`) | timestamp | `Int64` | **milliseconds** since 1970-01-01 00:00:00 | Proposed — needs storage ADR |
| `LDT` | timestamp | `Int64` | **nanoseconds** since 1970-01-01 00:00:00 | Proposed — needs storage ADR |

`TIME`/`LTIME` are firm: ADR-0021 fixes their width and millisecond unit, and the
VM already stores and formats them that way. The six calendar types
(`DATE`/`LDATE`/`TOD`/`LTOD`/`DT`/`LDT`) are currently **recognized as type tags
but their in-memory representation is not yet finalized** — debug formatting
implements only `TIME`/`LTIME` today. The encodings above are a concrete proposal
that FMI export needs, but they must be pinned by a storage ADR before export can
emit them; that ADR is a prerequisite tracked as [Q5](#open-questions). Until it
lands, an FMU whose interface uses a calendar type is rejected at
`compile --format fmu` with a clear "type not yet supported for FMI export"
error rather than emitting an unspecified encoding.

The exact unit/epoch for the time and date types is recorded in the
`modelDescription.xml` variable's unit annotation so the master interprets the
integer correctly.

## Interface Metadata (generated at compile time)

`modelDescription.xml` is generated at **compile time**, while the compiler still
has the full located-variable (IOM) information from the source. This means **no
container-format change is needed now** to support FMI export — the model
description is a compile output, produced directly from what the compiler already
computes, not reconstructed later from a new container section.

1. **The compiler emits the model description directly.** During
   `ironplcc compile --format fmu`, the compiler already knows every located
   variable's `{name, region (I/Q/M), size, address, iec_type, start value}` and
   assigns each a stable `valueReference`. It writes these straight into
   `modelDescription.xml`. Nothing has to be read back out of the compiled
   container, so the (optional, strippable) debug section is never on the FMI
   path.
2. **The runtime's IOM layout comes from the process-image work, not a new FMI
   section.** The VM still needs the `%I`/`%Q`/`%M` layout at run time, but that
   is exactly what the [process image](#process-image) already builds — the task
   table's `input_image_offset`/`output_image_offset` (today hardcoded to `0`,
   `compiler/container/src/builder.rs:342-343`) plus the located-variable
   placement it requires. Those **IOM points** are the durable, non-strippable
   truth in the container; FMI export does not add anything FMI-specific to the
   format.

Net: FMI export rides on information the compiler already has, emitted at compile
time, plus the process-image work already planned. We are **not** changing the
container format for FMI now.

## Time and Stepping

A PLC runs a fixed scan cycle; a co-simulation master picks communication points.
Reconciling them is the crux, and FMI 3.0 gives us the mechanisms.

- **The FMU advertises its scan period.** `modelDescription.xml`'s
  `<CoSimulation>` sets **`fixedInternalStepSize`** = the task interval. This is
  how the PLC *informs the master of its required step size* — the answer to
  "what if the simulation skips a cycle." A master that honors it steps the FMU
  at its native period.
- **Recommended usage: one `DoStep` = one scan.** With
  `canHandleVariableCommunicationStepSize = false`, the master must use the fixed
  period; the shim runs exactly one `run_round` per `DoStep`.
- **If we allow variable steps** (`canHandleVariableCommunicationStepSize =
  true`): the shim sub-steps internally, running `run_round` at each scan
  boundary that falls within `[t_c, t_c + h)`, using an **integer-microsecond
  accumulator** (FMI time is `double` seconds, VM time is `u64` µs) to avoid
  drift.
- **Misaligned or too-small steps.** `fmi3DoStep` returns `lastSuccessfulTime`.
  If `h` does not land on a scan boundary, the FMU advances to the last whole
  scan it could complete and reports that time via `lastSuccessfulTime` (an
  early-ish return), rather than executing a partial scan — a partial scan is
  meaningless for a PLC. A master that ignores `fixedInternalStepSize` still gets
  correct, self-describing behavior.

The precise policy (fixed-only vs variable-with-substepping, and whether the task
period is fixed by the program or overridable at instantiation) is
[Q2](#open-questions). Determinism holds in all cases because the VM consumes the
injected clock.

## The FMI Shim and Crate Reuse

All `unsafe` FFI lives in one boundary crate (proposed name **`ironplc-fmi`**),
built as a `cdylib`, calling only the safe `ironplc-vm` API. It must sit **outside**
the workspace `unsafe_code = "deny"` lint (a workspace member with a local
`[lints]` override, or a crate outside the compiler workspace). This keeps the VM
100% safe and no-std-compatible while confining `unsafe` to a thin, auditable
layer.

Shim responsibilities: own the opaque instance (loaded `Container`, `VmBuffers`,
current `Vm*` state); translate `valueReference → (region, offset)` and copy to/
from the process image; own the µs time accumulator and drive `run_round`; map VM
faults (`VmFaulted`) to `fmi3Error`/`fmi3Fatal`; rebuild the instance on
`fmi3Reset`.

**Reuse (answering the review question about an existing crate).** The **rust-fmi**
project provides **`fmi-export`** (a crate with an `FmuModel` derive macro and an
`export_fmu!` macro) plus the **`cargo-fmi`** subcommand. Together they generate
the FMI 3.0 C API implementation, type-safe variable handling, and the
`modelDescription.xml` from a Rust struct — i.e. exactly the boilerplate we would
otherwise hand-write, and it hides the `extern "C"` FFI from the user. This is a
strong reuse candidate: `ironplc-fmi` could define its FMU model in terms of the
IronPLC located-variable (IOM) info and let `fmi-export` emit the ABI, rather than
hand-rolling `fmi3*` exports. **Caveat:** rust-fmi's export path is relatively
new; maturity, license, and how cleanly its model abstraction accepts a
*container-driven* (rather than compile-time-struct) variable set need a short
spike before we commit. If it doesn't fit, the fallback
is a hand-written shim — larger, but well-understood.

## Process Image

This is the substrate the interface rides on. Per the runtime model it is
specified but unimplemented today: no `LOAD_INPUT`/`STORE_OUTPUT` opcodes exist,
the `INPUT_FREEZE`/`OUTPUT_FLUSH` phases are `// Stub` no-ops
(`compiler/vm/src/vm.rs:357,410`), and the image offsets are hardcoded to `0`.
The work:

1. Allocates the `%I`/`%Q`/`%M` regions and populates `input_image_offset` /
   `output_image_offset` from the located-variable (IOM) layout.
2. Adds `LOAD_INPUT`/`STORE_OUTPUT`/`LOAD_MEMORY`/`STORE_MEMORY` opcodes (or
   routes located access through the images) and compiles located variables to
   them instead of flat slots.
3. Wires `INPUT_FREEZE` (snapshot inputs before EXECUTE) and `OUTPUT_FLUSH`
   (stage outputs after EXECUTE) in `run_round`.

The FMU then writes the `%I` image before `DoStep` and reads the `%Q` image
after — giving hardware-accurate "inputs frozen for the whole scan, outputs
staged atomically" semantics. The process image is independently valuable to
IronPLC (it is real I/O), which is why it is core here rather than an optional
add-on.

## Lifecycle Mapping

| FMI 3.0 | IronPLC VM |
|---------|-----------|
| `fmi3InstantiateCoSimulation` | load container + `VmBuffers` → `VmReady` |
| `fmi3EnterInitializationMode` + set start values | hold `VmReady`; write parameter/input starts into images |
| `fmi3ExitInitializationMode` | `VmReady::start()` (runs init functions) → `VmRunning` |
| `fmi3Set*` (inputs) | write into `%I` image |
| `fmi3Get*` (outputs) | read from `%Q` image |
| `fmi3DoStep` | [Time and Stepping](#time-and-stepping) |
| trap during a step | `VmRunning::fault` → `VmFaulted`; return `fmi3Error`/`Fatal` |
| `fmi3Terminate` | `stop()` → `VmStopped` |
| `fmi3Reset` | **drop and rebuild** the instance from the container |

## Packaging: `ironplcc compile --format fmu`

Producing a `.fmu` is an **output format of `ironplcc compile`**, not a separate
tool. `compile` already exists and emits the `.iplc` container; an
output-format flag adds the FMU packaging path:

1. Compile the project to the `.iplc` container (existing behavior).
2. Generate `modelDescription.xml` at compile time from the located-variable
   (IOM) information (no container-format change; see
   [Interface Metadata](#interface-metadata-generated-at-compile-time)).
3. Zip together: `modelDescription.xml`, the `.iplc` container under
   `resources/`, and the **prebuilt** `ironplc-fmi` shared library for the host
   platform under `binaries/<platform>/`.

The compile step **does not build `ironplc-fmi`** — that library is a versioned
artifact shipped with the IronPLC distribution and simply copied into the
archive. Because only the host platform's binary is included, the resulting FMU
runs on the same OS/CPU it was compiled on (the v1 same-host constraint;
cross-platform is [Q6](#open-questions)).

## Implementation Phases

Each phase is independently testable. Detailed plans follow in `specs/plans/`
once the open questions are resolved.

1. **Typed variable access.** Widen `VmRunning` setters (today `write_variable`
   only accepts `i32`, `compiler/vm/src/vm.rs:603`) to mirror the read side —
   BOOL, all integer widths, `Float32`, `Float64`. Pure addition to the safe VM.
2. **Process image.** Regions, located-access opcodes, and the
   `INPUT_FREEZE`/`OUTPUT_FLUSH` wiring. Standalone I/O value for IronPLC.
3. **Interface section + `modelDescription.xml` generation.** The non-strippable
   located-variable metadata and the debug info's second emit format.
4. **`ironplc-fmi` shim** (evaluate `fmi-export`/`cargo-fmi` first) and the
   `ironplc compile --format fmu` packaging path.
5. **Golden vertical slice.** A trivial `%I`-in / `%Q`-out program exported as an
   FMU and stepped by a reference master, outputs asserted against a known
   trajectory.
6. **Integration test in CI.** An automated end-to-end test that compiles a PLC
   program to an FMU, loads it with an FMI 3.0 importer (e.g. FMPy or the rust-fmi
   importer), runs a multi-step scenario, and checks inputs→outputs — run as part
   of the normal CI pipeline so FMU export cannot silently regress.

## Out of Scope

- **FMI Model Exchange** (PLC scan semantics fit co-simulation; ME exposes
  continuous-state derivatives, which do not apply).
- **Importing** FMUs *into* an IronPLC program (the reverse direction).
- **Cross-platform FMUs** (multiple binaries in one `.fmu`) — v1 is same-host.
- **FMI clocks / event mode / hybrid co-simulation** beyond fixed/variable
  communication steps.
- **Multi-task programs** as a single FMU — v1 targets a single program / single
  scan task ([Q3](#open-questions)).
- Real-time / hardware-in-the-loop execution — co-simulation here is
  logical-time, master-driven.

## Open Questions

Please answer inline as PR feedback.

- **Q1 — Conformance target.** Which FMI 3.0 master(s) must the first release
  interoperate with (OMSimulator, FMPy, a specific OIP tool)? This pins the
  conformance/integration-test target in phases 5–6.

- **Q2 — Step-size policy.** Advertise `canHandleVariableCommunicationStepSize =
  false` (master must step at the scan period — simplest, most PLC-faithful), or
  `true` with internal sub-stepping? And is the scan/task period fixed by the
  program, or overridable by the master at instantiation?

- **Q3 — Multi-task programs.** Confirm v1 is single-program/single-scan-task.
  For a future multi-task program, is the FMU one unit that internally steps all
  tasks within a `DoStep`, or is that a later design?

- **Q4 — Parameters.** Should any non-I/O values be exportable as tunable FMI
  `parameter`s (set at initialization) — e.g. setpoints/limits — and if so, how
  are they marked in source (a pragma/attribute), given that `%M`/globals are not
  the interface? Or are parameters out of scope for v1 (I/O only)?

- **Q5 — Calendar date/time storage ADR.** `TIME`/`LTIME` are pinned by ADR-0021
  (`Int32`/`Int64` milliseconds). The six calendar types
  (`DATE`/`LDATE`/`TOD`/`LTOD`/`DT`/`LDT`) are recognized as type tags but have no
  finalized in-memory representation. The [date/duration table](#dateduration-types)
  proposes concrete integer encodings (unit + base); do they look right, and can
  we land a storage ADR to make them real? Until then, FMI export rejects
  interfaces that use a calendar type.

- **Q6 — Target-hardware runtime vs. co-simulation host.** IronPLC ships
  `ironplc-fmi` built for the PLC's **target hardware**, so the FMU binary matches
  that platform and the co-simulation must run there. What platform(s) does your
  OIP co-simulation host actually run on? If it differs from the target hardware,
  we need a cross-build/installer approach.

- **Q7 — OIP integration surface.** The FMU is a standard FMI 3.0 CS component,
  so it should drop into an OIP scenario like any other FMU. What is OIP's actual
  integration format — the scenario/config file (an OSP-style `SystemStructure`
  XML? something else?), how components are registered, and any OIP-specific
  connection or metadata requirements? This is the one part of the
  [User Experience](#user-experience) we can't specify from the IronPLC side and
  would document once you confirm it.
