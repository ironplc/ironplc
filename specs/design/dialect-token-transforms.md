# Design: Dialect Token Transform Pipeline

## Overview

This document describes the architecture for dialect-specific token transforms — the mechanism that enables vendor-dialect parsing without modifying the core logos lexer. It is the shared infrastructure underlying both the [Siemens SCL](siemens-scl-dialect.md) and [Beckhoff TwinCAT](beckhoff-twincat-dialect.md) dialect designs.

The core principle: **the logos lexer stays dialect-neutral**. It only recognizes IEC 61131-3 standard tokens. Vendor-specific syntax is handled by a pipeline of token transforms that run between lexing and parsing.

## Problem

The logos lexer in `token.rs` declares standard keywords with `#[token("KEYWORD", ignore(case))]` at higher priority than the `Identifier` regex. This means `FUNCTION_BLOCK` always lexes as `TokenType::FunctionBlock`, never as `Identifier`. This is correct for IEC 61131-3.

Vendor dialects introduce new keywords (`METHOD`, `REGION`, `BEGIN`, `EXTENDS`, etc.) and new syntax conventions (`#variable`, `"quoted_name"`, `{pragma}`). These cannot be added to the logos lexer because:

1. **Vendor keywords shadow valid identifiers.** In standard IEC 61131-3, `METHOD`, `INTERFACE`, `BEGIN`, `REGION` are all legal variable or type names. Making them always-on keywords breaks valid standard programs.

2. **Vendor syntax changes token semantics.** In Siemens SCL, `"FB_Motor"` is an identifier, not a wide string literal. In standard mode, it's a `DoubleByteString`. The same token text has different meaning depending on dialect.

3. **Vendor syntax merges or removes tokens.** Siemens SCL's `#counter` should become a single variable reference, not `Hash` + `Identifier`. Pragma blocks `{ ... }` should become a single opaque token.

## Transform Categories

Token transforms fall into four distinct categories, each with different invariants:

### Category 1: Keyword Promotion

**What:** Change an `Identifier` token's `token_type` to a vendor keyword type.

**Invariants:**
- `token_type` changes
- `text` unchanged
- `span`, `line`, `col` unchanged

**Example (Beckhoff):**

```
Before:  Token { token_type: Identifier, text: "METHOD", span: 45..51 }
After:   Token { token_type: Method,     text: "METHOD", span: 45..51 }
```

**Example (Siemens):**

```
Before:  Token { token_type: Identifier, text: "REGION", span: 10..16 }
After:   Token { token_type: Region,     text: "REGION", span: 10..16 }
```

**Implementation:** A lookup table maps identifier text (case-insensitive) to the target `TokenType`. Each dialect has its own promotion table. The transform iterates over all tokens and replaces matching identifiers.

**Correctness argument:** Since the `Identifier` regex in logos has the lowest priority (`priority = 1`), any text that matches a standard keyword will already have been lexed as that keyword. Only text that is NOT a standard keyword will be `Identifier`. So keyword promotion cannot accidentally re-classify standard keywords — they never reach this transform as `Identifier` tokens.

### Category 2: Token Rewriting

**What:** Change a token's `token_type` AND `text`, keeping `span` pointing to the original source.

**Invariants:**
- `token_type` changes
- `text` changes (content extraction)
- `span`, `line`, `col` unchanged — still points to original source location

**Example (Siemens double-quoted names):**

```
Before:  Token { token_type: DoubleByteString, text: "\"FB_Motor\"", span: 20..30 }
After:   Token { token_type: Identifier,       text: "FB_Motor",     span: 20..30 }
```

The span still covers the full `"FB_Motor"` range in the source file, including quotes. This means error messages will highlight the quoted name in context — which is correct and helpful.

**When this applies:** In Siemens SCL, double-quoted text is ALWAYS an identifier/name, never a string literal. String literals use single quotes. So the transform is a blanket rule for all `DoubleByteString` tokens in the SCL dialect — no context sensitivity needed.

**Key distinction from standard mode:** In standard IEC 61131-3, `"..."` is a wide (double-byte) string literal. In Beckhoff TwinCAT, it's also a WSTRING literal. This transform ONLY applies in the Siemens SCL dialect.

### Category 3: Token Filtering

**What:** Remove tokens from the stream that are syntactic markers with no semantic meaning.

**Invariants:**
- Tokens are removed entirely
- Remaining tokens have unchanged `span`, `text`, `line`, `col`
- Token indices shift (subsequent tokens have lower indices)

**Example (Siemens `#` variable prefix):**

In Siemens SCL, `#counter` lexes as two tokens: `Hash` + `Identifier("counter")`. The `#` is a syntactic marker indicating a block-local variable. The parser only needs the `Identifier`.

```
Before:  [Hash { span: 50..51 }, Identifier { text: "counter", span: 51..58 }]
After:   [Identifier { text: "counter", span: 51..58 }]
```

The `Hash` token is removed. The `Identifier` token retains its original span pointing to `counter` (not `#counter`). This is acceptable because:
- The variable's name IS `counter` — the `#` is not part of the name
- Error messages pointing to `counter` are clear and correct
- The `span` of the removed `Hash` token is not needed by anything downstream

**Filter rule:** Remove `Hash` when immediately followed by `Identifier`. This is safe because in standard IEC 61131-3, `Hash` before `Identifier` never occurs — the standard uses `#` only between type keywords and literal values (e.g., `INT#5` lexes as `Int` + `Hash` + `Digits`).

**Alternative considered — token merging:** Merge `Hash` + `Identifier` into a single `Identifier` with a combined span covering `#counter`. This was rejected because:
- The combined span is wider than the `text`, which is unusual and could confuse diagnostics that expect `span.len() ≈ text.len()`
- Filtering is simpler (just drop the `Hash`) and the information loss is minimal
- The `#` prefix carries no information the parser or analyzer needs

### Category 4: Token Collapsing

**What:** Replace a sequence of N tokens between delimiters with a single opaque token.

**Invariants:**
- N tokens become 1 token
- The new token's `span` covers from the start of the first collapsed token to the end of the last
- The new token's `text` is the concatenation of the collapsed tokens' text (or the original source substring)
- `line`, `col` come from the first token in the sequence

**Example (pragma collapsing):**

```
Before:  [LeftBrace, Identifier("S7_Optimized_Access"), ..., RightBrace]
After:   [Pragma { text: "{S7_Optimized_Access := 'TRUE'}", span: 20..55 }]
```

**Implementation:** Scan the token stream for `LeftBrace`. When found, scan forward for the matching `RightBrace` (pragmas do not nest in either Siemens or Beckhoff). Replace the entire range with a single `Pragma` token. The pragma content is opaque — the parser skips `Pragma` tokens like whitespace.

**Applies to both dialects:** Siemens SCL uses `{S7_Optimized_Access := 'TRUE'}` and Beckhoff uses `{attribute 'qualified_only'}`. Both use the same `{ ... }` delimiters. This transform is shared.

**Edge case — nested braces:** Siemens and Beckhoff pragmas do not use nested braces. If a `RightBrace` is not found, the `LeftBrace` is left as-is (it will produce a parse error in standard mode, which is the correct behavior for malformed input).

### What about REGION / END_REGION?

After keyword promotion, `REGION` and `END_REGION` are keyword tokens. Two options:

**Option A — Filter them out:** Remove `Region` tokens and everything until the next `Newline` (the region name text). Remove `EndRegion` tokens. This changes token indices.

**Option B — Parser skips them:** Keep them in the stream. The parser grammar includes rules to skip `Region ... Newline` and `EndRegion` at statement boundaries, treating them like comments. No token index changes.

**Recommended: Option A (filter).** The parser already skips `Whitespace`, `Newline`, and `Comment` tokens in its `_` (whitespace) rule. Adding `Region` to that list is possible but leaks dialect concerns into the parser grammar. Filtering in the transform pipeline keeps the parser dialect-unaware.

The filter removes: `Region` token + all tokens until `Newline` (the region name), and standalone `EndRegion` tokens.

## Pipeline Order

The transforms must run in a specific order because some depend on the output of others:

```
Source text
  │
  ▼
┌─────────────────────────────────────────────────┐
│ 1. Logos lexer                                  │
│    Standard IEC 61131-3 tokens only.            │
│    Vendor keywords arrive as Identifier.        │
│    "quoted" text arrives as DoubleByteString.   │
│    # arrives as Hash.                           │
│    { } arrive as LeftBrace / RightBrace.        │
└─────────────────────────────────────────────────┘
  │
  ▼
┌─────────────────────────────────────────────────┐
│ 2. Preprocessor (all dialects)                  │
│    Remove OSCAT ranged comments.                │
│    (Operates on source text, not tokens.        │
│     Runs before lexing in current code.)        │
└─────────────────────────────────────────────────┘
  │
  ▼
┌─────────────────────────────────────────────────┐
│ 3. Dialect token transforms (when dialect !=    │
│    Standard). Order within this stage:          │
│                                                 │
│    a. Keyword promotion                         │
│       Identifier → vendor keyword TokenType.    │
│       Must run first so subsequent transforms   │
│       can match on promoted types (e.g.,        │
│       Region, EndRegion).                       │
│                                                 │
│    b. Token rewriting                           │
│       DoubleByteString → Identifier (SCL only). │
│       Independent of keyword promotion.         │
│                                                 │
│    c. Pragma collapsing                         │
│       { ... } → single Pragma token.            │
│       Must run after keyword promotion so that  │
│       keywords inside pragmas are not promoted  │
│       — wait, actually pragmas are collapsed    │
│       into opaque text, so promotion of their   │
│       contents doesn't matter. But collapsing   │
│       must run before filtering so that { }     │
│       tokens are consumed before the filter     │
│       pass.                                     │
│                                                 │
│    d. Token filtering                           │
│       Remove Hash before Identifier (SCL).      │
│       Remove Region...Newline, EndRegion (SCL). │
│       Must run after keyword promotion (needs   │
│       Region/EndRegion to be keyword tokens).   │
│       Must run after pragma collapsing (don't   │
│       accidentally remove { } that are part of  │
│       a pragma).                                │
└─────────────────────────────────────────────────┘
  │
  ▼
┌─────────────────────────────────────────────────┐
│ 4. Standard token transforms (all dialects)     │
│    Insert keyword statement terminators          │
│    (existing xform_tokens.rs logic).            │
└─────────────────────────────────────────────────┘
  │
  ▼
┌─────────────────────────────────────────────────┐
│ 5. Token validation rules (all dialects)        │
│    C-style comment check, etc.                  │
│    (existing check_tokens logic).               │
└─────────────────────────────────────────────────┘
  │
  ▼
┌─────────────────────────────────────────────────┐
│ 6. PEG parser                                   │
│    Consumes transformed token stream.           │
│    Dialect-unaware for most rules.              │
│    Some rules may check dialect for extensions  │
│    (e.g., accept METHOD inside FUNCTION_BLOCK   │
│    only when dialect is TwinCAT).               │
└─────────────────────────────────────────────────┘
```

### Ordering constraints

| Transform | Depends on | Reason |
|-----------|-----------|--------|
| Keyword promotion | (none) | Runs first |
| Token rewriting | (none) | Independent |
| Pragma collapsing | (none, but before filtering) | Must consume `{ }` before filter pass |
| Token filtering | Keyword promotion | Needs `Region`/`EndRegion` to be keyword types |
| Token filtering | Pragma collapsing | Must not remove `{ }` that are pragma delimiters |

In practice, the simplest correct ordering is: promotion → rewriting → collapsing → filtering. These can be composed as sequential passes or as a single pass with careful state management.

## Integration with Existing Code

### Current pipeline (from `lib.rs`)

```rust
pub fn tokenize_program(source, file_id, options, line_offset, col_offset) {
    let source = preprocess(source);                           // step 2
    let (tokens, errors) = tokenize(&source, file_id, ...);   // step 1
    let tokens = insert_keyword_statement_terminators(tokens);  // step 4
    let result = check_tokens(&tokens, options);                // step 5
    (tokens, errors)
}
```

### Proposed pipeline

```rust
pub fn tokenize_program(source, file_id, options, line_offset, col_offset) {
    let source = preprocess(source);                           // step 2
    let (tokens, errors) = tokenize(&source, file_id, ...);   // step 1
    let tokens = apply_dialect_transforms(tokens, &options);    // step 3 (NEW)
    let tokens = insert_keyword_statement_terminators(tokens);  // step 4
    let result = check_tokens(&tokens, options);                // step 5
    (tokens, errors)
}

fn apply_dialect_transforms(tokens: Vec<Token>, options: &ParseOptions) -> Vec<Token> {
    match options.dialect {
        Dialect::Standard => tokens,
        Dialect::SiemensSCL => {
            let tokens = promote_keywords(tokens, Dialect::SiemensSCL);
            let tokens = rewrite_double_quoted_to_identifier(tokens);
            let tokens = collapse_pragmas(tokens);
            filter_scl_tokens(tokens)
        }
        Dialect::BeckhoffTwinCAT => {
            let tokens = promote_keywords(tokens, Dialect::BeckhoffTwinCAT);
            let tokens = collapse_pragmas(tokens);
            tokens // no filtering needed for TwinCAT
        }
    }
}
```

Keyword promotion uses a single `promote_keywords` function driven by the shared `DIALECT_KEYWORDS` table (see the [Extension Origin Model](beckhoff-twincat-dialect.md#extension-origin-model) in the Beckhoff design). Each entry in the table has an `origins` field; the function promotes identifiers whose entry origins intersect the active dialect's origin set. This avoids duplicating promotion logic per dialect.

### Module organization

New file: `compiler/parser/src/xform_dialect.rs`

Contains:
- `apply_dialect_transforms` (top-level dispatcher)
- `promote_keywords` (shared, driven by `DIALECT_KEYWORDS` table)
- `rewrite_double_quoted_to_identifier` (SCL only)
- `collapse_pragmas` (shared)
- `filter_scl_tokens` (SCL only)

Each function takes `Vec<Token>` and returns `Vec<Token>`. Each is independently testable.

## Span Preservation Summary

| Transform | span | text | token_type | Token count |
|-----------|------|------|------------|-------------|
| Keyword promotion | unchanged | unchanged | changed | same |
| Token rewriting | unchanged | changed (content extracted) | changed | same |
| Token filtering | unchanged on remaining | unchanged | unchanged | decreased |
| Pragma collapsing | new (union of collapsed) | new (full pragma text) | new (`Pragma`) | decreased |

The critical invariant: **all spans point into the original source file**. No transform creates spans that reference intermediate or synthetic text. The preprocessor (OSCAT comment removal) already maintains this invariant by replacing comment text with whitespace of equal length.

## Testing Strategy

Each transform category is tested independently:

1. **Keyword promotion tests** — verify that specific identifier text maps to the correct keyword type, case insensitivity works, and non-keyword identifiers are untouched

2. **Token rewriting tests** — verify that `DoubleByteString` with quotes becomes `Identifier` without quotes, span is preserved, and this only applies in SCL mode

3. **Token filtering tests** — verify that `Hash` before `Identifier` is removed, `Hash` in `INT#5` context is NOT removed, `Region`/`EndRegion` blocks are removed with their region name text

4. **Pragma collapsing tests** — verify that `{ ... }` becomes a single `Pragma`, nested content is opaque, unclosed braces are left as-is

5. **Pipeline integration tests** — verify the full transform chain on representative Siemens SCL and Beckhoff TwinCAT token streams

6. **Regression tests** — verify that `apply_dialect_transforms` with `Dialect::Standard` returns tokens unchanged
