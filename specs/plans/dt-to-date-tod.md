# Plan: Add DT_TO_DATE and DT_TO_TOD Functions

## Context

IronPLC is missing two IEC 61131-3 standard library functions for decomposing DATE_AND_TIME values:
- **DT_TO_DATE** — extracts the DATE portion from a DATE_AND_TIME
- **DT_TO_TOD** — extracts the TIME_OF_DAY portion from a DATE_AND_TIME

These are the logical complement of CONCAT_DATE_TOD (already implemented). The standard also defines long-form aliases: DATE_AND_TIME_TO_DATE and DATE_AND_TIME_TO_TIME_OF_DAY.

### Type representations (ADR-0025)
| Type | Width | Unit |
|------|-------|------|
| DATE | u32 | seconds since 1970-01-01 (midnight-aligned) |
| TOD | u32 | milliseconds since midnight |
| DT | u32 | seconds since 1970-01-01 |

### Math
- **DT_TO_DATE**: `DT - (DT % 86400)` → strips time, keeps date at midnight
- **DT_TO_TOD**: `(DT % 86400) * 1000` → extracts seconds since midnight, converts to ms

## Changes

### 1. Analyzer — Register function signatures
**File:** `compiler/analyzer/src/intermediates/stdlib_function.rs`
**Location:** Inside `get_time_functions()`, after CONCAT_DATE_TOD

Add 4 signatures (2 functions x 2 aliases each):
- DT_TO_DATE(IN: DATE_AND_TIME) -> DATE
- DATE_AND_TIME_TO_DATE(IN: DATE_AND_TIME) -> DATE
- DT_TO_TOD(IN: DATE_AND_TIME) -> TIME_OF_DAY
- DATE_AND_TIME_TO_TIME_OF_DAY(IN: DATE_AND_TIME) -> TIME_OF_DAY

### 2. Codegen — Add compilation functions
**File:** `compiler/codegen/src/compile.rs`

Add match arms for the lowercase function names and two new compilation functions:

- **compile_dt_to_date**: Compiles `IN - (IN % 86400)` using W32 unsigned ops
- **compile_dt_to_tod**: Compiles `(IN % 86400) * 1000` using W32 unsigned ops

### 3. Tests — End-to-end tests
**File:** `compiler/codegen/tests/end_to_end_time_functions.rs`

Test with DT#2000-01-01-12:00:00 = 946728000 seconds:
- DT_TO_DATE → 946684800 (D#2000-01-01 midnight)
- DT_TO_TOD → 43200000 (TOD#12:00:00 in ms)

### 4. Documentation
New RST files for each function plus index.rst updates.

## Scope decisions
- Long-form aliases included (DATE_AND_TIME_TO_DATE, DATE_AND_TIME_TO_TIME_OF_DAY)
- 64-bit variants (LDT_TO_LDATE, LDT_TO_LTOD) out of scope — follow-up
- plc2plc: no changes needed (generic function call rendering)
