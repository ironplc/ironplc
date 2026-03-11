# ADR-0021: TIME as 32-bit and LTIME as 64-bit with Millisecond Precision

status: proposed
date: 2026-03-11

## Context and Problem Statement

IronPLC currently represents the TIME data type as a 64-bit signed integer with microsecond precision. This does not match the IEC 61131-3 standard, which defines TIME as a 32-bit type (Edition 2) and LTIME as a 64-bit type (Edition 3, 2013). The internal representation needs to be corrected so that TIME occupies 32 bits and LTIME occupies 64 bits.

A related question is what time unit to use. IEC 61131-3 states that the resolution of TIME is implementation-defined. The current codebase uses microseconds, but with a 32-bit signed integer, microseconds would limit the maximum duration to approximately 35 minutes — far too small for practical PLC programs (timers commonly range from milliseconds to hours or days).

## Decision Drivers

* **Standard compliance** — TIME must be 32 bits per IEC 61131-3 Edition 2; LTIME must be 64 bits per Edition 3
* **Practical range** — PLC timers commonly measure intervals from milliseconds to days; the unit must provide useful range within 32 bits
* **Industry alignment** — most PLC vendors (Siemens, Beckhoff, CODESYS) use millisecond resolution for TIME
* **Simplicity** — a single unit for both TIME and LTIME reduces implementation complexity
* **Precision** — sub-millisecond timing is needed for some applications but not the common case

## Considered Options

### Option A: Milliseconds for both TIME and LTIME

TIME stores i32 milliseconds (max ~24.8 days). LTIME stores i64 milliseconds (max ~292 million years).

* Good, because 24.8 days covers all common timer use cases for 32-bit TIME
* Good, because same unit for both types simplifies type promotion (TIME → LTIME) and arithmetic
* Good, because the `T#Xms` literal format directly matches the storage unit
* Good, because it matches the most common industry practice
* Neutral, because LTIME has more range than needed but sub-millisecond precision is lost
* Bad, because sub-millisecond literals (`T#500us`) truncate to zero for TIME, and LTIME loses microsecond precision

### Option B: Microseconds for both TIME and LTIME

TIME stores i32 microseconds (max ~35.8 minutes). LTIME stores i64 microseconds (max ~292,000 years).

* Good, because microsecond precision is preserved
* Good, because same unit for both types
* Bad, because 35.8 minutes is far too limited for TIME — many PLC programs use multi-hour or multi-day timers
* Bad, because it contradicts industry practice

### Option C: Milliseconds for TIME, microseconds for LTIME

TIME stores i32 milliseconds. LTIME stores i64 microseconds.

* Good, because TIME gets practical range (24.8 days)
* Good, because LTIME gets high precision (microsecond)
* Bad, because mixed units complicate TIME → LTIME promotion (requires multiply by 1000)
* Bad, because mixed units complicate timer FBs that may need to support both types
* Bad, because developers must remember which unit applies to which type

## Decision Outcome

**Option A: Milliseconds for both TIME and LTIME.**

Both types use millisecond precision. TIME is a 32-bit signed integer (i32 ms, range approximately ±24.8 days). LTIME is a 64-bit signed integer (i64 ms, range effectively unlimited).

### Representation Summary

| Type | Storage | Unit | Max Duration | IEC Edition |
|------|---------|------|-------------|------------|
| TIME | i32 | milliseconds | ~24.85 days | Edition 2 (1993) |
| LTIME | i64 | milliseconds | ~292 million years | Edition 3 (2013) |

### Impact on Existing Components

**IntermediateType** (`analyzer/src/intermediate_type.rs`): The `Time` variant gains a `size: ByteSized` parameter, following the pattern already used by `Int`, `UInt`, `Real`, and `Bytes`. TIME maps to `Time { size: ByteSized::B32 }` and LTIME maps to `Time { size: ByteSized::B64 }`.

**Duration literals**: The codegen compiles `T#Xs` and `LTIME#Xs` literals using `whole_milliseconds()` from the DSL's `DurationLiteral`. For TIME, the value is emitted as an i32 constant. For LTIME, it is emitted as an i64 constant. Sub-millisecond literals (e.g., `T#500us`) truncate to zero for TIME; a future analyzer rule could warn about this.

**Timer function blocks** (TON, TOF, TP): PT and ET fields change from i64 microseconds to i32 milliseconds. The VM intrinsics convert from the VM's microsecond cycle time to milliseconds for elapsed-time calculations.

**Debug type tag** (ADR-0019): Tag 17 (`TIME`) changes interpretation from `raw as i64` microseconds to `raw as i32` milliseconds. A new tag is added for LTIME (`raw as i64` milliseconds).

**Playground display**: `format_time_value()` changes to accept i32 milliseconds for TIME. A separate formatter handles LTIME's i64 milliseconds.

### Consequences

* Good, because TIME now matches the IEC 61131-3 standard width (32 bits)
* Good, because type promotion from TIME to LTIME requires no unit conversion (both are milliseconds)
* Good, because the 24.8-day range covers all standard timer use cases
* Good, because timer FBs operate in the same unit regardless of TIME/LTIME
* Neutral, because sub-millisecond duration literals silently truncate for TIME
* Bad, because this is a breaking change — existing .iplc bytecode files with TIME variables are incompatible (acceptable for pre-1.0)

## More Information

### Why Not Nanoseconds?

IEC 61131-3 Edition 3 does not mandate a specific resolution for LTIME. While nanosecond precision would match the `time::Duration` type used internally in the DSL, it provides no practical benefit: PLC scan cycles are typically 1–100 ms, so nanosecond resolution in timer values is meaningless. Milliseconds keep the implementation simple and avoid wasting 6 orders of magnitude of precision on unused resolution.

### Relationship to ADR-0019

ADR-0019 defines the debug type tag encoding. This ADR changes the interpretation of tag 17 (TIME) from `raw as i64` microseconds to `raw as i32` milliseconds, and adds a new tag for LTIME. ADR-0019's table should be updated accordingly.
