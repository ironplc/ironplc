# Fix LSP semantic token encoding for syntax highlighting

## Problem

A user reported that syntax highlighting in the IronPLC VS Code extension shows
incorrect colors. Symptoms include:

- The same token (`BOOL`) on different lines renders in different colors.
- `VAR` and `END_VAR` render in different colors despite both being `Var`-style
  keywords with the same semantic token type.
- Block comments (`(* … *)`) appear to "break", with parts of the comment text
  rendered as if they were code.
- Plain identifiers like `PLC` appear in two different colors.

## Root cause

The issues are caused by two bugs in the semantic-token pipeline that feeds the
VS Code editor.

### Bug 1: LSP semantic tokens are not delta-encoded

`compiler/ironplc-cli/src/lsp_project.rs` builds `SemanticToken` values by
copying the lexer's absolute `line` and `col` directly into `delta_line` and
`delta_start`:

```rust
token_type.map(|token_type| SemanticToken {
    delta_line: val.0.line as u32,
    delta_start: val.0.col as u32,
    length: val.0.text.len() as u32,
    ...
})
```

The LSP semantic-tokens spec requires *relative* encoding:

- `deltaLine` is the line number *relative to the previous token*.
- `deltaStart` is the column relative to the previous token's start when
  `deltaLine == 0`, and absolute otherwise.

Because each token's absolute coordinates are interpreted as deltas, every
token after the first lands at the wrong line/column on the editor side. The
displaced tokens then "color" arbitrary spans of the document — explaining the
mismatched colors on identical keywords, the partial comment highlighting, and
the split coloring of single identifiers.

### Bug 2: Lexer does not advance `col` through comment characters

`compiler/parser/src/lexer.rs` walks the characters of a comment token to
advance line/column counters, but uses `col += 0` for every non-newline
character:

```rust
TokenType::Comment => {
    for c in lexer.slice().chars() {
        match c {
            '\n' => { line += 1; col = 0; }
            _ => { col += 0; }     // never advances
        }
    }
}
```

After a single-line comment, `col` is left at the comment's start column
instead of moving past the comment text. Subsequent tokens on the same line
report a column that is too small, which compounds the delta-encoding bug.

## Fix

1. **Delta-encode semantic tokens.** Sort/iterate tokens in source order and,
   for each token, compute `delta_line = line - prev_line` and
   `delta_start = col - prev_col` when on the same line, otherwise
   `delta_start = col`. The lexer already emits tokens in source order, so we
   only need a small running-state fold over the filtered sequence.

2. **Advance `col` through comment characters.** Change `col += 0` to
   `col += 1` so that the column counter tracks each non-newline character of
   the comment.

## Tests

- Unit test in `lsp_project.rs` that constructs a small program with multiple
  tokens on different lines, calls `tokenize`, and asserts the resulting
  `SemanticToken` deltas reconstruct the original absolute positions (i.e. the
  cumulative sum of `delta_line` matches each token's source line, and the
  cumulative `delta_start` resets across line breaks).
- Unit test in `lexer.rs` confirming that a token following a single-line block
  comment on the same line is reported with the correct absolute column.

## Out of scope

- Changes to the TextMate grammar (`integrations/vscode/syntaxes/`). The
  grammar tokenization is verified by snapshot tests and produces consistent
  scopes for the user's reported tokens; the user-visible inconsistency is
  driven by the semantic-token overlay, not the grammar.
