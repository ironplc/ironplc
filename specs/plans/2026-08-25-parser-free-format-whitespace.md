# Free-format whitespace in variable references and qualified paths

## Context

[Issue #1437](https://github.com/ironplc/ironplc/issues/1437) reports that the
parser rejects legal free-format ST. IEC 61131-3 permits whitespace between
tokens, but several rules in `compiler/parser/src/parser.rs` juxtapose tokens
with no optional-whitespace rule (`_`) between them, so these all fail with
**P0002** on `main`:

| Source | On `main` |
|---|---|
| `v := s . x;` | P0002 |
| `v := refs [0];` | P0002 |
| `refs [0] := 5;` | P0002 |
| `v := myRef ^;` | P0002 |
| `THIS^ .count := 1;` | P0002 |
| `THIS ^ . count := 1;` | P0002 |
| `VAR_CONFIG Some . Located . Item.Path AT %QB1: BYTE;` | P0002 |

The gap is not one rule. It spans the whole variable-element chain and every
dotted qualified path. `self_ref` already accepts the space and states the
principle in its own comment — "nothing in the grammar joins them into one
token" — which is why the results split the way they do: `THIS ^.count` parses
today, `THIS^ .count` does not.

The [test machinery](2026-08-24-whitespace-invariance-tests.md) landed
separately (#1441), so each row above costs one `#[case]` line here rather than
a hand-written test.

## Approach

### A. The variable element chain

`symbolic_variable` chained its elements — `.field`, `.bit`, `.%Xn`, `[i]`, `^`
— with no `_`, on one 1,100-character line. Extract the alternatives into
`symbolic_variable_element()` and have the chain read

```
head:symbolic_variable_head() elements:(_ e:symbolic_variable_element() { e })*
```

so every element may be preceded by whitespace. Inside each alternative the
`Period` and what follows it are separate tokens too (`. x`, `. 0`, `. %X0`),
so `_` goes there as well.

The deref alternative carries a lookahead — `tok(Caret) &(LeftBracket /
Period)` — which is what keeps a *trailing* caret out of the chain and in
`unary_expression`, where it becomes `ExprKind::Deref`. Widening means the
lookahead must skip whitespace too: `&(_ (LeftBracket / Period))`. The choice
of which rule owns the caret is unchanged; only the spelling it tolerates is.

`unary_expression` gets the matching change: `carets:(_ c:tok(Caret) { c })*`.

### B. Dotted qualified paths

Insert `_` either side of the `Period` in `global_var_reference`,
`instance_specific_init__located`, `instance_specific_init__fb_init`,
`structured_variable`, and `access_path` (which the issue does not list but is
the same shape, and reaches `symbolic_variable` through a trailing period).
`periodsep_oneplus_no_trailing` spelled its separator as bare `period()` while
its two siblings already used `(_ period() _)`; make the three agree.

### C. Not touched

The mechanical scan in the issue turns up 29 adjacent-token pairs. The ones
that sit *inside* a single lexical unit must stay rejected: typed-literal `#`
prefixes (`INT#16`, `T#100ms`, `COLOR#RED`), date/time internals
(`2024-01-20`, `14:30:20`), and `signed_integer__negative`. `REF=` is likewise
one token, so `REF =` stays a parse error. All of these are already asserted —
the `parse_when_gap_filled_then_rejected` table and
`reference_to.rs:127` are the tripwires, and they must keep passing unchanged.

## Risk

`_` matches newlines, so a greedy element could in principle absorb a `[` or
`.` that begins the next construct. Statements are semicolon-terminated and
`rust-peg` backtracks a failed iteration, so the reasoning says this is safe —
but the workspace suite is the actual check, along with the rejection table
above and the plc2plc golden fixtures. **No golden fixture should move.** The
renderer already emits the tight spelling, which stays canonical whatever the
parser tolerates; a fixture that moves is the signal that the change was not
purely additive.

## Tests

Fourteen `#[case]` rows added to `parse_when_gap_filled_then_same_ast`, one per
construct, each asserting every variant parses to the *same AST* as the tight
spelling: structured field, subscript read and assign, trailing deref, deref
followed by field and by subscript, bit access, partial access, a mixed chain,
`THIS^`/`SUPER^` chains, `VAR_ACCESS` paths, and both `VAR_CONFIG` forms. A new
`in_var_config` wrapper supplies the `CONFIGURATION`/`RESOURCE` scaffolding the
block requires.

## Documentation

`docs/reference/language/object-orientation/this-and-super.rst:30` already
presents `THIS ^ . member` as accepted. It was wrong on `main`; this change
makes it true, so the file needs no edit.
