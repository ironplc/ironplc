# Plan: Reject STRING/WSTRING literals whose characters the type cannot hold

Fixes #1733.

## Goal

A `STRING` literal containing a character above U+00FF, or a `WSTRING`
literal containing a character above U+FFFF, is a compile-time error (P4052)
reported by the analyzer, so `check`, `compile` and the language server all
point at the literal and say how to fix it. Today codegen narrows each
character with `ch as u8` / `ch as u16`, so `'等'` silently becomes `'I'`.

## Architecture

ADR-0016 fixes `STRING` as Latin-1 and `WSTRING` as UCS-2 within the Basic
Multilingual Plane. The change enforces that decision for literals with one
analyzer rule, `rule_string_literal_char_range`, which overrides
`visit_character_string_literal` and emits one P4052 per literal at the
literal's span. Codegen stays infallible; the rule guarantees its
precondition. `$` escapes are not decoded by the parser (#1587), but escapes
are ASCII, so checking raw source characters is correct now and after
decoding lands.

## Prefactoring

String literals in declarations (`x : STRING[10] := '…'` and
`TYPE T : STRING[5] := '…'`) are stored as bare `Vec<char>` / `String` with
no span, so a rule could neither reach them through the visitor nor label
them. The prefactor stores them as `CharacterStringLiteral`, gives every
literal a span in the parser, and removes the `#[recurse(ignore)]` so the
derive visits them. Consumers (codegen, plc2plc, MCP) read `lit.value`.
Behaviour is unchanged; existing tests pass without edits.

## Design doc reference

- ADR-0016 (amended with the rejection consequence)
- ADR-0036, ADR-0039, ADR-0005 (why an error rather than a warning or a
  silent substitution)

## File map

| File | Change |
|------|--------|
| `compiler/dsl/src/common.rs` | `StringInitializer.initial_value` and `StringDeclaration.init` become `Option<CharacterStringLiteral>` |
| `compiler/parser/src/parser.rs` | literal rules return a spanned `CharacterStringLiteral` |
| `compiler/codegen/src/compile_setup.rs` | read `lit.value` |
| `compiler/plc2plc/src/renderer.rs` | delegate to `visit_character_string_literal` |
| `compiler/mcp/src/tools/pou_scope.rs` | read `lit.value` |
| `compiler/problems/resources/problem-codes.csv` | add P4052 |
| `compiler/analyzer/src/rule_string_literal_char_range.rs` | new rule |
| `compiler/analyzer/src/lib.rs`, `stages.rs` | register |
| `compiler/codegen/src/compile.rs` | doc comment on `encode_string_literal` |
| `docs/reference/compiler/problems/P4052.rst` | problem page |
| `specs/adrs/0016-string-encoding.md` | amendment |

## Tasks

- [ ] Write plan
- [ ] Prefactor: spanned `CharacterStringLiteral` in declarations
- [ ] Add P4052 and the analyzer rule with tests
- [ ] Docs page and ADR amendment
- [ ] Run full CI pipeline (`cd compiler && just`), docs and specs checks
- [ ] Delete plan
