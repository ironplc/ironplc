# Real Literal Out of Range

Fixes [#1784](https://github.com/ironplc/ironplc/issues/1784): a real literal
outside the range of its type (`1.0E400`) parses to `inf` and compiles with
no diagnostic.

## Goal

Report a real literal whose value its type cannot represent as a new problem,
P2040 `RealLiteralOutOfRange`, alongside P2038 (dates) and P2039 (durations).

- An untyped or `LREAL#` literal is out of range when it does not fit `f64`.
- A `REAL#` literal is out of range when it does not fit `f32`: the prefix
  states the literal's own type, as `INT#40000` does for integers.

Checking an untyped literal against a `REAL` *destination* is out of scope
(the constant-range rule does not check real types today).

## Architecture

A new analyzer rule, `rule_real_literal_range`, visits every `RealLiteral`
and reports one whose value is not finite in its type. `f64::from_str`
already saturates to `inf` for out-of-range text, so the rule needs only
`is_finite` on the parsed value (narrowed to `f32` for `REAL#`).

## Prefactoring

`RealLiteral::try_parse` sets `SourceSpan::default()` and the parser keeps it,
so a real literal has no source position to report against. Prefactor: pass
the token's span into `try_parse`, as `Integer::new` already takes one.

## File map

- `compiler/dsl/src/common.rs` — `RealLiteral::try_parse` takes a span
- `compiler/parser/src/parser.rs` — pass the literal's span
- `compiler/analyzer/src/rule_real_literal_range.rs` — new rule
- `compiler/analyzer/src/{lib,stages}.rs` — register the rule
- `compiler/problems/resources/problem-codes.csv` — P2040
- `docs/reference/compiler/problems/P2040.rst` — problem page

## Tasks

- [ ] Prefactor: real literal carries its source span
- [ ] Add P2040 and the rule with tests
- [ ] Document P2040
- [ ] Remove this plan; run `cd compiler && just`
