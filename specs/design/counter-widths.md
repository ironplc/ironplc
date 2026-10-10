# Design: Counter Widths

status: implemented
date: 2026-10-09

## Overview

The standard counters `CTU`, `CTD` and `CTUD` (IEC 61131-3 Section 2.5.2.3.3) have a variant for each integer type of their `PV` and `CV`: `CTU_INT`, `CTU_DINT`, `CTU_UDINT`, `CTU_LINT` and `CTU_ULINT`, and the same for `CTD` and `CTUD`. Each variant counts in its own type. The compiler used to send every variant of a counter to one VM intrinsic that read and wrote `PV` and `CV` as signed 32-bit integers, so a `CTD_LINT` loaded with 5,000,000,000 held 705,032,704 and a `CTU_UDINT` read a preset of 3,000,000,000 as negative ([#2054](https://github.com/ironplc/ironplc/issues/2054)).

## Design

**VM.** Each width a counter counts in has its own `FB_CALL` type id (see `bytecode-instruction-set.md`):

| Counts in | `CTU` | `CTD` | `CTUD` |
|---|---|---|---|
| signed 32-bit (`CTU`, `_INT`, `_DINT`) | `0x0020` | `0x0021` | `0x0022` |
| unsigned 32-bit (`_UDINT`) | `0x0023` | `0x0024` | `0x0025` |
| signed 64-bit (`_LINT`) | `0x0026` | `0x0027` | `0x0028` |
| unsigned 64-bit (`_ULINT`) | `0x0029` | `0x002A` | `0x002B` |

One implementation of each counter is generic over the integer it counts in, so the widths cannot diverge. `PV` and `CV` are read and written at that width, and counting saturates at the type's bounds, as the standard counts only while `CV` is below the type's maximum or above its minimum. The `BOOL` fields are unchanged. `_INT` keeps counting in 32 bits, as before.

**Codegen.** A standard counter instance is dispatched to the type id of its width. Each field of a standard function block instance is stored and read at the operation type of its declared type, as a user-defined block's is, rather than at the default slot type: a `CTU_LINT`'s `PV` is stored as the `LINT` the analyzer converted its input to (REQ-IC-analyzer-046).

**REQ-CW-vm-001** The width a counter counts in follows from its type id: `CTU`, `CTD` and `CTUD` count in a signed 32-bit integer, and the `_UDINT`, `_LINT` and `_ULINT` ids in an unsigned 32-bit, a signed 64-bit and an unsigned 64-bit integer.

**REQ-CW-vm-002** A counter reads `PV` and writes `CV` at its own width: a `CTU_LINT` at the largest `DINT` counts past it, a `CTU_UDINT` with a preset of 3,000,000,000 and a `CV` of 0 sets `Q` to `FALSE`, and a `CTUD_ULINT` loaded with the largest `ULINT` holds it.

**REQ-CW-vm-003** Counting saturates at the bounds of the counter's type: a `CTU_UDINT` at the largest `UDINT` stays there, and a `CTD_ULINT` at 0 stays at 0 with `Q` `TRUE`.

**REQ-CW-codegen-001** A counter of each width keeps the values of its type end to end: a `CTD_LINT` and a `CTUD_LINT` loaded with 5,000,000,000 hold it in `CV`, read as a field and as an output; a `CTU_UDINT` with a preset of 3,000,000,000 keeps `Q` `FALSE`; a `CTD_UDINT` counting down from 0 stays at 0; and a `CTD_ULINT` loaded with 10,000,000,000,000,000,000 holds it.

## Out of scope

- A `CTU_INT` counts and saturates in 32 bits, not at the 16-bit bounds of `INT`.
