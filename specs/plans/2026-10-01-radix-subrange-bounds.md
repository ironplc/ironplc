# Accept hex, binary and octal subrange bounds

Issue: #1917 (depends on #1922, which checks every CASE label against the
selector's range)

## Goal

A subrange whose bounds are written in hex, binary or octal parses, in each
place the grammar has a subrange:

- a CASE label: `16#01..16#0F:`
- a subrange type or variable: `TYPE R : INT (16#00..16#FF); END_TYPE`,
  `x : INT(2#0..2#1111);`
- an array dimension: `ARRAY[16#0..16#F] OF INT`

Today `subrange()` takes its bounds from `signed_integer_ref()`, which accepts
only a decimal integer or an identifier, so each of these is P0002.

IEC 61131-3 (Annex B) defines `subrange ::= signed_integer '..'
signed_integer`, decimal only, so a radix bound is an extension and stays
gated:

- In a CASE label, by the flag that already gates a radix label,
  `--allow-bit-string-case-labels` (P4041), as the issue asks.
- In a declaration, by a new flag `--allow-radix-subrange-bounds` (P4073),
  enabled by the same dialects (RuSTy, CODESYS, TwinCAT).

A radix bound is unsigned, like a radix CASE label: `-16#10` and a typed
`INT#16#FF` stay outside the grammar, as they are for a single label.

## Architecture

- **Parser grammar**: a `subrange_bound()` rule accepts a decimal signed
  integer, an identifier, or a radix integer, the last as a non-negative
  `SignedIntegerRef::Literal`. The AST does not change, so the analyzer
  (range checks, P2026 from #1922), codegen and plc2plc treat a radix bound
  like a decimal one. plc2plc renders the bound decimal, as it renders every
  radix literal.
- **Gate**: the AST keeps no radix marker, so the gate is a token-stream
  rule (`rule_token_radix_subrange_bound`), like the paren string length
  gate: a radix token next to a `..` token (ignoring trivia) is a radix
  bound. Between `CASE` and `END_CASE` a `..` can only be a label range, so
  the rule tells a label bound from a declaration bound by CASE nesting
  depth.

## Prefactoring

Extract the radix alternatives shared by `case_bit_string_literal()` into a
`radix_integer()` rule, so the new bound rule reuses it.

## Design doc reference

None; the flag and problem pages document the behaviour.

## File map

- `compiler/parser/src/parser.rs` — `radix_integer()`, `subrange_bound()`
- `compiler/parser/src/rule_token_radix_subrange_bound.rs` — the gate (new)
- `compiler/parser/src/lib.rs` — register the gate
- `compiler/parser/src/options.rs`, `options/tests.rs` — the new flag
- `compiler/ironplc-cli/bin/main.rs` — CLI flag
- `compiler/mcp/src/feature_flag_conformance.rs` — flag fixture
- `compiler/problems/resources/problem-codes.csv` — P4073
- `compiler/parser/src/tests/radix_subrange_bound.rs` — AST shape (new)
- `compiler/plc2plc/src/tests/radix_subrange_bound.rs` — round trip (new)
- `compiler/analyzer/src/rule_constant_range/tests.rs` — P2026 on a radix
  bound
- `compiler/codegen/tests/it/end_to_end_radix_subrange_bound.rs` — run (new)
- `docs/reference/compiler/problems/P4041.rst`, `P4073.rst` (new)
- `docs/reference/language/structured-text/case.rst`
- `docs/explanation/enabling-dialects-and-features.rst`,
  `docs/reference/compiler/ironplcc.rst`, `specs/steering/syntax-support-guide.md`

## Tasks

- [ ] Prefactor `radix_integer()`
- [ ] Failing parser tests: radix bounds in a CASE label, subrange type,
      inline subrange variable, array dimension
- [ ] `subrange_bound()` grammar rule
- [ ] Failing gate tests; flag, problem code and token rule
- [ ] Analyzer range check, round trip and end-to-end tests
- [ ] Docs
- [ ] `git rm` this plan; `cd compiler && just`; `cd specs && just`
