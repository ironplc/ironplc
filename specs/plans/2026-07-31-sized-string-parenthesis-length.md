# Plan: `STRING(n)`/`WSTRING(n)` Parenthesis Length Form

## Goal

CODESYS/TwinCAT accept `STRING(n)`/`WSTRING(n)` with parentheses as an
alternate delimiter to the standard, only-currently-accepted
`STRING[n]`/`WSTRING[n]` bracket form. Add parenthesis support in `VAR`
declarations and function return types.

```
FUNCTION_BLOCK FB_Example
VAR
    hostName : STRING(255);   // currently a parse error -- only STRING[255] parses
END_VAR
END_FUNCTION_BLOCK
```

This is split out of #1222 (which bundled two unrelated syntax gaps) and
is part of #1199 (TwinCAT dialect). It is the standalone sized-string half
with no functional dependency on the inline FB-instance call-style
initializer that shipped in the other half.

## Key finding: an unconditional (no dialect flag) precedent already exists

`compiler/parser/src/parser.rs` already has
`string_type_declaration__parenthesis()` sitting right next to the bracket
form `string_type_declaration()` -- for `TYPE ... : STRING(n) ...;` alias
declarations -- with **no dialect gate at all**, both unconditionally tried
in `data_type_declaration()`. The gap is that the *same* parenthesis form
was never added to the other places `STRING`/`WSTRING` length appears with
the bracket-only form:

| Rule | Used for |
|---|---|
| `single_byte_string_spec()` / `double_byte_string_spec()` | `VAR`-declared string variables |
| `var_spec()` | `VAR` declarations going through the generic spec path |
| `function_return_type()` | `FUNCTION ... : STRING(n)` return type |

## Design

Pure grammar addition, no new dialect flag -- parens are just an alternate
delimiter; nothing about the resulting `StringSpecification`/
`StringInitializer` DSL shape depends on which delimiter was used (both
already store only `length: Option<IntegerRef>` with no bracket/paren
marker). No DSL change, no renderer change: the renderer already
normalizes to the bracket form unconditionally.

Add one shared rule and route the four length-capture sites through it:

```rust
rule string_length_spec() -> IntegerRef =
    tok(LeftBracket) _ i:integer_ref() _ tok(RightBracket) { i }
    / tok(LeftParen) _ i:integer_ref() _ tok(RightParen) { i }
```

- `single_byte_string_spec()` / `double_byte_string_spec()`: replace the
  inline `(tok(LeftBracket) _ integer_ref() _ tok(RightBracket))?` length
  capture with `length:string_length_spec()?`.
- `var_spec()` (both String/WString arms): replace inline bracket length
  with `length:(_ l:string_length_spec() { l })?`.
- `function_return_type()` (both String/WString arms): same replacement.

## Non-goals

- No dialect flag (a delimiter choice, not a new keyword).
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
| `compiler/parser/src/parser.rs` | New shared `string_length_spec()` rule; route `single_byte_string_spec()`, `double_byte_string_spec()`, `var_spec()`, `function_return_type()` through it |
| `compiler/parser/src/tests/types_and_returns.rs` | Paren-form + bracket-regression parse tests |
| `compiler/plc2plc/src/tests/declarations.rs` | Round-trip test asserting normalization to brackets |

## Testing Strategy

- Parser: `STRING(255)`/`WSTRING(100)` in a `VAR` declaration and in a
  `FUNCTION` return type parse with `length` populated. Regression: the
  bracket form still parses unchanged.
- plc2plc: `STRING(255)` renders as `STRING [ 255 ]` and round-trips
  through the parser (renderer normalizes to brackets).

## Tasks

- [x] Write plan (this document)
- [x] Add shared `string_length_spec()` rule; route the four sites through it
- [x] Parser tests + plc2plc round-trip test
- [x] Run full CI pipeline (`cd compiler && just`)
