# Decode `$` escapes in character string literals

Issue: #1836. Stacked on the branch for #1818 (the lexer must accept `$'`
and `$"` first).

## Goal

A character string literal holds the characters it denotes, not its source
spelling: `'$41$$'` is the two characters `A$`, so `LEN('$41$$')` is 2 and
the stored string is `A$`. An escape the standard does not define is a
diagnostic instead of being stored as text.

## Architecture

- **One escape table, in `dsl`** (`dsl/src/string_escape.rs`):
  `decode(text, width) -> Result<Vec<char>, EscapeError>` and
  `encode(chars, width) -> String`. `dsl` is the crate both the parser and
  every renderer already depend on, so there is one table (#1587 asks for
  this once decoding exists).
- **Escapes** (IEC 61131-3 ed. 2, B.1.2.2): `$$`, `$L`, `$N`, `$P`, `$R`,
  `$T` (either case) in both widths; `$'` and `$hh` in a single-byte string;
  `$"` and `$hhhh` in a double-byte string. `$N` decodes to a line feed
  (U+000A), the same character as `$L`; the standard leaves the newline
  character to the implementation. The other delimiter is accepted escaped
  too (`$"` in a single-byte string, `$'` in a double-byte one) and decodes to
  itself, since several toolchains accept it and nothing is ambiguous.
- **Invalid escapes**: a token rule (`rule_token_string_escape`, the same
  shape as `rule_token_no_c_style_comment`) reports a new parse problem,
  P0012 `InvalidStringEscape`, at the escape: `$` followed by any other
  character, or too few hex digits for the width. The parser then decodes
  leniently (an invalid escape is kept as written) so that it never fails.
- **Parser**: `unquote` becomes `decode` for both string rules.
- **plc2plc**: renders with `encode`: `$` → `$$`, the delimiter → `$'`/`$"`,
  LF/CR/FF/TAB → `$L`/`$R`/`$P`/`$T`, other control characters → `$hh` or
  `$hhhh`; every other character, including non-ASCII, as itself. The comment
  and tests that pin "passes through verbatim" change: that was right only
  while `value` held source text.
- **Other readers of `value`**: `ConstantKind`'s `Display` and the MCP
  `pou_scope` tool render with `encode`; `rule_string_literal_char_range`'s
  comment is updated (decoded characters are still within range); codegen
  already encodes the characters it is given.

## Prefactoring

None needed: `unquote` is the only place the parser builds the value, and
the renderer's `character_string_text` is the only place it is written back.
Both are replaced, not reshaped.

## Design doc reference

ADR-0016 (string encoding). Add requirement IDs for escapes to a new
`specs/design/string-literals.md` (REQ-SL-*), with conformance tests.

## File map

- `compiler/dsl/src/string_escape.rs` (new), `compiler/dsl/src/lib.rs`,
  `compiler/dsl/src/common.rs` (`Display`)
- `compiler/parser/src/parser.rs`, `compiler/parser/src/rule_token_string_escape.rs`
  (new), `compiler/parser/src/lib.rs`
- `compiler/plc2plc/src/renderer.rs`, `compiler/plc2plc/src/tests/string_literals.rs`
- `compiler/mcp/src/tools/pou_scope.rs`
- `compiler/analyzer/src/rule_string_literal_char_range.rs` (comment)
- `compiler/problems/resources/problem-codes.csv`,
  `docs/reference/compiler/problems/P0012.rst` (new)
- `compiler/codegen/tests/it/end_to_end_string_escapes.rs` (new)
- `specs/design/string-literals.md` (new)
- `docs/reference/language/data-types/elementary/string.rst`, `wstring.rst`

## Tasks

- [ ] Tests for `decode`/`encode` (every escape, both widths, invalid cases,
      round trip)
- [ ] `string_escape.rs`
- [ ] Token rule, P0012 and its documentation
- [ ] Parser decodes; parser tests on the literal value
- [ ] plc2plc encodes; round-trip tests for every escape
- [ ] Display, MCP, char-range comment
- [ ] End-to-end: `LEN('$41$$') = 2`, `'$L'` is one character, `$'` in the
      value, WSTRING `"$00E9"`
- [ ] Design doc with REQ-SL-* and conformance tests
- [ ] Reference docs list the escapes
- [ ] `cd compiler && just`, `cd docs && just`
- [ ] Delete this plan
