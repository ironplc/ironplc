# Exact Type Matching for Function Arguments

status: proposed
date: 2026-03-12

## Context and Problem Statement

When a user-defined function declares `VAR_INPUT A : DINT; END_VAR` and the caller passes an `INT` expression, should the compiler accept this (implicit widening) or reject it (exact match required)?

## Decision Drivers

* **Safety-first design principle** (ADR-0005) — implicit conversions can mask bugs, especially in safety-critical PLC code
* **IEC 61131-3 type system** — the standard defines type hierarchies but leaves implementation latitude on implicit conversions
* **Implementation simplicity** — exact matching is straightforward; widening requires a type compatibility matrix and potentially implicit conversion codegen
* **Explicitness** — IEC 61131-3 provides explicit conversion functions (e.g., `INT_TO_DINT`) for all type pairs

## Considered Options

* Exact type matching — argument type must exactly match parameter type
* Implicit widening — safe widening conversions (e.g., INT → DINT) are accepted silently

## Decision Outcome

Chosen option: "Exact type matching", because it aligns with the project's safety-first principle, avoids hidden type conversions, and keeps the analysis rule simple. Users can use explicit conversion functions when types differ.

### Consequences

* Good, because no implicit conversions can introduce subtle bugs
* Good, because the type-checking rule is simple: compare `TypeName` equality
* Good, because all type conversions are visible in the source code
* Bad, because users must write explicit conversion calls even for safe widenings — but IEC 61131-3 programmers are accustomed to explicit conversions
* Neutral, because implicit widening can be added later as an opt-in relaxation without breaking existing programs
