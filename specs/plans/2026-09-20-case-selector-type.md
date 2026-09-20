# Diagnose a CASE selector that is not an integer or enumeration

## Goal

Reject a `CASE` statement whose selector is not an integer or an enumeration
during analysis, so that `ironplcc check` and the editor report it, instead of
`ironplcc compile` failing with a P9999 that has no source location.

This is the `REAL` selector part of
[#1734](https://github.com/ironplc/ironplc/issues/1734):

```st
PROGRAM main
VAR
    level : REAL;
    alarm : BOOL;
END_VAR
    CASE level OF
        1: alarm := TRUE;
    END_CASE;
END_PROGRAM
```

`check` accepts this program. `compile` reaches the `_ => Diagnostic::todo()`
arm in `compile_case_selector` when the selector's operand width is a float,
and reports P9999 at `:1:1`. The other two parts of the issue, spans on every
`Diagnostic::todo()` site and whether `check` should run codegen, are not in
this plan.

## Architecture

### Where the check belongs

IEC 61131-3 defines the `CASE` selector as an expression that "shall evaluate
to a variable of type `ANY_INT` or enumerated data type". The grammar is
`CASE expression OF`, so any expression is syntactically legal and the parser
is right to accept `CASE level OF`. The parser already enforces the syntactic
half of the rule, by restricting a label to a signed integer, a subrange, an
enumerated value, or (behind a flag) a bit-string literal. The type of the
selector is only known once `xform_resolve_expr_types` has filled in
`resolved_type`, so the check is a semantic rule, like the subscript check
proposed in #1470.

### The rule

A new rule, `rule_case_selector_type`, visits every `Case`, looks up the
selector's `resolved_type` in the `TypeEnvironment`, and accepts a
representation that is:

- a signed or unsigned integer (`Int`, `UInt`),
- a subrange (whose base type is an integer by construction), or
- an enumeration.

Anything else with a known representation is reported with a new problem code
against the selector expression's span, naming the selector's actual type.
That includes `REAL` and `LREAL`, `BOOL`, the bit strings `BYTE`, `WORD`,
`DWORD` and `LWORD`, strings, times, dates, and aggregates.

**Bit strings are rejected**, although codegen compiles a `WORD` selector
today. The standard allows only `ANY_INT` and enumerations, and a program
that compiled but should not have is a program the rule exists to catch.
The bit-string *label* extension (`--allow-bit-string-case-labels`) is
unaffected: it pairs a radix-prefixed label with an integer selector.

A selector whose resolved type is missing, or is a generic category that is
not in the type environment (a bare literal, `CASE 1 OF`, resolves to
`ANY_INT`), is skipped rather than reported, as the other type rules do.

### Codegen afterwards

Once the rule stands in front of it, a float-width selector in codegen is an
invariant violation, not a missing capability. The single remaining
float arm (see prefactoring) becomes `Diagnostic::internal_error_at` with the
selector's span.

## Prefactoring

### Codegen: one width decision for CASE labels

`compile_case_selector` in `compiler/codegen/src/compile_stmt.rs` repeats the
same shape three times, for integer labels, subrange labels, and bit-string
labels: compile the selector, then match the width with a `W32` branch, a
`W64` branch, and a `_ => Diagnostic::todo()` branch. Those three `_` arms are
the P9999 site the issue reports, and the new behaviour would otherwise touch
all three.

Collapse them:

- A `CaseLabelValue` names how a label's value narrows to the selector's
  width. A decimal literal must fit the signed range of the width (the
  existing `signed_integer_to_i32` / `_i64`). A radix-prefixed literal is a
  bit pattern that must fit the unsigned range (the existing `u32` / `u64`
  narrowing, moved from the bit-string arm). No narrowing behaviour changes.
- One helper loads the selector, loads the label value as a constant at the
  selector's width, and emits one comparison at that width. The integer
  label arm and the bit-string arm become "value, then helper with `EQ`".
  The subrange arm becomes "helper with `GE`, helper with `LE`, `BOOL_AND`".
- The float rejection lives in that helper alone, with the selector's span,
  so this prefactor also fixes the missing span for this site.

Existing coverage: `compiler/codegen/tests/it/compile_case.rs` (bytecode
shape), `compiler/codegen/tests/it/end_to_end_case.rs` (integer, subrange,
ELSE, hex and binary labels), and `end_to_end_enum.rs` (enumeration
selectors). No new tests are needed for a behaviour-preserving change.

`compile_stmt.rs` is already over the 1000-line module limit. This prefactor
shrinks it but does not bring it under; splitting the module is not part of
this change.

### Analyzer: share the expression-to-representation lookup

The new rule needs the `IntermediateType` behind an expression's
`resolved_type`. `rule_constant_range::expr_type` already does exactly that
lookup, privately. Hoist it to `TypeEnvironment` so both rules ask the same
question the same way. No behaviour change.

## Design doc reference

- [Expression Type Resolution](../design/expression-type-resolution.md):
  the `resolved_type` the rule reads, and why it is filled in by the analyzer
  and not the parser.

## File map

Modified:

- `compiler/codegen/src/compile_stmt.rs`: collapse `compile_case_selector`
  (prefactor); later, the float arm becomes an internal error
- `compiler/analyzer/src/type_environment.rs`: expression representation
  lookup (prefactor)
- `compiler/analyzer/src/rule_constant_range.rs`: use the shared lookup
  (prefactor)
- `compiler/analyzer/src/lib.rs`, `compiler/analyzer/src/stages.rs`:
  register the new rule
- `compiler/problems/resources/problem-codes.csv`: the new code
- `docs/reference/language/structured-text/case.rst`: the selector is an
  integer or enumeration expression, not only an integer; link the new code

Created:

- `compiler/analyzer/src/rule_case_selector_type.rs`: the rule and its tests
- `docs/reference/compiler/problems/P4052.rst`: the problem page

## Tasks

- [ ] Prefactor codegen: collapse `compile_case_selector` onto one label
      comparison helper; float rejection in one place with the selector span
- [ ] Prefactor analyzer: hoist the expression-to-representation lookup onto
      `TypeEnvironment`; `rule_constant_range` uses it
- [ ] Add problem code P4052
- [ ] Add `rule_case_selector_type` and register it; tests for each accepted
      representation (integer, unsigned integer, subrange, enumeration, bare
      literal skipped) and each rejected family (`REAL`, `LREAL`, `BOOL`,
      `WORD`, `STRING`, `TIME`)
- [ ] Codegen: the float arm becomes `internal_error_at` with the selector
      span
- [ ] Write `P4052.rst`; correct `case.rst`
- [ ] `git rm` this plan
- [ ] Full CI (`cd compiler && just`)
