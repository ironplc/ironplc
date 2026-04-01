# Literal Type Inference Across Numeric Families

status: proposed
date: 2026-04-01

## Context and Problem Statement

Bare integer literals (e.g. `0`, `42`) resolve as `ANY_INT` during type resolution — they have no declared type. When passed to a function parameter declared as `REAL` or `LREAL`, the compiler rejects them with P4026 (type mismatch) because `ANY_INT` is not in the `REAL`/`LREAL` type family.

This is inconsistent: variable initializers already accept integer literals for real types (`VAR x : REAL := 10; END_VAR` compiles), and `ANY_INT` literals already resolve to any concrete integer type (`INT`, `SINT`, `DINT`, etc.) via `GenericTypeName::is_compatible_with`.

The question is whether bare integer literals should also be inferred as `REAL`/`LREAL` in function argument contexts, crossing the integer/real family boundary.

## Decision Drivers

* **Consistency** — variable initializers already allow integer literals for real targets; function arguments should behave the same way
* **Untyped vs typed** — a bare literal `0` is `ANY_INT` (untyped), not `INT` (typed). Type inference for untyped values is distinct from implicit widening of typed values (ADR-0022)
* **Industry practice** — CODESYS, TwinCAT, RuSTy, and other IEC 61131-3 implementations accept this pattern
* **Practical impact** — OSCAT's `RDM(0)` pattern (integer literal to `REAL` parameter) blocks 294 testable functions
* **Safety** — integer-to-real conversion is lossless for typical literal values; this is not narrowing

## Considered Options

* Reject (status quo) — bare integer literals cannot match `REAL`/`LREAL` parameters
* Allow with `--allow-*` flag — gate behind a vendor-extension flag
* Allow universally — bare integer literals infer to `REAL`/`LREAL` in all modes

## Decision Outcome

Chosen option: "Allow universally", because bare literals are untyped and type inference across numeric families is safe, consistent with initializer behavior, and standard industry practice.

### Scope

This decision applies **only to bare (untyped) literals** — values whose resolved type is a generic type like `ANY_INT`. It does **not** change behavior for:

* **Typed variables** — an `INT` variable passed to a `REAL` parameter is still rejected per ADR-0022
* **Typed literals** — `INT#5` passed to a `REAL` parameter is still rejected
* **Narrowing** — `ANY_REAL` literals (e.g. `3.14`) passed to an `INT` parameter are still rejected

### Consequences

* Good, because function arguments and variable initializers now behave consistently for bare literals
* Good, because OSCAT and similar libraries work without requiring explicit `INT_TO_REAL(0)` for every literal
* Good, because no `--allow-*` flag is needed — this is standard IEC 61131-3 practice, not a vendor extension
* Good, because ADR-0022 (exact type matching for typed expressions) remains in force
* Neutral, because the codegen already handles integer literals in float context — no codegen changes required

## Relationship to ADR-0022

ADR-0022 decides that typed expressions require exact type matching. This ADR carves out an exception for **untyped literals only**. The distinction:

| Expression | Type | REAL param | Rationale |
|---|---|---|---|
| `0` (bare literal) | `ANY_INT` | Accepted | Type inference — literal has no fixed type |
| `INT#0` (typed literal) | `INT` | Rejected | Exact match per ADR-0022 |
| `x` (INT variable) | `INT` | Rejected | Exact match per ADR-0022 |
