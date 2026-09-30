# Plan: One semantic token per line for multi-line lexemes

## Context

[#1712](https://github.com/ironplc/ironplc/issues/1712): the language server
turns each lexer token into exactly one LSP semantic token, with
`length: text.len()`. The LSP specification does not allow a semantic token to
span lines, so a `(* ... *)` comment written over several lines produces one
token whose length runs past the end of its first line. The client drops or
clips it and the comment falls back to the TextMate grammar colouring.

The same code path carries the other lexemes that can contain a line break:

- `/* ... */` comments (when C-style comments are allowed);
- `{ ... }` pragmas, collapsed into one `Pragma` token by
  `xform_collapse_pragmas`;
- character strings: the string regexes accept a raw line break.

Two related position problems sit next to the length:

- The server does not negotiate `positionEncoding`, so the protocol default,
  UTF-16 code units, applies. The token `length` is in bytes, and the lexer's
  `col` is in bytes for most tokens but in `char`s after a comment. Both are
  wrong as soon as a line holds non-ASCII text.
- The lexer only advances `line` for `Newline` tokens and inside comments, so a
  string holding a line break leaves every later token on the wrong line.

## Goals

1. No emitted semantic token spans a line: a lexeme covering several lines is
   emitted as one token per line, each covering that line's part of the
   lexeme, without the line terminator (`\n` or `\r\n`).
2. Token start columns and lengths are in UTF-16 code units, the encoding the
   protocol uses when none is negotiated.
3. The lexer's `line` stays correct after any lexeme that contains a line
   break, not only comments.

## Non-goals

- Negotiating another position encoding with the client.
- Treating a lone `\r` or a form feed as a line break differently from today.

## Design

### Prefactor

None needed: the lexer's position update is already one `match`, and the
semantic token conversion is a single `From` impl feeding `to_deltas`.

### Lexer (`compiler/parser/src/lexer.rs`)

After each token other than `Newline`, walk its text: a `\n` advances `line`
and resets `col`, any other character adds its UTF-16 length to `col`. This
replaces the comment-only walk and the `span().len()` byte count. `Token::col`
is documented as counted in UTF-16 code units.

### Semantic tokens (`compiler/ironplc-cli/src/semantic_tokens.rs`)

`to_semantic_tokens` flat-maps each kept lexer token into one absolute token
per line of its text: the first at the token's `(line, col)`, the others at
column 0 of the following lines. A part's length is its UTF-16 length with a
trailing `\r` removed; empty parts (a blank line inside a comment) are not
emitted. `to_deltas` is unchanged.

## Tests (written first)

- Lexer: a token after a multi-line string sits on the right line; a token
  after non-ASCII text has its column in UTF-16 code units.
- Semantic tokens:
  - a two-line `(* ... *)` comment gives two comment tokens, none running past
    the end of its line (the check suggested in the issue);
  - the same with CRLF line endings: no token covers the `\r`;
  - a multi-line pragma and a multi-line string are split the same way;
  - a comment holding non-ASCII text has a UTF-16 length, and the token after
    it starts at its UTF-16 column.

## Verification

`cd compiler && just`, `cd specs && just`.
