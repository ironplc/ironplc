# Enforce `--allow-top-level-var-global` (P4028)

## Problem

The `allow_top_level_var_global` feature flag is declared, surfaced by
`list_options`, exposed on the CLI/LSP, and documented, but is **never
enforced**. A top-level `VAR_GLOBAL ... END_VAR` block compiles cleanly
whether the flag is on or off. Problem code **P4028
`TopLevelVarGlobalNotAllowed`** is reserved and documented but emitted
nowhere. See ironplc/ironplc#1233.

## Approach

Add a token-level validation rule following the established pattern in
`parser/src/rule_no_empty_var_blocks.rs` and
`parser/src/rule_token_no_c_style_comment.rs`. The rule runs in
`check_tokens()` during `tokenize_program()`.

A `VAR_GLOBAL` is "top level" when it appears outside any
`CONFIGURATION`/`RESOURCE` block — the only two places the standard permits
`VAR_GLOBAL`. Track block-nesting depth over the token stream:

- Increment depth on `CONFIGURATION` / `RESOURCE`.
- Decrement on `END_CONFIGURATION` / `END_RESOURCE`.
- A `VAR_GLOBAL` keyword seen at depth `0` is top-level → emit P4028 when
  `allow_top_level_var_global` is `false`.

This unambiguously matches the grammar, where a top-level `VAR_GLOBAL`
parses to `LibraryElementKind::GlobalVarDeclarations` while config/resource
globals are absorbed into the `ConfigurationDeclaration`.

## Changes

1. `compiler/parser/src/rule_no_top_level_var_global.rs` — new rule module
   with the depth-tracking logic and unit tests.
2. `compiler/parser/src/lib.rs` — register the module and add it to the
   `check_tokens()` rule list.
3. `compiler/mcp/src/feature_flag_conformance.rs` — move
   `allow_top_level_var_global` from `UNENFORCED` to `FLAG_FIXTURES` with a
   source snippet (rejected off, accepted on), per the file's own comment.

## Testing

- Unit tests in the new rule module (top-level rejected when flag off,
  accepted when flag on; config/resource globals never flagged).
- The behavioral conformance fixture proves the off→on flip end-to-end.
- Full CI: `cd compiler && just`.
