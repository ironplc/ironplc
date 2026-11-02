# Constant Display as IEC 61131-3 Source Text

Fixes [#1939](https://github.com/ironplc/ironplc/issues/1939): the `pou_scope`
MCP tool shows a temporal or bit-string initial value as Rust `Debug` text,
span included.

## Goal

`Display` for `ConstantKind`, and for every literal type it holds, writes the
IEC 61131-3 spelling of the literal — the text `plc2plc` writes for it — and
never `{:?}` text. `TOD#10:00:00.25` displays as `TIME_OF_DAY#10:00:00.25`,
`T#1s` as `TIME#1000ms`, never as `TimeOfDayLiteral { value: …, span: … }`.

The same defect sits in `ActionQualifier`'s `Display` (`dsl/src/sfc.rs`), which
`plc2plc` uses for SFC action associations: `a1(SD, T#1s);` echoes as
`a1 ( SD(Duration(DurationLiteral { span: SourceSpan { … } … })) );`, which is
not even parseable. It is fixed in the same change.

## Architecture

The literal types already implement `Display`, but `ConstantKind` does not
delegate to them for five of its nine variants, and several of those `Display`
impls disagree with what `plc2plc` writes:

| Literal | `Display` today | `plc2plc` today |
|---|---|---|
| `RealLiteral` `2.0` | `2` (not a real literal) | `2.0` |
| `CharacterStringLiteral` `'a$'b'` | `'a'b'` (unescaped) | `'a$'b'` |
| `DurationLiteral` `LTIME#1s` | `TIME#1000ms` (wrong type) | `LTIME#1000ms` |
| `TimeOfDayLiteral` `LTOD#…` | `TIME_OF_DAY#…` (wrong type) | `LTIME_OF_DAY#…` |
| `DateLiteral` `LDATE#…`, year < 1000 | `DATE#…`, year unpadded | `LDATE#…`, `{year:0>4}` |
| `DateAndTimeLiteral` `LDT#…` | `DATE_AND_TIME#…` | `LDATE_AND_TIME#…` |
| `BitStringLiteral`, `IntegerLiteral` | same as `plc2plc` | — |

The change makes each literal's `Display` the one place its spelling lives:

1. Each literal `Display` in `dsl` writes what `plc2plc` writes today
   (`type_name()` prefix for the temporal literals, `string_escape::encode` for
   strings, a decimal point for whole reals, a zero-padded year for dates).
2. `ConstantKind`'s `Display` delegates every variant to its literal's
   `Display`. `Boolean` keeps `TRUE`/`FALSE`; `plc2plc` keeps writing
   `BOOL#TRUE`, which is its own choice of spelling and not a duplicate.
3. `plc2plc`'s `visit_real_literal`, `visit_character_string_literal`, the
   temporal and bit-string visitors, and its `character_string_text` helper
   write `node.to_string()` instead of formatting the literal themselves, so
   the spelling is no longer duplicated between the two crates. The
   round-trip tests prove the output is unchanged.
4. `ActionQualifier`'s `Display` writes the keyword the parser's
   `action_qualifier` rule accepts (`N`, `R`, `S`, `L`, `D`, `P`, and `P1`/`P0`
   for the `PR`/`PF` variants), followed for a timed qualifier by `, ` and the
   action time: `SD, TIME#1000ms` through `DurationLiteral`'s `Display`, or
   `SD, t_var` through the `Id`'s.
5. `render_initial_value` in `mcp/src/tools/pou_scope.rs` needs no change: it
   already calls `ConstantKind`'s `Display`.

`ConstantKind` has no enumerated-value or reference variant; `pou_scope`
already renders those with `EnumeratedValue`'s `Display` and the `NULL`/`REF`
strings, and they are covered by its existing tests.

### Audit of other user-visible `{:?}`

Non-test format strings with `:?}` in `mcp`, `ironplc-cli`, `sources`,
`parser`, `analyzer`, `dsl`, `cli-support` and `vm-cli`:

- `dsl/src/common.rs` `ConstantKind` — this change.
- `dsl/src/sfc.rs` `ActionQualifier` — this change (item 4).
- `dsl/src/sfc.rs` `TimedQualifier` — unit variants only, `Debug` text equals
  the keyword; left as is.
- `sources/src/parsers/mod.rs` `Unsupported file type: {file_type:?}` — only
  reached for `FileType::Unknown`, prints `Unknown`; left as is.
- `sources/src/project.rs`, `cli-support/src/logger.rs`,
  `analyzer/src/xform_toposort_declarations.rs`, `parser/src/token.rs`,
  `vm-cli/build.rs`, `ironplc-cli/src/lsp.rs` (a quoted string) — log,
  build-script or debugging output, not literal values; left as is.

### Out of scope

- `plc2plc` drops the type prefix of a typed integer literal (`INT#5` echoes
  as `5`). `IntegerLiteral`'s `Display` keeps the prefix; `plc2plc` keeps its
  current output. This changes meaning (a rejected `r := INT#100;` into a
  `SINT` echoes as an accepted `r := 100 ;`) and is reported as
  [#1974](https://github.com/ironplc/ironplc/issues/1974).
- A bit string keeps no radix in the AST, so `BYTE#16#FF` displays as
  `BYTE#255`, as `plc2plc` writes it.

## Prefactoring

None. The only behaviour-preserving step available would be moving
`plc2plc`'s spellings into new `dsl` functions next to the `Display` impls,
which the core change would then fold into those same `Display` impls and
delete. Doing it in one step keeps one spelling per literal throughout.
`dsl/src/common.rs` is already over 1000 lines; the change edits existing
`Display` bodies there and adds no new logic to it. The new tests go in a new
test-only module, `compiler/dsl/src/constant_display_tests.rs`.

## Design doc reference

`specs/design/mcp-server.md` REQ-TOL-mcp-220 (the `pou_scope` `initial_value`
is a display rendering). The requirement text is tightened to say the
rendering is the literal's IEC 61131-3 spelling for a constant.

## File map

- `compiler/dsl/src/common.rs` — `ConstantKind`, `RealLiteral`,
  `CharacterStringLiteral` `Display`.
- `compiler/dsl/src/time.rs` — temporal literal `Display`.
- `compiler/dsl/src/sfc.rs` — `ActionQualifier` `Display`.
- `compiler/dsl/src/constant_display_tests.rs` (new) — `Display` tests per
  kind.
- `compiler/plc2plc/src/renderer.rs` — literal visitors use `Display`;
  `character_string_text` removed.
- `compiler/plc2plc/` tests — SFC timed-qualifier round trip.
- `compiler/mcp/src/tools/pou_scope.rs` — test for temporal, bit-string,
  real and escaped string initial values.
- `specs/design/mcp-server.md` — REQ-TOL-mcp-220 wording.

## Tasks

Core change PR (`fix/constant-display-text`, from `main`):

- [ ] Failing tests: `Display` of each `ConstantKind` variant (untyped and
      typed integer, negative integer, whole and fractional real with and
      without `REAL#`/`LREAL#`, `TRUE`/`FALSE`, `STRING` and `WSTRING` with
      `$'`, `$"`, `$$` and a control character, `TIME`/`LTIME`, `TOD`/`LTOD`
      with a fraction, `DATE`/`LDATE` with a year below 1000, `DT`/`LDT`,
      `BYTE#16#FF` and an untyped bit string).
- [ ] Failing test: `pou_scope` on the program of #1939 returns
      `TIME_OF_DAY#10:00:00.25` and `TIME#1000ms`.
- [ ] Failing test: `plc2plc` round trip of `a1(SD, T#1s);` and
      `a1(P1, t_var);`.
- [ ] Make the literal `Display` impls write the `plc2plc` spelling and
      `ConstantKind` delegate to them; fix `ActionQualifier`.
- [ ] Switch `plc2plc`'s literal visitors to `Display`; remove
      `character_string_text`.
- [ ] Update REQ-TOL-mcp-220.
- [ ] `cd compiler && just`, `cd specs && just`.
