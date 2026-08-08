# Plan: Reduce repeated builtin-lookup match arms in codegen

## Problem

`compiler/codegen/src/compile_call.rs::lookup_builtin` writes a builtin opcode
lookup table longhand. Three families of arms share an identical shape and
account for ~55 lines of duplication that the dupes gate flags:

1. **Float-only transcendentals** (SQRT, LN, LOG, EXP, SIN, COS, TAN, ASIN,
   ACOS, ATAN, ATAN2) — 11 arms, each a 5-line `match op_width` returning the
   F32/F64 opcode and `None` for integer widths.
2. **All-four-widths numeric** (EXPT, ABS, SEL) — each a 6-line
   `Some(match op_width { W32 => _I32, W64 => _I64, F32 => _F32, F64 => _F64 })`.
3. **Signed/unsigned + float numeric** (MIN, MAX, LIMIT) — each an 8-line
   `Some(match (op_width, signedness) { ... })`.

## Judgment

A small set of local `macro_rules!` genuinely improves this file: each arm
collapses to one line, greppability is preserved (both the name string and each
opcode identifier still appear verbatim), and the shared shape is documented
once. The workspace has no `paste` dependency, so opcode identifiers are passed
explicitly rather than concatenated. This is a clear net win over the longhand
table, so we implement it rather than documenting the status quo.

## Changes

- Add three module-scoped macros above `lookup_builtin`:
  `numeric_builtin!`, `signed_numeric_builtin!`, `float_builtin!`.
- Rewrite the `lookup_builtin` arms to use them. Signature and behavior are
  identical — every `(name, op_width, signedness)` maps to the same opcode.
- Add focused BDD-named unit tests locking the mapping for one representative
  of each family plus the integer-width `None` case.

## Validation

Full CI pipeline (compile, coverage >=85%, clippy, fmt, dupes gate) via the
shared lock. VM end-to-end tests already cover these builtins; the new unit
tests guard the table structure directly.
