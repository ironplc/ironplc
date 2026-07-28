# Enforce `--allow-top-level-var-global` (P4028)

## Problem

The `allow_top_level_var_global` feature flag is declared, surfaced by
`list_options`, exposed on the CLI/LSP, and documented, but is **never
enforced**. A top-level `VAR_GLOBAL ... END_VAR` block compiles cleanly
whether the flag is on or off. Problem code **P4028
`TopLevelVarGlobalNotAllowed`** is reserved and documented but emitted
nowhere. See ironplc/ironplc#1233.

## Approach

Add a semantic (AST-level) rule in the analyzer, alongside the other
flag-gated rules (`rule_ref_to`, `xform_int_to_bool_initializer`, …). The
parser already represents a top-level `VAR_GLOBAL` block as a distinct
`LibraryElementKind::GlobalVarDeclarations` element, while config/resource
globals are absorbed into the `ConfigurationDeclaration`. So the rule is
simply: iterate `library.elements` and emit P4028 for each
`GlobalVarDeclarations` when `allow_top_level_var_global` is `false` — no
token depth-tracking needed. (An earlier draft used a token-level rule with
manual CONFIGURATION/RESOURCE nesting tracking; the AST check is
considerably less code for the same behavior — see PR #1251 review.)

## Changes

1. `compiler/analyzer/src/rule_no_top_level_var_global.rs` — new semantic
   rule module with unit tests.
2. `compiler/analyzer/src/lib.rs` — declare the module.
3. `compiler/analyzer/src/stages.rs` — register the rule in the `semantic`
   stage's rule list.
4. `compiler/mcp/src/feature_flag_conformance.rs` — move
   `allow_top_level_var_global` from `UNENFORCED` to `FLAG_FIXTURES` with a
   source snippet (rejected off, accepted on), per the file's own comment.

## Testing

- Unit tests in the new rule module (top-level rejected when flag off,
  accepted when flag on; config/resource globals never flagged).
- The behavioral conformance fixture proves the off→on flip end-to-end.
- Full CI: `cd compiler && just`.
