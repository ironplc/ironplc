# Compound duration literals

Issue: #1814

## Goal

`T#1m30s`, `T#1h2m3s4ms`, `T#1d_2h` and every other duration with more than
one unit parse (REQ-TL-021, REQ-TL-022), instead of P0002.

## Architecture

`specs/design/time-literals.md` (Future Work) attributes the gap to the
parser's ordered choice. There are two causes:

1. **Lexing.** `1m30s` lexes as `Digits(1)` and `Identifier(m30s)`: an
   identifier may contain digits and `_`, so every unit after the first is
   glued to the number that follows it, and `dt_sep("m")` never matches.
2. **Grammar.** `days()` and the other unit rules try the scalar form
   (`fixed_point d`) first; once it matches, PEG does not come back for the
   compound form, and the literal fails on the leftover tokens.

Fix:

- A token transform, `xform_split_duration_units`, runs after lexing: inside
  a duration literal (a `T`, `TIME` or `LTIME` prefix, `#`, an optional `-`,
  then the adjacent tokens), an identifier such as `m30s` or `h_30m` is split
  into its letter, `_` and digit runs (`m`, `30`, `s`), with their own spans.
- `interval()` becomes one rule: one or more `number unit` parts, optionally
  separated by `_`, checked in its action: units strictly descending
  (`d` > `h` > `m` > `s` > `ms`), and only the last part may be fixed-point.
  A violation is a syntax error at the literal, like any other malformed
  literal.

## Prefactoring

The five unit rules (`days` … `milliseconds`) each repeat the scalar and the
compound form. They collapse into the single `interval()` rule above, which
is the prefactor and the fix at once; no separate reshaping is needed first.

## Design doc reference

`specs/design/time-literals.md`: REQ-TL-021 and REQ-TL-022 lose their
"not currently implemented" note, Future Work and the implementation section
are updated, and the ignored conformance tests are enabled.

## File map

- `compiler/parser/src/xform_split_duration_units.rs` (new), `lib.rs`
- `compiler/parser/src/parser.rs` (duration grammar)
- `compiler/parser/src/tests/duration.rs` (enable REQ-TL-021/022, add cases)
- `compiler/plc2plc` round trip for a compound literal
- `specs/design/time-literals.md`

## Tasks

- [ ] Tests: the literals in #1814, fixed-point last part, skipped units
      (`T#1d30m`), and rejections (ascending units, fixed-point before the
      last part, a repeated unit)
- [ ] Token transform with its own tests
- [ ] Grammar
- [ ] Design doc
- [ ] `cd compiler && just`
- [ ] Delete this plan
