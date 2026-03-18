# ADR-0025: Unsigned Representation for DATE, TIME_OF_DAY, DATE_AND_TIME

## Status

Accepted

## Context

The IEC 61131-3 standard defines three datetime types beyond duration:

- **DATE** (D#) - Calendar date (e.g., D#2024-01-01)
- **TIME_OF_DAY** (TOD#) - Time within a day (e.g., TOD#12:30:00)
- **DATE_AND_TIME** (DT#) - Combined date and time (e.g., DT#2024-01-01-12:30:00)

ADR-0021 established that TIME and LTIME (durations) use **signed** integers because
durations can be negative (e.g., subtracting two time values). We need to decide the
representation for these absolute datetime types.

## Decision

Use **unsigned** integers matching the CODESYS/Beckhoff industry standard:

| Type | Storage | Unit | Epoch | Range |
|------|---------|------|-------|-------|
| DATE | u32 | seconds since 1970-01-01 | Unix epoch | to 2106-02-07 |
| TIME_OF_DAY | u32 | ms since midnight | midnight = 0 | 0 to 86,399,999 |
| DATE_AND_TIME | u32 | seconds since 1970-01-01 | Unix epoch | to 2106-02-07-06:28:15 |
| LDATE | u64 | nanoseconds since 1970-01-01 | Unix epoch | to ~2554 |
| LTOD | u64 | nanoseconds since midnight | midnight = 0 | 0 to 86,399,999,999,999 |
| LDT | u64 | nanoseconds since 1970-01-01 | Unix epoch | to ~2554 |

The codegen maps these types with `Signedness::Unsigned`, so the VM uses unsigned
comparison opcodes (GT_U32, LT_U32, etc.) which are already implemented.

### Industry Standard Alignment

We follow the CODESYS/Beckhoff representation rather than inventing our own because:

1. DATE and DT use u32 seconds since 1970-01-01 (Unix epoch), which is the de facto
   standard across PLC vendors.
2. TOD uses u32 milliseconds since midnight, providing sub-second precision for
   time-of-day operations.
3. Long variants (LDATE, LTOD, LDT) use u64 nanoseconds, matching the IEC 61131-3
   Third Edition pattern where L-prefixed types extend resolution to nanoseconds.
4. This ensures compatibility with existing PLC programs and libraries.

### Why Unsigned (Not Signed Like TIME/LTIME)

- **Durations can be negative** (T#5s - T#10s = T#-5s), so TIME/LTIME need signed storage.
- **Absolute timestamps cannot be negative** - a date before 1970-01-01 or a
  time-of-day before midnight has no meaning in IEC 61131-3.
- Using unsigned doubles the positive range and prevents invalid negative values.

## Consequences

- DATE, TIME_OF_DAY, and DATE_AND_TIME use the same unsigned comparison/division
  opcodes as UDINT/ULINT (no new VM opcodes needed).
- The playground displays formatted values (D#YYYY-MM-DD, TOD#HH:MM:SS, DT#...).
- IntermediateType now has three separate variants (Date, TimeOfDay, DateAndTime)
  instead of the single generic Date variant, enabling proper type checking.
- Edition 3 long variants (LDATE, LTOD, LDT) use u64 nanoseconds with the same
  Unix epoch, ready for when parser support is added.
