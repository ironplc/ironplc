# Assignment and Standard-Library Argument Type Checking — Implementation Plan

## Overview

The type checker fails to report several categories of type mismatch:

1. A variable declared `BOOL` but assigned a `REAL` expression (plain assignment
   statement, RHS is not a function call) is accepted silently.
2. A standard-library function (e.g. `SIN`, which expects `ANY_REAL`) called with
   an incompatible argument (e.g. a `BOOL` variable) is accepted silently.
3. A type-conversion function (e.g. `UINT_TO_REAL`, whose input parameter is the
   concrete type `UINT`) called with a mismatched argument (e.g. a `UDINT`) is
   accepted silently.

Root causes:

- `rule_function_call_type_check.rs` **skips all standard-library functions**
  ("they use `ANY_*` generic types which require different handling"), so gaps
  (2) and (3) are never checked.
- No rule checks the compatibility of an **assignment statement's target** against
  the **whole RHS expression** when the RHS root is not a user-function call. Only
  user-function-return-to-target is checked (`check_return_type`), so gap (1) is
  never checked.

## Scope

In `compiler/analyzer/src/rule_function_call_type_check.rs`:

1. **Generalize `are_types_compatible`** to also accept a *generic* expected type
   (e.g. `ANY_REAL`, `ANY_NUM`, `ANY_ELEMENTARY`). This lets stdlib parameter types
   (which are generic) be checked against concrete or generic argument types, reusing
   `GenericTypeName::is_compatible_with`. The existing widening/literal-inference
   rules (ADR-0028/0029/0031) are preserved unchanged.

2. **Check standard-library function arguments.** Stop skipping stdlib functions for
   argument checking. Each positional argument is checked against its parameter type.
   Emits `P4026` (`FunctionCallArgTypeMismatch`) — the existing code. Output-argument
   `NotImplemented` emission stays limited to user functions. Extensible functions
   (MUX) still only check declared positional parameters.

3. **Check assignment-statement target type.** For an assignment whose target is a
   simple named variable and whose RHS root is *not* a function call, verify the
   target's (elementary-resolved) declared type is compatible with the RHS
   `resolved_type`. Emits a new problem code **`P4035` (`AssignmentTypeMismatch`)**.
   Function-call RHS keeps its existing `P4027` path (`check_return_type`).

Conservative guards to avoid false positives:

- Assignment check runs only when the target resolves to an **elementary** type and
  the RHS `resolved_type` is elementary-or-generic. User types (enum/struct/FB/array
  element/string-with-length/REF) are skipped.
- Argument check runs only when the argument has a resolved type.

## New problem code

`P4035,AssignmentTypeMismatch,Assignment value type does not match assignment target type`
added to `compiler/problems/resources/problem-codes.csv`, documented in
`docs/reference/compiler/problems/P4035.rst`.

## Tests

- Unit tests in the rule module: BOOL := REAL expr (P4035), SIN(BOOL) (P4026),
  UINT_TO_REAL(UDINT) (P4026), plus passing cases (SIN(real), UDINT_TO_REAL(udint),
  INT widened to REAL assignment, matching assignments) to lock down the boundaries.
- A `generic`-expected branch unit test for `are_types_compatible`.
- Verify the reported reproduction program now emits diagnostics via the CLI.
- Full CI (`cd compiler && just`) must pass — the existing corpus (first_steps, e2e,
  plc2plc round-trip) exercises many stdlib calls and assignments and is the
  regression guard.

## Out of scope

- Bit-string family for functional `AND`/`OR`/`XOR` (modeled as `BOOL` in stdlib,
  a pre-existing limitation) beyond what the corpus needs.
- Assignment checking for non-elementary targets (enum/struct/array/string).
- Return-type checking for stdlib function calls (the argument check already covers
  the reported cases and avoids inference-quirk false positives).
