# `VAR PERSISTENT` support (TwinCAT/CODESYS)

## Goal

Support the `PERSISTENT` variable-declaration qualifier: `VAR_GLOBAL
PERSISTENT` (the common case, in a GVL), `PROGRAM ... VAR PERSISTENT`,
and `FUNCTION_BLOCK ... VAR PERSISTENT`. Closes
[#1679](https://github.com/ironplc/ironplc/issues/1679).

`PERSISTENT` is not part of the IEC 61131-3 standard (unlike `RETAIN`/
`NON_RETAIN`/`CONSTANT`, which are core keywords parsed unconditionally)
— it is a Beckhoff/CODESYS vendor extension. So unlike `RETAIN`, it is
gated behind a new dialect-extension flag, following the established
precedent for this exact class of extension: `allow_reference_to` /
`allow_pointer_to` / `allow_adr`, all enabled for `[Codesys, TwinCat]`
only (deliberately *not* `Rusty` — confirmed by reading
`rusty_dialect_enables_exactly_these_flags`, which omits all three).

## What's already there, half-built

`compiler/sources/src/xml/schema.rs:353` (`VarList.persistent: bool`)
and `compiler/sources/src/xml/position.rs:469` (reads the XML
`persistent="true"` attribute into it) already exist — the TwinCAT XML
schema layer captures this. But
`compiler/sources/src/xml/transform.rs:573`
(`transform_var_list`) never checks `var_list.persistent`, so a real
`.TcGVL` file with `Persistent="true"` on a `<GlobalVars>` block
silently loses that fact today. This plan fixes that dead code as part
of adding the qualifier it should have produced all along.

## Architecture

Mirrors the existing `RETAIN`/`NON_RETAIN` machinery exactly, plus one
new flag gate:

1. **Token** (`compiler/parser/src/token.rs`): new `Persistent` token
   for `PERSISTENT` (case-insensitive), next to `Retain`/`NonRetain`.
2. **Flag** (`compiler/parser/src/options.rs`): new
   `allow_persistent_var` descriptor, `--allow-persistent-var`,
   `[Codesys, TwinCat]`.
3. **Demotion** (`compiler/parser/src/xform_demote_keywords.rs`): new
   `demote_persistent = !options.allow_persistent_var` gate, own match
   arm (not folded into `demote_oop` — unrelated feature), doc comment
   updated.
4. **Empty-var-block check**
   (`compiler/parser/src/rule_no_empty_var_blocks.rs`): add
   `TokenType::Persistent` to `is_qualifier` (this pass runs after
   demotion, so when the flag is on the token is still `Persistent` and
   must be recognized the same way `Retain`/`NonRetain` already are).
5. **DSL** (`compiler/dsl/src/common.rs:2442`): new
   `DeclarationQualifier::Persistent` variant.
6. **Grammar** (`compiler/parser/src/parser.rs`), three call sites,
   matching where `RETAIN` already reaches each context:
   - `global_var_declarations__qualifier()` (`:1265`) — add `Persistent`
     as a third alternative. Covers `VAR_GLOBAL PERSISTENT` in GVLs,
     the most common real-world case.
   - `program_var_declarations()` (`:1521`) — already has a flexible
     `(Constant | Retain | NonRetain)?` group; add `Persistent` as a
     fourth alternative. Covers `PROGRAM ... VAR PERSISTENT`.
   - `other_var_declarations()` (`:1499`) — unlike the two rules above,
     this doesn't use a flexible-qualifier group; `RETAIN` gets its own
     dedicated production (`retentive_var_declarations`, `:1220`). Add
     a new `persistent_var_declarations()` rule mirroring it exactly,
     and add it to the `other_var_declarations()` alternation. Covers
     `FUNCTION_BLOCK ... VAR PERSISTENT` (and, incidentally, `METHOD`
     bodies, which share this rule — harmless, not separately tested).
   - Deliberately **not** touched: `located_var_declarations()` (`VAR
     ... AT %I* : ...`). Combining a physical I/O-mapped address with
     `PERSISTENT` is not a real pattern worth the extra grammar surface
     right now; can be added later if it turns out to matter.
7. **XML transform fix**
   (`compiler/sources/src/xml/transform.rs:573`): add the
   `var_list.persistent` check to `transform_var_list`'s
   constant/retain/nonretain if-else chain, producing
   `DeclarationQualifier::Persistent`.
8. **Renderer** (`compiler/plc2plc/src/renderer.rs`): three exhaustive
   `match node.qualifier` / `match storage` sites (`:678`, `:715`,
   `:1236`) all need a new arm since there's no wildcard. The
   `visit_program_configuration` storage match (`:1236`) can never
   actually receive `Persistent` (that path is fed only by
   `Retain`/`NonRetain` in `program_configuration()`, `:1741`, an
   unrelated "CONFIGURATION resource retain" concept) — map it to `""`
   with a short comment, matching how `Unspecified`/`Constant` already
   do, rather than leaving it a compile error.
9. **Semantics deferred**: `rule_var_decl_const_initialized.rs` needs
   one new exhaustive-match arm, `DeclarationQualifier::Persistent =>
   {}` (a persistent var doesn't require an initializer any more than
   `RETAIN` does). Nothing else in the analyzer changes — matching the
   precedent already set for `PROPERTY` (#1420) and `RETAIN` itself,
   actual "survives a reload" behavior is separate, larger, later work.
10. **Feature-flag conformance**
    (`compiler/mcp/src/feature_flag_conformance.rs`): a new flag
    *must* get a `FLAG_FIXTURES` entry or the
    `every_feature_flag_has_a_fixture` meta-test fails the build. One
    entry: a `VAR_GLOBAL PERSISTENT` snippet, rejected off / accepted
    on.

## Prefactoring

None needed. Every touched site is either a new match arm (required by
Rust's exhaustiveness checking, not optional cleanup) or a new
alternative in an existing choice-of-rules grammar production, in a
codebase that already has three near-identical qualifiers going through
this exact shape three times over (`Constant`/`Retain`/`NonRetain`).
Adding a fourth is following the grain, not fighting it — factoring the
repeated `(Constant | Retain | NonRetain)?` grouping into a shared
sub-rule now, for a mechanical fourth alternative, would be the kind of
speculative refactor the standards warn against for existing call
sites that aren't being touched for their own sake.

## Design doc reference

None exists yet — `specs/design/beckhoff-twincat-dialect.md` doesn't
mention `PERSISTENT` at all (confirmed absent during the audit that
found this gap). Not adding one here: this is a single grammar
extension of an existing, well-understood pattern (three prior
qualifiers), not new architecture that needs its own design doc.

## File map

- `compiler/parser/src/token.rs` — new `Persistent` token
- `compiler/parser/src/options.rs` — new `allow_persistent_var` flag
- `compiler/parser/src/xform_demote_keywords.rs` — new demotion gate
  + tests
- `compiler/parser/src/rule_no_empty_var_blocks.rs` — recognize the
  new qualifier token
- `compiler/dsl/src/common.rs` — new `DeclarationQualifier` variant
- `compiler/parser/src/parser.rs` — three grammar call sites
- `compiler/parser/src/tests/var_declarations.rs` — new parse tests
- `compiler/sources/src/xml/transform.rs` — fix dead `persistent` field
- `compiler/plc2plc/src/renderer.rs` — three new match arms
- `compiler/plc2plc/src/tests/` — new render/round-trip test(s)
- `compiler/analyzer/src/rule_var_decl_const_initialized.rs` — one new
  match arm
- `compiler/mcp/src/feature_flag_conformance.rs` — one new
  `FlagFixture`

## Tasks

- [ ] Token + flag + demotion gate + empty-var-block recognition
- [ ] `DeclarationQualifier::Persistent` in the DSL
- [ ] Grammar: `global_var_declarations__qualifier`,
      `program_var_declarations`, new `persistent_var_declarations`
      wired into `other_var_declarations`
- [ ] Fix `transform_var_list` to honor `var_list.persistent`
- [ ] Renderer: three new match arms
- [ ] Analyzer: one new match arm (no-op, matching `Retain`/`NonRetain`)
- [ ] Parser tests mirroring
      `parse_when_program_mixed_vars_with_retain_qualifier_then_ok`,
      for `VAR_GLOBAL PERSISTENT`, `PROGRAM ... VAR PERSISTENT`, and
      `FUNCTION_BLOCK ... VAR PERSISTENT`
- [ ] `xform_demote_keywords` on/off tests (mirroring the `REFERENCE`
      pair)
- [ ] plc2plc round-trip test (mirroring the existing render tests'
      local-options pattern)
- [ ] `FLAG_FIXTURES` entry for `allow_persistent_var`
- [ ] Run `cd compiler && just` (compile, coverage, clippy, fmt, dupes)
- [ ] `git rm` this plan file before opening the PR
- [ ] Push the branch and open a PR against `ironplc/ironplc` `main`
