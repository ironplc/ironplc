# Implicit Integer Widening

status: proposed
date: 2026-04-02

## Context and Problem Statement

ADR-0022 established exact type matching for user-defined function arguments: passing an `INT` variable to a `DINT` parameter is rejected, requiring an explicit `INT_TO_DINT()` call. While safe, this is stricter than the IEC 61131-3 standard and every major commercial PLC runtime (CODESYS, TwinCAT, Siemens TIA Portal, Rockwell Logix 5000, PLCnext).

This strictness blocks compatibility with widely-used libraries like OSCAT, where patterns like `EVEN(disc)` (INT argument, DINT parameter) are pervasive — 294 functions fail because of this restriction.

## Decision Drivers

* **IEC 61131-3 compliance** — the standard permits implicit widening when the source type's range and precision are fully preserved in the target type
* **Industry alignment** — CODESYS, TwinCAT, Siemens, Rockwell, and PLCnext all support implicit integer widening
* **Practical impact** — OSCAT and similar libraries rely on implicit widening; requiring explicit casts for every call is impractical
* **Safety** — integer widening is always lossless; no data is lost or truncated

## Considered Options

* Keep exact matching (status quo) — users must write explicit conversion calls
* Allow implicit integer widening by default — compiler accepts narrower integer types where wider types are expected

## Decision Outcome

Chosen option: "Allow implicit integer widening by default", because the conversions are lossless, standard-compliant, and universally supported by commercial runtimes.

### Widening Rules

A source integer type can implicitly widen to a target integer type when the source's full value range fits within the target's range:

* **Signed chain:** SINT(8) → INT(16) → DINT(32) → LINT(64)
* **Unsigned chain:** USINT(8) → UINT(16) → UDINT(32) → ULINT(64)
* **Cross-sign:** unsigned n-bit → signed m-bit where m > n (e.g., USINT → INT, UINT → DINT, UDINT → LINT)

Not allowed:

* Signed → unsigned (any width)
* Unsigned → signed of equal or smaller width
* Any narrowing conversion
* Bit-string types (BYTE, WORD, DWORD, LWORD) — separate type family
* Integer → REAL/LREAL — separate concern

### Scope

This applies to:

* **Function arguments (P4026)** — narrower integer argument passed to wider integer parameter
* **Function return types (P4027)** — narrower integer return value assigned to wider integer variable

### Consequences

* Good, because OSCAT and similar libraries compile without modification
* Good, because IronPLC aligns with industry-standard behavior
* Good, because no `--allow-*` flag is needed — this is standard IEC 61131-3 practice
* Good, because all widening conversions are lossless by definition
* Neutral, because explicit conversion functions remain available and recommended for cross-family conversions (integer ↔ real, integer ↔ bit-string)

## Relationship to ADR-0022

This decision supersedes ADR-0022's exact-matching rule **for integer types only**. ADR-0022's principle still applies to:

* Integer ↔ real type mismatches
* Integer ↔ bit-string type mismatches
* Narrowing conversions
* User-defined type mismatches
