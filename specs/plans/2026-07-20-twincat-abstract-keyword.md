# Plan: `ABSTRACT` Function Block Keyword

## Goal

`FUNCTION_BLOCK ABSTRACT <name> ...` doesn't parse at all today — `ABSTRACT`
isn't a registered keyword, so it gets consumed as the function block's
own *name* (`derived_function_block_name()` is just `type_name()`, a
single identifier), which is why the *real* name that follows, and any
`EXTENDS`/`IMPLEMENTS` clause after that, then fail to parse. This is one
root cause, not two separate gaps — the original survey counted "plain
`ABSTRACT`" and "`ABSTRACT` + `EXTENDS`/`IMPLEMENTS`" as separate buckets
(7 files each), but both fail for the exact same reason.

```
FUNCTION_BLOCK ABSTRACT FB_BaseAxis IMPLEMENTS I_BaseAxis  // currently a parse error
END_FUNCTION_BLOCK
```

## Verification against real files / direct repro

Confirmed directly with a minimal repro:
`FUNCTION_BLOCK ABSTRACT FB_Derived END_FUNCTION_BLOCK` fails with a
`P0002` syntax error at `END_FUNCTION_BLOCK`, not at `FB_Derived` —
because `ABSTRACT` is swallowed as the FB's name, and `FB_Derived` is
then parsed as a bare statement in the (VAR-less) function block body,
which needs a continuation token that never comes.

Real usage (a private test corpus) always combines `ABSTRACT` with
`EXTENDS` and/or `IMPLEMENTS` (e.g.
`FUNCTION_BLOCK ABSTRACT FB_AxisControl EXTENDS FB_BaseAxis IMPLEMENTS I_Axis`,
`FUNCTION_BLOCK ABSTRACT FB_BaseAxis IMPLEMENTS I_BaseAxis`) — no example
of bare `ABSTRACT` with neither clause, though the grammar doesn't need
to assume that combination.

## Design

### New token: `Abstract`

Added alongside the existing CODESYS/TwinCAT OOP tokens
(`EXTENDS`/`IMPLEMENTS`/`INTERFACE`/`END_INTERFACE`) in `token.rs`, and
demoted under the exact same `allow_oop_extensions` condition in
`xform_demote_oop_keywords.rs` — not a new flag. `ABSTRACT` is used
exclusively alongside `EXTENDS`/`IMPLEMENTS` in every real example found,
so it belongs with the same feature flag, not `--allow-ref-to` or a
flag of its own.

### Grammar: optional prefix on `function_block_declaration()`

```
rule function_block_declaration() -> FunctionBlockDeclaration =
  start:tok(TokenType::FunctionBlock) _
  is_abstract:(tok(TokenType::Abstract) {})? _
  name:derived_function_block_name() _
  extends:(...)? _ implements:(...)? _ ...
```

`FunctionBlockDeclaration` gains `is_abstract: bool` (not `abstract` —
that's a reserved word in Rust). No ordering hazard: `Abstract` is a
dedicated keyword token via the demotion pattern, not an identifier that
could collide with an existing grammar path.

### Semantic: extend the existing `P9004` condition, don't add a new check

`rule_unsupported_extension.rs`'s `visit_function_block_declaration`
currently flags when `extends.is_some() || !implements.is_empty()`.
Extend to `|| node.is_abstract` — `ABSTRACT` modifies function-block
semantics (not instantiable directly) the same "recognized but not
enforced" way `EXTENDS`/`IMPLEMENTS` already are; IronPLC doesn't check
instantiation legality for any function block today, abstract or not.
No real file found needs bare `ABSTRACT` (no `EXTENDS`/`IMPLEMENTS`) to
*not* be flagged, so there's no reason to special-case that combination.

## Non-goals

- Enforcing that an `ABSTRACT` function block is never directly
  instantiated — no evidence any real file needs this checked, and it's
  squarely in the same "not yet implemented" bucket as inheritance
  dispatch itself.
- Any change to `INTERFACE`'s own handling — interfaces don't have their
  own `ABSTRACT` modifier in the survey or real files.

## File Map

| File | Change |
|------|--------|
| `compiler/parser/src/token.rs` | New `Abstract` token |
| `compiler/parser/src/xform_demote_oop_keywords.rs` | Demote `Abstract` under the existing `allow_oop_extensions` condition |
| `compiler/parser/src/parser.rs` | Optional `ABSTRACT` prefix in `function_block_declaration()` |
| `compiler/dsl/src/common.rs` | `FunctionBlockDeclaration.is_abstract: bool` |
| `compiler/analyzer/src/rule_unsupported_extension.rs` | Extend the existing flag condition |

## Testing Strategy

- Parser tests: `FUNCTION_BLOCK ABSTRACT <name> END_FUNCTION_BLOCK`
  parses (`is_abstract: true`); combined with `EXTENDS`/`IMPLEMENTS`
  (matching real usage) also parses; regression — a plain
  `FUNCTION_BLOCK <name>` (no `ABSTRACT`) still parses with
  `is_abstract: false`; `ABSTRACT` used as an ordinary identifier when
  `allow_oop_extensions` is disabled still parses as a variable/type name
  (demotion regression, matching `EXTENDS`/`IMPLEMENTS`'s existing tests).
- Demotion tests: `Abstract` demotes to identifier when
  `allow_oop_extensions` is off, stays a keyword when on (same shape as
  the existing `Extends`/`Implements`/`Interface` tests).
- Semantic test: `ABSTRACT` alone (no `EXTENDS`/`IMPLEMENTS`) still
  produces `P9004`; combined with `EXTENDS`/`IMPLEMENTS` also produces
  exactly one `P9004` (not two).
- plc2plc round-trip test.

## Tasks

- [x] Write plan (this document)
- [x] `Abstract` token + demotion wiring
- [x] Grammar: optional `ABSTRACT` prefix + `is_abstract` field
- [x] Extend `rule_unsupported_extension.rs`'s flag condition
- [x] Check plc2plc renderer; fix/extend if needed (renderer needed a new
      `if node.is_abstract { self.write_ws("ABSTRACT"); }` branch)
- [x] Tests from Testing Strategy
- [x] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork
- [ ] Merge into `twincat-dev`, update `twincat-status.md`, push

## Implementation Notes

- Every other `FunctionBlockDeclaration` construction site in the
  codebase (`sources/xml/transform.rs` for PLCopen XML, 4 sites in
  `analyzer/xform_resolve_late_bound_type_initializer.rs`'s tests) needed
  `is_abstract: false` added — surfaced by `cargo build`/`cargo test`,
  following the established pattern of building first and letting the
  compiler enumerate construction sites rather than grepping for them
  upfront.
- `is_abstract: bool` needed `#[recurse(ignore)]` (the `Recurse` derive
  macro has no generated visitor method for a bare `bool`) — the same
  attribute already used on `SourceSpan` and other non-recursed fields
  elsewhere in `common.rs`.
- `VendorExtension::extension_name()` for `FunctionBlockDeclaration` was
  renamed from `"EXTENDS/IMPLEMENTS clause"` to
  `"EXTENDS/IMPLEMENTS/ABSTRACT clause"` — no test asserted the old exact
  string, only the `P9004` code, so this was safe to update for accuracy.
- Verified end-to-end via the actual CLI (`--dialect=codesys`): a
  function block combining `ABSTRACT`, `EXTENDS`, and `IMPLEMENTS` now
  parses correctly and produces exactly one `P9004` diagnostic for the FB
  (plus a separate one for the `INTERFACE` declaration it references) —
  confirming the single-root-cause fix (a token the FB-name rule was
  swallowing) resolves both the "bare ABSTRACT" and "ABSTRACT combined
  with EXTENDS/IMPLEMENTS" buckets from the original survey.
- No CLI flag exists for `allow_oop_extensions` itself (only
  `--dialect=rusty`/`--dialect=codesys` and the LSP `allowOopExtensions`
  setting) — this predates this change (the original EXTENDS/IMPLEMENTS/
  INTERFACE work didn't add one either) and is out of scope here.
