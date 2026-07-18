# Plan: Skip `{ ... }` Pragmas (TwinCAT/Siemens Attribute Headers)

## Goal

Treat `{ ... }` pragma blocks (e.g. `{attribute 'qualified_only'}`, `{attribute 'strict'}`)
as opaque trivia — parsed and discarded like a comment, with no diagnostic and no AST
node — so that real-world Beckhoff TwinCAT `.TcPOU`/`.TcGVL`/`.TcDUT` files parse past
their (nearly universal) pragma headers.

Context: issue #1199 reports that of 158 real TwinCAT files tested against
`ironplcc check --dialect codesys`, only 17 (~11%) parse clean today, and that
pragma headers are by far the single biggest blocker — they appear on almost
every `Declaration` block emitted by modern TwinCAT project templates and cause
even trivially simple files (e.g. a plain enum) to fail. This is a narrow,
low-risk first slice of the broader Beckhoff TwinCAT dialect work described in
[ADR-0012](../adrs/0012-accept-vendor-dialect-files-as-is.md) and
[specs/design/beckhoff-twincat-dialect.md](../design/beckhoff-twincat-dialect.md).
It does not attempt OOP constructs (`METHOD`, `EXTENDS`, ...), `REFERENCE TO`, or
any other item from that design — those are tracked separately.

## Non-goals

- No interpretation of pragma contents (no `qualified_only`/`strict` semantics).
- No new `Dialect::BeckhoffTwinCAT` variant — reuses the existing `Codesys` and
  `Rusty` dialects, consistent with how other vendor-extension flags are gated
  today.
- No `P90xx` problem code — pragmas produce no AST node, so there is nothing to
  flag as "recognized but unsupported" (unlike the `VendorExtension`/`P9004`
  machinery proposed for OOP constructs in the larger design).

## Current State

- `{` and `}` already lex as `TokenType::LeftBrace`/`RightBrace`
  (`compiler/parser/src/token.rs`) but nothing in the grammar
  (`compiler/parser/src/parser.rs`) ever consumes them, so any `{ ... }` in a
  source file is a parse error today.
- Comments are handled by lexing `(* ... *)` as a single `Comment` token, then
  the PEG grammar's `_` (trivia) rule matches `whitespace() / comment()`
  wherever trivia is allowed (`compiler/parser/src/parser.rs:199-202`).
- Vendor-extension syntax is gated by boolean flags on `CompilerOptions`,
  declared via the `define_compiler_options!` macro in
  `compiler/parser/src/options.rs`, each flag listing which `Dialect` presets
  enable it by default.
- Token-stream transforms that depend on these flags live as `xform_*.rs`
  modules (e.g. `xform_demote_time_keyword.rs`) and run in
  `tokenize_program` (`compiler/parser/src/lib.rs`) between lexing and
  `insert_keyword_statement_terminators`.

## Design

Follow the existing `xform_*` + dialect-flag pattern rather than changing the
lexer grammar itself — this keeps the change reviewable in isolation and
carries zero regression risk for standard IEC 61131-3 parsing (nothing today
depends on `LeftBrace`/`RightBrace` reaching the parser).

1. **New flag**: `allow_pragmas` added to the `define_compiler_options!` table
   in `options.rs`, enabled for `[Rusty, Codesys]` (the same set as most other
   vendor-extension flags). This directly unblocks the `--dialect codesys`
   workflow used in issue #1199.

2. **New token type**: `TokenType::Pragma` added to `token.rs` with **no**
   `#[token]`/`#[regex]` attribute — it is never produced directly by the
   logos lexer, only by the transform below (same approach the existing
   dialect design docs use for tokens that only exist post-transform).

3. **New transform**: `compiler/parser/src/xform_collapse_pragmas.rs`.
   - No-ops entirely when `!options.allow_pragmas`.
   - Otherwise scans the token stream for `LeftBrace`. On a match, scans
     forward for the next `RightBrace` (pragmas do not nest — matches the
     `dialect-token-transforms.md` design). If found, replaces the whole
     `LeftBrace ..= RightBrace` run with a single `Pragma` token whose span is
     `SourceSpan::join(first.span, last.span)` and whose text is the
     concatenation of the collapsed tokens' text (so it reads back as the
     original `{...}` slice).
   - If no matching `RightBrace` is found before EOF, the `LeftBrace` is left
     untouched — it will surface as a parse error, same as today.
   - Wired into `tokenize_program` in `lib.rs`, run before
     `insert_keyword_statement_terminators` (order doesn't interact with it,
     but keeps dialect transforms grouped before the existing keyword-demotion
     transforms per the pipeline shape already used for
     `xform_demote_edition3_keywords`/`xform_demote_time_keyword`).

4. **Parser change**: in `parser.rs`, add
   `rule pragma() -> () = tok(TokenType::Pragma) ()` and extend the trivia rule:
   `rule _ = (whitespace() / comment() / pragma())*`.
   When `allow_pragmas` is off, no `Pragma` tokens are ever produced, so this
   rule addition is inert and standard parsing is unaffected.

This mirrors how comments are already trivia that never appear in the AST —
pragmas are dropped the same way, so there's no plc2plc round-trip concern
beyond what already exists for comments (neither survives render).

## File Map

| File | Change |
|------|--------|
| `compiler/parser/src/options.rs` | Add `allow_pragmas` flag (`[Rusty, Codesys]`); update `FEATURE_DESCRIPTORS` count assertions in tests |
| `compiler/parser/src/token.rs` | Add `TokenType::Pragma` variant + `describe()` arm + display test case |
| `compiler/parser/src/xform_collapse_pragmas.rs` | New transform module (collapse logic + unit tests) |
| `compiler/parser/src/lib.rs` | Register new module; call transform in `tokenize_program` |
| `compiler/parser/src/parser.rs` | Add `pragma()` rule; extend `_` rule |
| `compiler/parser/src/tests.rs` | Parser-level tests: pragma header parses under `Codesys`/`Rusty`, still errors under `Iec61131_3Ed2` |
| `docs/explanation/enabling-dialects-and-features.rst` | Document `--allow-pragmas` |
| `docs/reference/compiler/ironplcc.rst` | Add flag to CLI reference |
| `specs/steering/syntax-support-guide.md` | Add flag to the flag table |

## Tasks

- [x] Write plan
- [ ] Add `allow_pragmas` flag to `options.rs` (+ update existing count/feature-list tests)
- [ ] Add `TokenType::Pragma` to `token.rs`
- [ ] Implement `xform_collapse_pragmas.rs` with unit tests (simple pragma, multi-token
      content, unclosed brace left as-is, no-op when flag disabled)
- [ ] Wire transform into `lib.rs`
- [ ] Extend `parser.rs` trivia rule
- [ ] Add parser-level tests: a minimal enum preceded by
      `{attribute 'qualified_only'}`/`{attribute 'strict'}` parses cleanly under
      `Codesys`; the same source still fails under default (`Iec61131_3Ed2`)
- [ ] Update docs (`enabling-dialects-and-features.rst`, `ironplcc.rst`, `syntax-support-guide.md`)
- [ ] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork, open PR referencing issue #1199
