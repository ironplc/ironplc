# Plan: `STRING(n)`/`WSTRING(n)` Parenthesis Length Form

## Goal

CODESYS/TwinCAT accept `STRING(n)`/`WSTRING(n)` with parentheses as an
alternate delimiter to the standard `STRING[n]`/`WSTRING[n]` bracket form.
Add parenthesis support in `VAR` declarations and function return types,
gated behind a vendor-extension flag.

```
FUNCTION_BLOCK FB_Example
VAR
    hostName : STRING(255);   // parse error in strict mode; accepted under --allow-paren-string-length
END_VAR
END_FUNCTION_BLOCK
```

This is split out of #1222 (which bundled two unrelated syntax gaps) and
is part of #1199 (TwinCAT dialect). It is the standalone sized-string half
with no functional dependency on the inline FB-instance call-style
initializer that shipped in the other half.

## Is the parenthesis form standard IEC 61131-3? No.

IEC 61131-3 (both Edition 2 and Edition 3, Annex B) declares a string
length **only** with square brackets:
`single_byte_string_spec ::= 'STRING' ['[' unsigned_int ']'] ...` (and the
matching `'WSTRING' ['[' unsigned_int ']']`). The parenthesis form is a
CODESYS/TwinCAT vendor extension. Therefore the strict default dialects
(`iec61131-3-ed2`, `iec61131-3-ed3`) must reject it, and it belongs behind
an `--allow-*` flag, consistent with every other vendor extension in this
compiler.

Note: the pre-existing `string_type_declaration__parenthesis()` rule (for
`TYPE ... : STRING(n);` aliases) was accepted *unconditionally* — a latent
non-conformance. The token-level gate below closes that gap too, so
parentheses are now rejected uniformly in strict mode wherever a string
length appears.

## Design

### Grammar: parse permissively

Add one shared rule and route the four length-capture sites through it:

```rust
rule string_length_spec() -> IntegerRef =
    tok(LeftBracket) _ i:integer_ref() _ tok(RightBracket) { i }
    / tok(LeftParen) _ i:integer_ref() _ tok(RightParen) { i }
```

- `single_byte_string_spec()` / `double_byte_string_spec()`: replace the
  inline `(tok(LeftBracket) _ integer_ref() _ tok(RightBracket))?` length
  capture with `length:string_length_spec()?`.
- `var_spec()` (both String/WString arms): `length:(_ l:string_length_spec() { l })?`.
- `function_return_type()` (both String/WString arms): same replacement.

The DSL is delimiter-agnostic (`StringSpecification`/`StringInitializer`
store only `length: Option<IntegerRef>` with no bracket/paren marker) and
the renderer already normalizes to brackets, so no DSL or renderer change
is needed.

### Gating: reject in strict mode at the token stage

Following the `rule_token_no_partial_access_syntax` precedent, gating lives
in a token-stream rule (`check_tokens`), not in the grammar (the peg parser
takes no options) and not in the AST (which carries no delimiter marker):

- New flag `allow_paren_string_length` (`--allow-paren-string-length`),
  enabled by the `rusty`, `codesys`, and `twincat` dialects; off in the
  strict `iec61131-3-*` dialects.
- New rule `rule_token_no_paren_string_length`: when the flag is off, a
  `STRING`/`WSTRING` keyword followed (ignoring trivia) by `(` yields a
  `P4042` diagnostic. That token sequence is unambiguous — neither keyword
  is callable and typed string literals use `STRING#` — so no standard
  construct is mis-flagged, and the standard bracket form is untouched.
- New problem code `P4042 ParenStringLengthNotAllowed`, with docs page.

## Non-goals

- No DSL or renderer change (already delimiter-agnostic; renderer
  normalizes to brackets, so `STRING(255)` round-trips to `STRING [ 255 ]`
  -- expected normalization, not a bug).
- No inline FB-instance call-style initializer, no `call_params`, no
  toposort/late-bound-resolution changes -- those belong to the other half
  of #1222.
- No array-element-type `STRING(n)` (`ARRAY[1..10] OF STRING(n)`) -- left
  bracket-only unless a real file needs it.

## File Map

| File | Change |
|------|--------|
| `compiler/parser/src/parser.rs` | New shared `string_length_spec()` rule; route the four length sites through it |
| `compiler/parser/src/options.rs` | New `allow_paren_string_length` flag (rusty/codesys/twincat) + dialect assertion tests |
| `compiler/parser/src/rule_token_no_paren_string_length.rs` | Token-stream gate emitting `P4042` when the flag is off |
| `compiler/parser/src/lib.rs` | Register the new token rule in `check_tokens` |
| `compiler/problems/resources/problem-codes.csv` | `P4042 ParenStringLengthNotAllowed` |
| `docs/reference/compiler/problems/P4042.rst` (+ `index.rst`) | Problem docs |
| `compiler/ironplc-cli/bin/main.rs` | `--allow-paren-string-length` CLI arg + merge |
| `compiler/mcp/src/feature_flag_conformance.rs` | Off→on fixture for the flag |
| `compiler/parser/src/tests/types_and_returns.rs` | Paren parse tests (flag on), strict-reject test, bracket regression |
| `compiler/plc2plc/src/tests/declarations.rs` | Round-trip test (parse paren with flag, render to brackets) |
| `compiler/analyzer/src/**`, `compiler/resources/test/type_decl.st` | Switch pre-existing `STRING(n)` test inputs to the standard `STRING[n]` (they now parse under strict default) |

## Testing Strategy

- Parser: `STRING(255)`/`WSTRING(100)` parse under the flag in both a `VAR`
  declaration and a `FUNCTION` return type. Strict default dialect rejects
  the paren form (P4042). Regression: bracket form still parses unchanged.
- Token rule: unit tests for flag on/off, both keywords, trivia between
  keyword and `(`, and the bracket form staying allowed.
- Options: the rusty/codesys/twincat dialect assertion tests include the
  new flag; the MCP conformance fixture proves it flips reject→accept.
- plc2plc: `STRING(255)` (flag on) renders as `STRING [ 255 ]` and the
  normalized output round-trips under the strict default.

## Tasks

- [x] Write plan (this document)
- [x] Add shared `string_length_spec()` rule; route the four sites through it
- [x] Add `allow_paren_string_length` flag + P4042 + token-stream gate
- [x] CLI arg, MCP fixture, docs page, dialect assertion tests
- [x] Migrate pre-existing paren test inputs to brackets
- [x] Parser tests (positive/negative) + plc2plc round-trip test
- [x] Run full CI pipeline (`cd compiler && just`)
