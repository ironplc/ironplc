# Expanded Implicit Type Widening

status: proposed
date: 2026-04-03

## Context and Problem Statement

ADR-0029 introduced implicit integer widening (e.g. SINT → INT → DINT → LINT) but explicitly excluded bit-string types, REAL/LREAL, and cross-family conversions. This leaves several common patterns unsupported that work in RuSTy, CODESYS, and TwinCAT:

1. Passing an INT variable to a REAL parameter (integer → real)
2. Passing a BYTE variable to a WORD parameter (bit-string widening)
3. Passing a BYTE variable to an INT parameter (cross-family)
4. Passing a bare integer literal `0` to a BYTE parameter (cross-family literal)
5. Assigning a BYTE return value to an INT variable (cross-family return)

Cases 1 and 2 fall within the IEC 61131-3 type hierarchy (ANY_NUM and ANY_BIT respectively). Cases 3–5 cross from ANY_BIT to ANY_INT, which is outside the standard's implicit widening rules.

## Decision Drivers

* **IEC 61131-3 compliance** — the standard's type hierarchy places ANY_INT and ANY_REAL under ANY_NUM, and defines BYTE/WORD/DWORD/LWORD under ANY_BIT, implying widening within each branch
* **Safety** — lossless integer-to-real widening preserves all values; bit-string widening is zero-extension
* **Industry alignment** — CODESYS, TwinCAT, and RuSTy support all five cases
* **Practical impact** — OSCAT and similar libraries rely on these patterns

## Considered Options

* Keep current restrictions — users must write explicit conversion calls
* Allow standard-compliant widening by default, gate cross-family behind a flag

## Decision Outcome

Chosen option: "Allow standard-compliant widening by default, gate cross-family behind `--allow-cross-family-widening`."

### Standard Widening (enabled by default)

#### Integer → REAL/LREAL (lossless only)

An integer type can implicitly widen to a real type when all values of the source type are exactly representable in the target type:

* SINT(8), INT(16), USINT(8), UINT(16) → REAL (32-bit float, 23-bit mantissa)
* Any integer type → LREAL (64-bit float, 52-bit mantissa)

Not allowed (lossy): DINT(32), LINT(64), UDINT(32), ULINT(64) → REAL. Use explicit conversion (e.g. `DINT_TO_REAL(x)`) or widen to LREAL.

#### Bit-string widening

Wider bit-string types can accept narrower bit-string values:

* BYTE(8) → WORD(16) → DWORD(32) → LWORD(64)

BOOL is excluded. While IEC 61131-3 places BOOL under ANY_BIT, it is semantically a boolean (TRUE/FALSE), not a numeric bit container. Standard implementations do not define BOOL → BYTE as implicit widening.

### Cross-Family Widening (requires `--allow-cross-family-widening`)

These conversions cross the ANY_BIT / ANY_INT boundary and are not part of the IEC 61131-3 standard:

* Bit-string → integer: BYTE → INT, WORD → DINT, etc. (target must be strictly wider)
* Bare integer literal → bit-string: `0` where BYTE is expected
* Return type: function returning BYTE assigned to INT variable

The flag is enabled by default in the `Rusty` dialect.

### Scope

This applies to:

* **Function arguments (P4026)** — argument type compatible with parameter type
* **Function return types (P4027)** — return type compatible with assignment target

### Consequences

* Good, because standard-compliant widening works without any flags
* Good, because cross-family widening is available for RuSTy compatibility
* Good, because the flag makes the non-standard behavior explicit
* Good, because all standard widening conversions are lossless
* Neutral, because explicit conversion functions remain available and recommended

## Relationship to Prior ADRs

* **ADR-0022** (exact type matching): Still applies for non-widening cases
* **ADR-0028** (literal type inference): Bare literal → REAL/LREAL remains as-is; bare literal → ANY_BIT is new and gated
* **ADR-0029** (integer widening): Extended to include integer → real (lossless) and bit-string widening within ANY_BIT
