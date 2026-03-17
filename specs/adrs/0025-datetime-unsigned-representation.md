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

Use **unsigned** integers for all three datetime types:

| Type | Storage | Unit | Epoch | Range |
|------|---------|------|-------|-------|
| DATE | u32 | days since 0001-01-01 | Julian day 1721426 | ~11.7M years |
| TIME_OF_DAY | u32 | ms since midnight | midnight = 0 | 0 to 86,399,999 |
| DATE_AND_TIME | u64 | ms since 0001-01-01 00:00:00 | same epoch | ~584M years |

The codegen maps these types with `Signedness::Unsigned`, so the VM uses unsigned
comparison opcodes (GT_U32, LT_U32, etc.) which are already implemented.

### Epoch Choice

We use 0001-01-01 (Julian day 1721426) as the epoch rather than a Unix-style epoch
(1970-01-01) because:

1. IEC 61131-3 does not specify a minimum date, and PLC applications in historical
   data processing may need dates before 1970.
2. Using year 1 avoids negative offsets entirely, which aligns with unsigned storage.
3. The u32 range for DATE (4+ billion days) far exceeds practical needs regardless
   of epoch.

### Why Unsigned (Not Signed Like TIME/LTIME)

- **Durations can be negative** (T#5s - T#10s = T#-5s), so TIME/LTIME need signed storage.
- **Absolute timestamps cannot be negative** - a date before the epoch 0001-01-01 or a
  time-of-day before midnight has no meaning in IEC 61131-3.
- Using unsigned doubles the positive range and prevents invalid negative values.

## Consequences

- DATE, TIME_OF_DAY, and DATE_AND_TIME use the same unsigned comparison/division
  opcodes as UDINT/ULINT (no new VM opcodes needed).
- The playground displays formatted values (D#YYYY-MM-DD, TOD#HH:MM:SS, DT#...).
- IntermediateType now has three separate variants (Date, TimeOfDay, DateAndTime)
  instead of the single generic Date variant, enabling proper type checking.
- Edition 3 long variants (LDATE, LTOD, LDT) can be added later with the same
  representation but u64 storage.
