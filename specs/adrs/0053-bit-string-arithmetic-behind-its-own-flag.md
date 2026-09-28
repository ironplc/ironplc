# Bit-String Arithmetic Behind Its Own Flag

status: accepted
date: 2026-09-19

## Context and Problem Statement

IEC 61131-3 defines the arithmetic functions `ADD`, `SUB`, `MUL` and `DIV`
over `ANY_NUM` (Table 24) and over the time and date types (Table 30). It
defines no arithmetic on `ANY_BIT`. CODESYS, TwinCAT and RuSTy accept it
anyway, treating a bit string as the unsigned integer of its width, and
programs written for them use it: `b + 1` on a `BYTE` is a counter, `w * 2`
on a `WORD` is a shift.

IronPLC accepts such expressions today only because the operator spellings of
`+`, `-`, `*` and `/` are not operand-checked at all. The
[Arithmetic Operator Overloads](../design/arithmetic-operator-overloads.md)
design holds the operators to what the standard defines, so `b + 1` becomes a
type error in the strict dialects, and the question is where the vendor
behaviour lives.

The obvious home is `--allow-cross-family-widening`, the flag that already
governs the bit-string-to-integer boundary
([ADR-0031](0031-expanded-implicit-type-widening.md)). Its 2026-09-13
amendment argues against that: one flag had gated three rules and named one
of them, and a user reading "widening" could not tell they were also enabling
a bidirectional equal-width conversion and a literal-typing rule. The flag was
split into three, one per rule, so that each name says what it enables.

Bit-string arithmetic is a fourth rule, and a different shape again. Widening
moves a value into a strictly wider type; this rule moves nothing and changes
no type. `b + 1` on `BYTE` stays `BYTE`. It admits an operand category the
standard's arithmetic does not name, rather than relaxing a check between two
categories the standard does name. The compatibility predicate cannot express
it: `BYTE` is not in `ANY_NUM` under any flag, so the rule lives in the
resolver, not in `are_types_compatible`.

## Decision Drivers

* **A flag names one rule.** The amendment to ADR-0031 is the project's
  standing position on this, and it was reached by finding that a shared flag
  had misled.
* **Dialects, not flags, are what most users select.** A per-rule flag costs
  a user of the `CODESYS` dialect nothing, because the dialect turns it on.
* **The strict dialects reject what the standard does not define**
  ([ADR-0036](0036-no-ironplc-dialect.md) and
  [ADR-0040](0040-dialect-violations-diagnosed-in-policy-phase.md)): an
  extension is diagnosed unless a dialect or flag admits it.
* **No silent behaviour change.** A program that compiled cleanly and
  correctly, `b + 1` on `BYTE` in the strict dialect, will now be reported.
  That has to be a documented consequence, not a side effect.

## Considered Options

* Gate bit-string arithmetic behind `--allow-cross-family-widening`
* Gate it behind a new flag, `--allow-bit-string-arithmetic`
* Accept bit-string arithmetic in every dialect

## Decision Outcome

Chosen option: a new flag, `--allow-bit-string-arithmetic`, enabled by default
in the `Rusty`, `CODESYS` and `TwinCAT` dialects, the same three that enable
the cross-family flags, because those are the compilers whose behaviour it
follows.

With the flag on, the arithmetic resolver judges a `BYTE`, `WORD`, `DWORD` or
`LWORD` operand as the unsigned integer of its width. `BOOL` is excluded, as
ADR-0031 excludes it from bit-string widening. Two bit-string operands give
the wider bit-string type; a bit-string operand with an integer or real
operand gives whatever the ordinary widening between that operand and the
bit string's unsigned integer gives. The flag affects only the arithmetic
resolver: it does not make a bit string acceptable where an `ANY_NUM`
parameter is expected, which stays the business of the cross-family flags.

### Consequences

* Good, because the flag's name says what it enables and nothing else.
* Good, because no dialect's behaviour changes: every dialect that accepts
  `b + 1` today keeps accepting it.
* Good, because the strict dialects report what the standard does not define,
  with a diagnostic (P4049) that names the operand types.
* Bad, because a program compiled with no dialect and no flags that relied on
  bit-string arithmetic now fails to compile until the flag or a dialect is
  chosen. Compiling with no dialect selects strict Edition 2, in which the
  program was never valid.
* Neutral, because the flag count grows by one. The 2026-09-13 amendment
  accepted this cost for the same reason.

## Pros and Cons of the Options

### Gate it behind `--allow-cross-family-widening`

* Good, because no new flag and no new documentation entry.
* Bad, because the flag's documented rule is "bit string to a strictly wider
  integer", which this is not: the result keeps the bit-string type.
* Bad, because it re-creates the situation the amendment to ADR-0031 undid.

### A new flag

* Good, because one flag, one rule.
* Good, because the dialect presets absorb the cost for the users who need it.
* Bad, because one more flag to document and to keep in the dialect tables.

### Accept it in every dialect

* Good, because no program that compiles today stops compiling.
* Bad, because the strict dialects would accept an expression the standard
  does not define, which ADR-0036 and ADR-0040 exist to prevent.
* Bad, because the resolver would then have a non-standard rule with no way
  to turn it off.

## More Information

The change that implements the design landed the flag and flipped this ADR to
`accepted`. The design is
[Arithmetic Operator Overloads](../design/arithmetic-operator-overloads.md);
REQ-AO-analyzer-010 and REQ-AO-analyzer-011 are its requirements.

### Postscript, 2026-09-23

Planning the implementation settled two points this ADR left implicit. The
decision holds.

* The rule applies to `ADD`, `SUB`, `MUL` and `DIV`, the four this ADR names,
  and not to `MOD`. The function form `MOD(b, 2)` is checked against its
  `ANY_INT` signature, which the flag does not change, so admitting `b MOD 2`
  would split the two spellings of one operator. `b MOD 2` is rejected in
  every dialect today, so nothing regresses.
* A bit string judged as the unsigned integer of its width widens only as
  that unsigned integer. `w + i` with `w : WORD` and `i : INT` is therefore
  rejected even with the flag on, because `UINT` and `INT` do not widen to
  each other; `b + i` with `b : BYTE` is accepted because `USINT` widens to
  `INT`. Admitting `w + i` would be a widening rule, which is the business of
  the cross-family flags, not of this one.
