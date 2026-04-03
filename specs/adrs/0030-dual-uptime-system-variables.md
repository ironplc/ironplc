# ADR-0030: Dual System Uptime Variables (TIME and LTIME)

## Status

Proposed

## Context

IronPLC needs to expose the VM's monotonic uptime counter so that user code
can implement vendor-specific time functions like CODESYS's `TIME()` in pure ST.
The IEC 61131-3 standard does not define a monotonic elapsed timer function —
`TIME()` returning ms-since-boot is a CODESYS vendor extension.

The original design proposed a single `__SYSTEM_TIME_MS : TIME` global (i32 ms).
Review identified several issues:

1. **Wrap hazard**: i32 wraps at ~24.8 days. PLCs run continuously for months or
   years, so this wrap will occur in production. Siemens S7's `TIME_TCK` has the
   same limitation and is widely criticized for it.
2. **Signed/unsigned mismatch with CODESYS**: CODESYS represents TIME as unsigned
   (UDINT-backed, wrapping at ~49.7 days). IronPLC's TIME is signed (i32 per
   ADR-0021). Elapsed-duration math and comparisons behave differently at the wrap
   boundary.
3. **Naming ambiguity**: "SYSTEM_TIME" could be mistaken for wall-clock time. This
   value is uptime (ms since VM start).

## Decision Drivers

* **Avoid unnecessary type conversions** — users should be able to use the system
  variable directly with timer FBs and time arithmetic without explicit conversion
* **Safety for long-running systems** — provide a variant that does not wrap in
  any practical deployment
* **CODESYS library compatibility** — enable writing a `TIME()` wrapper that
  behaves like CODESYS for elapsed-duration subtraction
* **Clarity** — the variable name should unambiguously communicate "uptime"

## Considered Options

### Option A: Single `__SYSTEM_TIME_MS : TIME` (i32)

The original design. One variable, simplest implementation.

* Bad, because i32 wraps at ~24.8 days — a real hazard for continuously-running PLCs
* Bad, because signed wrap differs from CODESYS's unsigned wrap
* Good, because no conversion needed for TIME-typed parameters

### Option B: Single `__SYSTEM_UPTIME_MS : LTIME` (i64)

One variable using the 64-bit long time type. No practical wrap (~292M years).

* Good, because wrap is not a concern
* Bad, because using it with TIME-typed timer FB inputs (TON.PT, etc.) requires
  an LTIME_TO_TIME conversion on every access — this is the most common use case
* Neutral, because LTIME is only available with Edition 3

### Option C: Single `__SYSTEM_UPTIME_MS : UDINT` (u32)

Raw unsigned counter matching CODESYS's internal representation.

* Good, because unsigned arithmetic matches CODESYS exactly (~49.7 day wrap)
* Bad, because UDINT is not a time type — cannot be passed to timer FBs or used
  in time arithmetic without explicit conversion (UDINT_TO_TIME)
* Bad, because it exposes a raw integer where a time-typed value is more natural

### Option D: Dual variables — `__SYSTEM_UP_TIME : TIME` and `__SYSTEM_UP_LTIME : LTIME`

Two system globals, one per time width. Users choose based on their needs.

* Good, because TIME variant works directly with timer FBs — no conversion needed
* Good, because LTIME variant never wraps in practice
* Good, because users self-select: short-lived timing uses TIME, long-running uses LTIME
* Good, because a CODESYS-compatible `TIME()` library function trivially wraps
  `__SYSTEM_UP_TIME` (elapsed-duration subtraction works correctly within any
  ~24.8-day window, which covers all practical timer intervals)
* Neutral, because two variable slots are consumed even if only one is used
* Neutral, because the TIME variant still wraps at ~24.8 days (but users who
  care about long durations use the LTIME variant instead)

## Decision Outcome

**Option D: Dual variables.**

The primary driver is avoiding unnecessary conversions. The most common use case
is passing uptime to timer-related logic that expects TIME. Forcing users through
LTIME_TO_TIME or UDINT_TO_TIME on every access adds friction and error potential
for no benefit when the timing interval is well within the 24.8-day range (as it
almost always is for timer FBs).

For the less common case of long-running elapsed measurements, the LTIME variant
provides a wrap-free alternative without requiring any compromise in the common path.

### Variable Definitions

| Variable | Type | Storage | Wrap |
|----------|------|---------|------|
| `__SYSTEM_UP_TIME` | TIME | i32 ms | ~24.8 days |
| `__SYSTEM_UP_LTIME` | LTIME | i64 ms | ~292M years |

Both are injected at the start of the global variable table (indices 0 and 1)
and written by the VM before each scan round.

### Naming

"UP_TIME" / "UP_LTIME" instead of "TIME_MS" to:
- Clearly communicate this is uptime, not wall-clock time
- The `_MS` suffix is dropped because the type already implies the unit (TIME is
  defined as milliseconds per ADR-0021)

## Consequences

* Two variable slots are always consumed when the feature is enabled, even if
  user code only references one. This is negligible overhead (16 bytes total).
* The TIME variant wraps at ~24.8 days. Documentation must clearly state this
  and recommend the LTIME variant for durations exceeding days.
* A CODESYS-compatible `TIME()` function is a pure library concern:
  `FUNCTION TIME : TIME ... TIME := __SYSTEM_UP_TIME; END_FUNCTION`
* Future system variables (cycle count, etc.) follow the same injection pattern
  at subsequent indices.
