# Plan: Assert the LSP semantic token tags (#1710)

## Goal

Replace `from_lsp_token_type_for_semantic_token`, which enumerates ~130
`TokenType` variants and asserts nothing, with a test that runs real IEC
61131-3 source through the tokenizer and checks the semantic token tag that
every emitted token carries. Fix the one mis-mapping the new test exposes
(`STRING` keyword tagged as a string literal; string literals untagged).

## Architecture

`LspProject::tokenize` lexes a document with `tokenize_program`, maps each
`Token` to an `Option<SemanticToken>` via `From<LspTokenType>`, and delta
encodes the survivors with `to_deltas`. The new test drives exactly that
pipeline (lexer -> mapping -> deltas) over one source snippet that contains
every `TokenType`, decodes the deltas back to source lexemes, and compares the
`(lexeme, legend name)` stream against an explicit expected list. Tokens the
mapping drops (punctuation, numbers, trivia) are checked to be present in the
lexer output so their absence from the LSP output is a real assertion.

## Prefactoring

`lsp_project.rs` is 1853 lines, well past the 1000-line module limit, and the
semantic token mapping is a self-contained concern inside it. Move the legend,
the index constants, `LspTokenType`, `to_deltas` and the vacuous test into a
new `semantic_tokens.rs` module behind one `to_semantic_tokens` entry point,
and move the delta-decoding test helper into `test_helpers.rs` so the new test
and the existing position tests share it.

## File Map

| File | Change |
|------|--------|
| `compiler/ironplc-cli/src/semantic_tokens.rs` | New: legend, mapping, delta encoding, tests |
| `compiler/ironplc-cli/src/lsp_project.rs` | Call `to_semantic_tokens`; drop moved code and test |
| `compiler/ironplc-cli/src/lsp.rs` | Import `TOKEN_TYPE_LEGEND` from the new module |
| `compiler/ironplc-cli/src/test_helpers.rs` | Shared delta-decoding helper |
| `compiler/ironplc-cli/src/lib.rs` | Register the module |

## Tasks

- [ ] Write plan
- [ ] Prefactor: extract `semantic_tokens.rs` and the shared test helper
- [ ] Fix `STRING` keyword / string literal tag mapping
- [ ] Replace the vacuous test with the all-tokens tagging test
- [ ] Delete the plan, run `cd compiler && just`
