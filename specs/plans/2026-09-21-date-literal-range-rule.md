# Diagnose a date literal the compiler cannot represent

Issue: [#1560](https://github.com/ironplc/ironplc/issues/1560) (problem 1 of two)

## Goal

A `DATE`, `LDATE`, `DT` or `LDT` literal outside the range the compiler stores
a date in is reported as a problem code, instead of panicking the compiler in a
debug build and silently wrapping to a different date in a release build.

`LDATE#2200-01-01` today:

```
thread 'main' panicked at dsl/src/time.rs:217:9:
attempt to multiply with overflow
```

and in release, wraps to 2063-11-24 with no diagnostic at any stage.

## Scope

Issue #1560 separates two problems. This plan delivers **problem 1** only: an
out-of-range date literal becomes a diagnostic.

**Problem 2 — the 2106 ceiling is spurious for `LDATE` and `LDT`** — is not
delivered here and the issue stays open for it. The ceiling this rule enforces
is the one the compiler actually has: `compile_expr.rs` lowers every date
literal through one `seconds_since_epoch` accessor and stores the result as
unsigned 32-bit seconds, for the 64-bit types as much as the 32-bit ones (see
the [ADR-0025 amendment](../adrs/0025-datetime-unsigned-representation.md)).
Until a `u64` lowering path exists, an `LDATE` past 2106 is a date the compiler
cannot emit, and saying so is better than wrapping it. When problem 2 lands,
the rule's ceiling is the one place that has to widen.

## Architecture

**The check is per-literal, not per-destination.** `DATE#` and `LDATE#` both
parse to `ConstantKind::Date`, and `DT#`/`LDT#` both to
`ConstantKind::DateAndTime`; the width of the variable a literal is stored into
does not reach the accessor that overflows. So the rule needs no declaration
tracking and no type environment — it visits every date literal in the library
and asks whether its second count fits the `u32` the compiler stores.

The rule runs in `stages::semantic`, which runs after `resolve_types` and so
after `xform_fold_initializer_expressions` and
`xform_fold_constant_expressions`. A literal that reaches a date only by
folding — a `VAR_GLOBAL CONSTANT` substituted into an initializer expression —
is checked on the folded value, which is the value codegen will lower. Placing
the check in codegen instead would report against an expression the user did
not write; placing it before folding would miss the substituted literal
entirely.

Both ends of the range matter. A date before 1970-01-01 is a negative second
count, which `as u32` turns into a number near `u32::MAX` and the multiply then
overflows, so `DATE#1969-12-31` panics exactly as `DATE#2200-01-01` does.

## Prefactoring

`DateLiteral::seconds_since_epoch` and
`DateAndTimeLiteral::seconds_since_epoch` compute in `u32`, so the value the
rule needs in order to judge the literal is a value that cannot be obtained
without trapping. They move to `i64`, which holds every date the parser accepts
(the `time` crate caps a year at 9999), and `compile_expr.rs` narrows to the
stored `u32` at the point of emission. Release-build bytecode is unchanged: the
narrowing is the same `mod 2^32` the `u32` arithmetic performed.

The two date arms of `compile_constant` are the same eight lines twice, and
both grow the same narrowing, so they fold into one helper first.

## File map

| File | Change |
|---|---|
| `compiler/dsl/src/time.rs` | `seconds_since_epoch` returns `i64` on both literal types |
| `compiler/codegen/src/compile_expr.rs` | one helper for both date arms; narrows to `u32`, reports rather than truncating |
| `compiler/problems/resources/problem-codes.csv` | P2038 `DateLiteralOutOfRange` |
| `compiler/analyzer/src/rule_date_literal_range.rs` | new rule |
| `compiler/analyzer/src/lib.rs` | `mod rule_date_literal_range;` |
| `compiler/analyzer/src/stages.rs` | add to the `semantic` rule list |
| `docs/reference/compiler/problems/P2038.rst` | new problem page |
| `docs/reference/language/data-types/elementary/*date*.rst` | state the range each date type represents |

## Tasks

- [ ] Prefactor: one `compile_constant` helper for `Date` and `DateAndTime`
- [ ] Prefactor: `seconds_since_epoch` computes in `i64`; codegen narrows
- [ ] Add P2038 to the problem codes
- [ ] Add `rule_date_literal_range` and register it in `stages::semantic`
- [ ] Tests: both boundaries of both types, the pre-epoch end, every literal
      position, and that the rule reports every violation rather than the first
- [ ] Document P2038 and the range on the four date type pages
- [ ] `cd compiler && just` and `cd specs && just`
- [ ] `git rm` this plan
