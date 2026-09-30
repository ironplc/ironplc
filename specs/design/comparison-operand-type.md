# Design: Comparison Operand Type

status: implemented
date: 2026-09-30

## Problem

A comparison (`=`, `<>`, `<`, `<=`, `>`, `>=`, and the function forms `EQ`,
`NE`, `LT`, `LE`, `GT`, `GE`) has a `BOOL` result, but its operands are
compared at some operand type, and the bytecode compares at one operation
width and signedness. Codegen used to choose the type of the left operand, so
a wider right operand was truncated first: `DINT 1 < LINT 4294967297` compared
`1 < 1` and gave FALSE while `LINT 4294967297 > DINT 1` gave TRUE
([#1920](https://github.com/ironplc/ironplc/issues/1920)). The function form
compared at the type of the enclosing expression instead, the `BOOL` it is
assigned to, so `GT(l1, l2)` truncated two `LINT` operands to 32 bits.

## Operand type

A comparison is computed at the type one operand widens to, whichever side it
is on. That is the choice the numeric arithmetic overload makes for its result
(see [Arithmetic Operator Overloads](arithmetic-operator-overloads.md)): the
operand the other is acceptable as by the implicit widening of
`type_compat::are_types_compatible` (ADR-0028, ADR-0029, ADR-0031, and the
short-to-long widening of a temporal family from the ADR-0025 amendment). Both
ask `intermediates::common_operand`, so a comparison and an arithmetic
expression on the same pair agree. The analyzer exposes the answer as
`comparison_operand_type`, a pure function of the two operand types.

**REQ-CMP-analyzer-001** The operand type of a comparison of two integer types, one of which widens to the other, is the wider one whichever side it is on: `DINT` and `LINT` compare as `LINT`, as do `LINT` and `DINT`.

**REQ-CMP-analyzer-002** The operand type of a comparison of two bit-string types is the wider one: `DWORD` and `LWORD` compare as `LWORD`.

**REQ-CMP-analyzer-003** The operand type of a comparison of `REAL` and `LREAL` is `LREAL`, and of an integer that widens losslessly to a real and that real is the real: `INT` and `REAL` compare as `REAL`.

**REQ-CMP-analyzer-004** The operand type of a comparison of the short and long types of one temporal family is the long type: `TIME` and `LTIME` compare as `LTIME`, `DATE_AND_TIME` and `LDATE_AND_TIME` as `LDATE_AND_TIME`.

**REQ-CMP-analyzer-005** The operand type of a comparison of an untyped literal and a concrete type it is acceptable as is the concrete type: `1` and `LINT` compare as `LINT`.

**REQ-CMP-analyzer-006** A comparison of two types neither of which widens to the other, such as `DINT` and `UDINT`, has no operand type.

## Codegen

Each operand is compiled at its own type and converted to the operand type,
then the comparison is emitted at the operand type. Converting from the
operand's own type is what widens it by its own signedness: an unsigned
integer, a bit string or a date type (unsigned, ADR-0025) is zero-extended, a
signed integer or a `TIME` (ADR-0021) is sign-extended, and an integer is
converted to a real. An operand whose own type codegen cannot place, a
literal's category or a subrange, is compiled at the operand type directly.

A comparison with no operand type is compiled as it was before this design:
at the concrete left operand's type, else the concrete right operand's. The
analyzer does not check the operand pair of a comparison, so such a pair
reaches codegen (see Out of scope).

The operator and the function form compile through the same routine, so the
two spellings cannot diverge.

**REQ-CMP-codegen-001** A comparison of two integers computes at the wider type whichever side it is on, so `DINT 1 < LINT 4294967297` and `LINT 4294967297 > DINT 1` are both TRUE.

**REQ-CMP-codegen-002** A narrower unsigned operand is zero-extended to the operand type, so `UDINT 4000000000 = ULINT 4000000000` and `DWORD 16#FFFFFFFF = LWORD 16#FFFFFFFF` are TRUE.

**REQ-CMP-codegen-003** A `REAL` compared with an `LREAL` is converted to `LREAL`, so `REAL 1.0 < LREAL 1.00000001` is TRUE.

**REQ-CMP-codegen-004** A short temporal operand is widened to the long type by its signedness, so a `DATE_AND_TIME` compares below an `LDATE_AND_TIME` after 2106, and a negative `TIME` below an `LTIME` of zero.

**REQ-CMP-codegen-005** A call to `EQ`, `NE`, `LT`, `LE`, `GT` or `GE` computes at the operand type of its two inputs, as the operator expression does, whatever the type it is assigned to.

## Out of scope

- The analyzer accepts a comparison of any two elementary operands, including
  a pair with no operand type (`DINT` and `UDINT`, `DINT` and `REAL`), which
  compiles at the left operand's type. Checking the pair is a separate change.
- The ordered comparison functions are binary; the extensible monotonic form
  `GT(a, b, c)` is not implemented (see
  [Keyword Function Forms](keyword-function-forms.md)).
- String comparisons take their own path and are unchanged.
