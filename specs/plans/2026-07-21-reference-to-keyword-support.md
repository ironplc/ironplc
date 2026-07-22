# Plan: TwinCAT `REFERENCE TO` Keyword Support

## Goal

Add support for Beckhoff TwinCAT / CODESYS `REFERENCE TO` reference types as a
distinct, separately-flagged alternative to the existing IEC 61131-3 `REF_TO`
syntax. The two are surface-level variants of the same underlying concept
(a strongly-typed reference implemented as a variable-table index) but with
*different usage models*:

| Concern | IEC `REF_TO` (`--allow-ref-to`) | TwinCAT `REFERENCE TO` (`--allow-reference-to`) |
|---------|--------------------------------|------------------------------------------------|
| Declare | `r : REF_TO INT;` | `r : REFERENCE TO INT;` |
| Bind    | `r := REF(x);` | `r REF= x;` |
| Read    | `y := r^;` (explicit `^`) | `y := r;` (implicit dereference) |
| Write   | `r^ := 5;` (explicit `^`) | `r := 5;` (implicit dereference) |
| Validity| `r = NULL` | `__ISVALIDREF(r)` / `r = 0` |

`REF_TO` and `REFERENCE TO` are **not** made mutually exclusive at the compiler
level. Per [ADR-0038](../adrs/0038-no-restrictions-on-flag-combinations.md), the
compiler does not restrict `--allow-*` flag combinations; preference is expressed
through dialect presets (only the CODESYS/Beckhoff-facing dialect bundles
`REFERENCE TO`, and no dialect bundles both). Coexistence stays well-defined
because the `RefSyntax` tag makes dereference behavior per-declaration — the
PR-2 implicit-dereference transform keys on `RefSyntax::ReferenceTo`, so `REF_TO`
variables are never implicitly dereferenced even when both flags are set.

The work is delivered in **two phases, one PR each**:

- **PR 1 — Front end & binding.** Flag, lexer keyword, parser productions for
  the `REFERENCE TO` type constructor and the `REF=` binding operator, AST
  tagging so the two syntaxes round-trip distinctly, and reuse of the entire
  existing `REF_TO` analyzer/codegen/VM backend. Access in this phase is via the
  existing explicit `^` operator (enough to prove end-to-end execution).
- **PR 2 — Implicit dereference & TwinCAT-faithful semantics.** An analyzer
  transform that makes bare uses of a `REFERENCE`-typed variable behave as an
  automatic dereference, plus `__ISVALIDREF` (gated behind `allow_reference_to`),
  so real TwinCAT source executes without explicit `^`.

`__ISVALIDREF` is **gated behind the `allow_reference_to` flag** — it is
recognized as a builtin only when that flag is set. IronPLC has no standard
library yet, and even with one this function is meaningful only for
`REFERENCE TO` (it reports whether a reference is bound). Gating it keeps it from
leaking into standard or `REF_TO`-only programs as a magic name.

## Status & handoff notes

- **Nothing is implemented yet.** The only artifacts committed so far are this
  plan and [ADR-0038](../adrs/0038-no-restrictions-on-flag-combinations.md). The
  design doc `specs/design/reference-to-twincat.md` is **not yet written** — it is
  the first task of PR 1, and until it exists the traceability tables in this plan
  are the source of truth for the REQ-RTO set.
- **Rebase onto `main` first.** This branch was cut before #1210/#1211 merged and
  is a few commits behind. The `parser.rs` line anchors (`:442` type decl,
  `:554` `array_specification`, `:860` var-init) and `compile_array.rs:31`
  (`ArraySpec.ref_to`) are **guides that will have drifted** — re-locate them by
  symbol, not line number, after rebasing.
- **ADR numbering:** ours is **ADR-0038**; `0037` is taken by the crate-slug ADR
  (`0037-mandatory-crate-slug-in-requirement-ids.md`) that landed with #1211.
- **Auto-wired surfaces (no per-flag code):** MCP applies flags via
  `set_flag_by_key`, and the playground via `FEATURE_DESCRIPTORS`, so both pick up
  `allow_reference_to` automatically (playground exposes it in its allow-list, not
  on by default). **LSP is manual** — add the `allowReferenceTo` line in
  `extract_compiler_options`.
- **Spec harness:** only `codegen` has it today; `parser`, `analyzer`, and
  `plc2plc` each need the one-time bootstrap (see enforcement approach) before
  they can host their slug's `#[spec_test]`s.

## Architecture

The key insight is that the reference **backend is already complete and is
representation-agnostic**: references are type-erased to `u64` variable-table
indices, codegen emits `LOAD_INDIRECT`/`STORE_INDIRECT`, and the VM traps on
null. None of that cares about the surface keyword. Therefore `REFERENCE TO`
maps onto the existing AST (`ReferenceDeclaration`, `ReferenceInitializer`,
`IntermediateType::Reference`, `ExprKind::{Ref, Deref, Null}`) and needs **no
new backend**.

All new work is confined to the front end plus one analyzer transform:

1. **Gating** follows the established token-demotion pattern. A new
   `REFERENCE` keyword token is demoted to `Identifier` unless
   `--allow-reference-to` is set — exactly how `REF_TO`/`REF`/`NULL` are demoted
   today in `xform_demote_edition3_keywords.rs`. The always-present grammar
   productions simply never fire when the keyword is demoted.
2. **No combination restriction.** The compiler does not reject enabling both
   `--allow-reference-to` and `--allow-ref-to`/Edition 3 (see
   [ADR-0038](../adrs/0038-no-restrictions-on-flag-combinations.md)). The two
   syntaxes coexist unambiguously because dereference behavior is keyed on each
   declaration's `RefSyntax` tag, not on flag state. Preference is expressed by
   dialect presets, not validation — the CODESYS-facing dialect bundles
   `REFERENCE TO`; no dialect bundles both.
3. **AST tagging.** `ReferenceDeclaration` and `ReferenceInitializer` currently
   render a hard-coded `REF_TO` (see `plc2plc/src/renderer.rs`). Since both
   syntaxes share these nodes, add a small `syntax: RefSyntax` discriminant so
   the renderer can reproduce the original keyword and binding operator. This is
   the only AST change.
4. **Implicit dereference (PR 2)** is a post-type-resolution analyzer transform
   that wraps reads of `REFERENCE`-typed variables in `ExprKind::Deref` and sets
   `deref: true` on assignment targets, skipping the contexts that must *not*
   auto-deref (the `REF=` target, an `__ISVALIDREF` argument). Everything
   downstream is unchanged.

### Design doc references

- [specs/design/ref-to.md](../design/ref-to.md) — the reference backend this
  plan reuses wholesale (tokens, AST, `IntermediateType::Reference`,
  `LOAD_INDIRECT`/`STORE_INDIRECT`, V4004 null trap).
- [specs/design/beckhoff-twincat-dialect.md](../design/beckhoff-twincat-dialect.md)
  — §2.1 and §3.6 sketch `REFERENCE TO` and `REF=`. **Note the divergence:** that
  document treats `REFERENCE TO` as parse-only (a separate `TypeSpec::ReferenceTo`
  reported as unsupported via P9004) under the `codesys` *dialect*. This plan
  instead introduces a standalone `--allow-reference-to` flag that *reuses the
  `REF_TO` backend* to produce executable code.
- **New:** [specs/design/reference-to-twincat.md](../design/reference-to-twincat.md)
  — **not yet written**; authored as the first task of PR 1. Holds the slugged
  `**REQ-RTO-<slug>-NNN**` requirement markers for both phases and the
  requirements→test traceability table. Per the
  design-requirement standard, every testable claim carries a REQ ID and every
  REQ ID has a corresponding spec-linked test (see below). This doc also
  reconciles the divergence from the dialect design above (supersedes
  `beckhoff-twincat-dialect.md` §2.1/§3.6).
- **New:** [ADR-0038](../adrs/0038-no-restrictions-on-flag-combinations.md) —
  records the decision *not* to reject `--allow-reference-to` +
  `--allow-ref-to`/Edition 3 combinations; flags are freely composable and
  dialects express the preferred combinations.

### Spec conformance & requirements traceability

Every testable claim in `reference-to-twincat.md` gets a `**REQ-RTO-<slug>-NNN**`
marker, and every marker has a spec-linked conformance test. `RTO` is an unused
area prefix (existing prefixes: CF, IS, EN, PAB, STL, TOL, ARC, DST, SR, TL, VC).

**Enforcement approach — formal, cross-crate `#[spec_test]`.** The cross-crate
mechanism landed in [#1211](https://github.com/ironplc/ironplc/pull/1211)
(mandatory crate slug in requirement IDs;
[ADR-0037](../adrs/0037-mandatory-crate-slug-in-requirement-ids.md)), which
resolves the gap tracked by #1210. This feature uses it directly — no workarounds.

How it works for this feature:

- **Slugged IDs.** Each requirement's ID carries the owning crate's slug:
  `REQ-RTO-<slug>-NNN`, where the slug is `CARGO_PKG_NAME` minus the `ironplc-`
  prefix (`parser`, `analyzer`, `codegen`, `plc2plc`). The `#[spec_test]`
  attribute uses the underscore form, e.g. `#[spec_test(REQ_RTO_codegen_400)]`.
- **Single design doc.** One `reference-to-twincat.md` holds **all** REQ-RTO
  markers (0xx–6xx). Requirements are grouped by owning crate via their slug.
- **Each participating crate lists the doc.** `parser`, `analyzer`, `codegen`,
  and `plc2plc` each add `reference-to-twincat.md` to their
  `spec_requirements_gen::generate([...])` call in `build.rs`. Only `codegen`
  has the spec-conformance harness today, so `parser`, `analyzer`, and `plc2plc`
  each need a one-time bootstrap (`build.rs` + the `ironplc-spec-requirements-gen`
  build-dep and `spec_test_macro` dep + `mod spec_requirements { include!(…) }`
  in `lib.rs` + a `spec_conformance.rs` with the meta-test). A crate is
  accountable only for the requirements whose slug matches its own; each crate's
  `all_spec_requirements_have_tests` meta-test enforces its subset independently.

Test **function** names keep the `{area}_spec_req_rto_{nnn}_{description}`
convention (no slug in the fn name, matching the migrated `REQ-EN-codegen-*`
tests); the slug lives in the requirement ID / `#[spec_test]` attribute. The
"Crate" column in the traceability tables below equals the ID's slug — where each
`#[spec_test]` lives.

## File map

### PR 1 — Front end & binding

| File | Change |
|------|--------|
| `specs/adrs/0038-no-restrictions-on-flag-combinations.md` | **New** — records that the compiler does not restrict `--allow-*` flag combinations; preference is expressed through dialect presets. |
| `specs/design/reference-to-twincat.md` | **New** — single design doc with **all** `**REQ-RTO-<slug>-NNN**` markers (slugs: `parser`/`analyzer`/`codegen`/`plc2plc`) and the traceability table; supersedes `beckhoff-twincat-dialect.md` §2.1/§3.6; references ADR-0038. Committed first, before implementation code. (PR 1 authors the 0xx–4xx + 6xx requirements; PR 2 adds 5xx.) |
| `compiler/{parser,analyzer,codegen,plc2plc}/build.rs` | Add `reference-to-twincat.md` to each crate's `spec_requirements_gen::generate([...])` list. `codegen` already has `build.rs`; **bootstrap** the harness in `parser`, `analyzer`, `plc2plc` (build.rs + `ironplc-spec-requirements-gen`/`spec_test_macro` deps + `mod spec_requirements` include + `spec_conformance.rs` meta-test). Uses the crate-slug mechanism from #1211. |
| `compiler/parser/src/options.rs` | New `allow_reference_to` field via `define_compiler_options!`; add to `Codesys` dialect preset; **not** `Rusty` (Rusty already carries `REF_TO`). Dialect tests (REQ-RTO-parser-001/002). |
| `compiler/ironplc-cli/bin/main.rs` | `--allow-reference-to` clap arg; `|=` overlay in `compiler_options()`. No combination validation (ADR-0038). |
| `compiler/ironplc-cli/src/lsp.rs` | `allowReferenceTo` extraction in `extract_compiler_options()`; test. |
| `compiler/mcp/src/tools/common.rs` | **No code change** — the MCP option layer applies flags generically via `CompilerOptions::set_flag_by_key`, so `allow_reference_to` is picked up automatically from `FEATURE_DESCRIPTORS`. (Add a test only if desired.) |
| `compiler/parser/src/token.rs` | New `#[token("REFERENCE", ignore(case))] Reference`; `describe()` arm `"'REFERENCE'"`; lexer test. (`TO` already exists.) |
| `compiler/parser/src/xform_demote_reference_keyword.rs` | **New** — demote `Reference` → `Identifier` when `!allow_reference_to`. Separate module because it is vendor-flag-gated, not edition-gated. |
| `compiler/parser/src/lib.rs` | Register the new demotion transform in `tokenize_program()` before `check_tokens()`/`parse_library()`. |
| `compiler/dsl/src/common.rs` | Add `RefSyntax { RefTo, ReferenceTo }` enum (same derives as other leaf AST enums — `Debug, Clone, PartialEq, Recurse`); add `syntax: RefSyntax` field to `ReferenceDeclaration` and `ReferenceInitializer`. Change **only the DSL** `ArraySubranges.ref_to: bool` → `ref_to: Option<RefSyntax>` (`None` = non-reference element; `Some(_)` = reference element, tagged with its surface syntax) so `ARRAY [..] OF REFERENCE TO T` round-trips distinctly from `ARRAY [..] OF REF_TO T`. |
| `compiler/parser/src/parser.rs` | (a) `REFERENCE TO` productions paralleling the `RefTo` productions at `parser.rs:442` (type decl) and `:860` (var init decl), tagging nodes `RefSyntax::ReferenceTo`; existing `REF_TO` productions tag `RefSyntax::RefTo`. (b) **Array elements:** extend `array_specification` (`parser.rs:554`) so the element type accepts `REFERENCE TO` as well as the existing `REF_TO`, recording the `RefSyntax` in `ArraySubranges.ref_to`. `REFERENCE TO ARRAY[..] OF T` (reference *to* an array) reuses `ref_to_target`'s existing `ReferenceTarget::Array` arm and comes for free. (c) `REF=` binding operator in assignment/statement context: recognize `Identifier("REF") + Equal` after the LHS and lower to the existing reference-assignment (`ExprKind::Ref`) form. |
| `compiler/plc2plc/src/renderer.rs` | `visit_reference_declaration` / `visit_reference_initializer` emit `REFERENCE TO` (and `REF=`) when `syntax == ReferenceTo`, else `REF_TO`; array-element rendering follows `ArraySubranges.ref_to`'s tag. |
| `compiler/analyzer/src/intermediates/array.rs` | Reads `ArraySubranges.ref_to` (the resolve site) and constructs the analyzer's own array intermediate, which keeps a **`ref_to: bool`** field. Populate it from the DSL via `subranges.ref_to.is_some()` — the syntax tag is only needed up to the renderer, not in the type system. Update the several `ref_to: false/true` literal constructions in this file's tests to the bool value they already use (unchanged). |
| `compiler/codegen/src/compile_array.rs` | The codegen `ArraySpec` struct (`compile_array.rs:31`) also keeps **`ref_to: bool`**; set it from the intermediate's bool. The `if spec.ref_to` / `if subranges.ref_to` read sites stay bool-valued. Only the DSL→intermediate boundary changes (`.is_some()`); reference-array codegen behavior is unchanged (already handled for `REF_TO`). |
| `compiler/resources/test/reference_to.st` | **New** — `REFERENCE TO` declarations, `REF=` binding, explicit `^` access, and `ARRAY [..] OF REFERENCE TO T`. |
| `compiler/plc2plc/resources/test/reference_to_rendered.st` | **New** — expected round-trip output. |
| `compiler/plc2plc/src/tests.rs` | Round-trip test using `CompilerOptions { allow_reference_to: true, .. }`. |
| `compiler/parser/src/tests.rs` | Keyword-safety regression (`REFERENCE` as identifier in standard mode); parser tests for the new productions incl. array-of-reference. |
| `compiler/codegen/tests/it/end_to_end_reference_to.rs` | **New** — bind via `REF=`, read/write via explicit `^`, verify values; array-of-reference element access (parallels the existing `end_to_end_array_ref_to.rs`). Proves backend reuse. |
| `docs/explanation/enabling-dialects-and-features.rst` | Document `--allow-reference-to`, noting it is the TwinCAT/CODESYS alternative to `--allow-ref-to` and that dialects (not a compiler restriction) express the preferred combination (ADR-0038). |
| `docs/reference/compiler/ironplcc.rst` | Add the flag to the Options section. |
| `docs/reference/language/data-types/derived/reference-types.rst` | Note the TwinCAT variant. |
| `specs/steering/syntax-support-guide.md` | Add `--allow-reference-to` to the flag table. |

### PR 2 — Implicit dereference & semantics

| File | Change |
|------|--------|
| `specs/design/reference-to-twincat.md` | Extend with the implicit-dereference 5xx markers (`REQ-RTO-codegen-5xx` / `REQ-RTO-analyzer-5xx`, incl. `__ISVALIDREF` gating) and their traceability rows. |
| `compiler/analyzer/src/xform_insert_implicit_deref.rs` | **New** — post-type-resolution transform that wraps reads of `REFERENCE`-typed variables in `ExprKind::Deref` and sets `deref: true` on assignment targets, skipping `REF=` targets and `__ISVALIDREF` arguments. |
| `compiler/analyzer/src/stages.rs` | Wire the transform into the pipeline after type resolution, before codegen. |
| `compiler/analyzer/src/rule_ref_to.rs` | Review which rules apply under implicit-deref semantics. Notably, arithmetic/ordering that `REF_TO` rejects (P2033/P2035) is *legal* for `REFERENCE` because it operates on the dereferenced value — ensure the transform runs first so these rules see `ref^`, not `ref`. Gate any `REF_TO`-only checks on `RefSyntax`. |
| `compiler/analyzer/src/…` (builtins) | `__ISVALIDREF(ref)` → lower to `ref <> NULL` (reuses existing null comparison). **Gated behind `allow_reference_to`**: recognized as a builtin only when the flag is set; otherwise it stays an ordinary (unresolved) identifier. |
| `compiler/codegen/tests/it/end_to_end_reference_to.rs` | Extend: bare implicit read/write (`ref := 5;`, `y := ref;`), aliasing, `__ISVALIDREF`, uninitialized-reference trap. |
| `compiler/parser` / analyzer tests | Cover the non-deref contexts (REF= target, `__ISVALIDREF` arg are not auto-dereffed). |
| `docs/reference/language/data-types/derived/reference-types.rst` | Document the implicit-dereference access model. |

## Tasks

### Phase 1 (PR 1): Front end & binding

- [ ] **Write `specs/adrs/0038-no-restrictions-on-flag-combinations.md`** recording that the compiler does not restrict `--allow-*` combinations; commit before implementation code.
- [ ] **Author `specs/design/reference-to-twincat.md`** — a single design doc with **all** slugged `**REQ-RTO-<slug>-NNN**` markers (0xx–4xx + 6xx here; PR 2 adds 5xx) and the traceability table below; reference ADR-0038; commit it *before* implementation code (Planning/Design-requirement standard).
- [ ] Add `reference-to-twincat.md` to `codegen/build.rs`'s `generate([...])`, and bootstrap the spec-conformance harness in `parser`, `analyzer`, and `plc2plc` (build.rs + deps + `mod spec_requirements` include + `spec_conformance.rs` meta-test) so each can host its slug's `#[spec_test(REQ_RTO_<slug>_NNN)]` tests. Uses the crate-slug mechanism from #1211.
- [ ] Add `allow_reference_to` to `define_compiler_options!` (`options.rs`); enable it in the `Codesys` dialect preset, not `Rusty`; update the `from_dialect` / `FEATURE_DESCRIPTORS` count tests. Tests: `options_spec_req_rto_001_codesys_enables_reference_to`, `options_spec_req_rto_002_rusty_does_not_enable_reference_to`, `options_spec_req_rto_003_reference_to_and_ref_to_coexist` (both flags set is accepted — ADR-0038).
- [ ] Add `--allow-reference-to` clap arg and `|=` overlay in `ironplc-cli/bin/main.rs`. No combination validation (ADR-0038).
- [ ] Add `allowReferenceTo` to LSP `extract_compiler_options()` + test.
- [ ] MCP: no code change needed — `set_flag_by_key` picks up `allow_reference_to` automatically. (Optional: add an MCP test asserting the key is accepted.)
- [ ] Add the `REFERENCE` token to `token.rs` (+ `describe()` arm); confirm `TO` tokenizes separately. Test: `lexer_spec_req_rto_100_reference_lexes_as_reference_token`.
- [ ] Create `xform_demote_reference_keyword.rs` (demote when `!allow_reference_to`); register it in `lib.rs` `tokenize_program()`. Tests: `xform_spec_req_rto_101_reference_demoted_when_flag_off`, `xform_spec_req_rto_102_reference_kept_when_flag_on`.
- [ ] Add `RefSyntax { RefTo, ReferenceTo }` and the `syntax` field to `ReferenceDeclaration` / `ReferenceInitializer` (`dsl/src/common.rs`); update all constructors/pattern matches (parser, renderer, analyzer, codegen) to set/handle the tag; existing `REF_TO` paths set `RefSyntax::RefTo`. Test: `parser_spec_req_rto_202_ref_to_is_tagged_ref_to`.
- [ ] Add `REFERENCE TO` parser productions (type decl + var init) tagging `RefSyntax::ReferenceTo`. Tests: `parser_spec_req_rto_200_reference_to_var_decl_is_tagged`, `parser_spec_req_rto_201_reference_to_type_decl_is_tagged`.
- [ ] Support `ARRAY [..] OF REFERENCE TO T`: change the **DSL** `ArraySubranges.ref_to: bool` → `Option<RefSyntax>`, extend `array_specification` to accept `REFERENCE TO` in the element type, and collapse to `bool` via `.is_some()` at the DSL→intermediate boundary (`analyzer/src/intermediates/array.rs`) and intermediate→codegen `ArraySpec` (`compile_array.rs`); renderer reads the DSL tag. Tests: `parser_spec_req_rto_220_array_of_reference_to_is_tagged`, `codegen_spec_req_rto_420_array_of_reference_element_access`.
- [ ] Add the `REF=` binding operator in assignment context, lowering to the existing `ExprKind::Ref` reference-assignment. Test: `parser_spec_req_rto_210_ref_assign_parses_as_reference_binding`.
- [ ] Update the renderer to emit `REFERENCE TO` / `REF=` based on `RefSyntax`. Tests: `plc2plc_spec_req_rto_600_reference_to_declaration_renders`, `plc2plc_spec_req_rto_601_ref_assign_renders`, `plc2plc_spec_req_rto_602_ref_to_still_renders`.
- [ ] Keyword-safety regression: `REFERENCE` usable as an identifier in standard mode. Test: `parser_spec_req_rto_103_reference_is_identifier_in_standard_mode`.
- [ ] Add reference type resolution + bind type-check tests. Tests: `analyzer_spec_req_rto_300_reference_to_resolves_to_reference_type`, `analyzer_spec_req_rto_301_reference_bind_type_mismatch_is_rejected`.
- [ ] Add plc2plc round-trip fixtures (`reference_to.st` → `reference_to_rendered.st`) and the round-trip test.
- [ ] Add end-to-end execution tests (bind via `REF=`, access via explicit `^`). Tests: `codegen_spec_req_rto_400_read_through_reference`, `codegen_spec_req_rto_401_write_through_reference`, `codegen_spec_req_rto_402_unbound_reference_deref_traps`.
- [ ] Update docs (`enabling-dialects-and-features.rst`, `ironplcc.rst`, `reference-types.rst`) and the flag table in `syntax-support-guide.md`.
- [ ] `cd compiler && just` — all checks (compile, coverage ≥85%, clippy, fmt) pass.

### Phase 2 (PR 2): Implicit dereference & semantics

- [ ] Extend `specs/design/reference-to-twincat.md` with the 5xx markers (`REQ-RTO-codegen-5xx` / `REQ-RTO-analyzer-5xx`: implicit-deref behavior + non-deref contexts) and their traceability rows; commit before implementation code.
- [ ] Implement `xform_insert_implicit_deref.rs`: wrap reads of `REFERENCE`-typed variables in `Deref`; set `deref: true` on `REFERENCE`-typed assignment targets; skip `REF=` targets and `__ISVALIDREF` arguments.
- [ ] Wire the transform into `stages.rs` after type resolution and before codegen.
- [ ] Reconcile `rule_ref_to.rs`: ensure the transform runs before the rules so arithmetic/ordering on `REFERENCE` values (legal, because auto-dereferenced) is not wrongly rejected; gate any `REF_TO`-only checks on `RefSyntax`. Test: `codegen_spec_req_rto_510_arithmetic_on_reference_uses_deref_value`.
- [ ] Add `__ISVALIDREF(ref)` lowering to `ref <> NULL`, **gated behind `allow_reference_to`** (recognized as a builtin only when the flag is set; otherwise it remains an ordinary identifier). Tests: `codegen_spec_req_rto_503_isvalidref_reflects_binding`, `analyzer_spec_req_rto_505_isvalidref_not_recognized_without_flag`.
- [ ] Extend end-to-end tests: bare implicit read/write and aliasing. Tests: `codegen_spec_req_rto_500_bare_read_auto_dereferences`, `codegen_spec_req_rto_501_bare_write_auto_dereferences`, `codegen_spec_req_rto_504_aliasing_observed_through_implicit_deref`.
- [ ] Prove the non-deref contexts are not auto-dereferenced. Test: `analyzer_spec_req_rto_502_ref_assign_target_is_not_dereferenced`.
- [ ] Update `reference-types.rst` with the implicit-dereference access model.
- [ ] `cd compiler && just` — all checks pass.

## Requirements traceability

The authoritative copy lives in `specs/design/reference-to-twincat.md`; this
table mirrors it so the plan is self-contained. Each REQ has a spec-linked test
named `{area}_spec_req_rto_{nnn}_{description}` (see enforcement approach above).

### PR 1

The requirement ID's slug names the crate that owns (hosts) the test; all
markers live in the single `reference-to-twincat.md`, enforced per-crate via the
slug mechanism from #1211 (see enforcement approach above). The `#[spec_test]`
attribute is the underscore form of the ID (e.g. `REQ_RTO_codegen_400`); the test
**function** name keeps the `{area}_spec_req_rto_{nnn}_{description}` convention.

| Req | Claim | Test fn | Crate |
|-----|-------|---------|-------|
| **REQ-RTO-parser-001** | The `codesys` dialect enables `allow_reference_to` | `options_spec_req_rto_001_*` | parser |
| **REQ-RTO-parser-002** | The `rusty` dialect does *not* enable `allow_reference_to` | `options_spec_req_rto_002_*` | parser |
| **REQ-RTO-parser-003** | Setting both `allow_reference_to` and `allow_ref_to` is accepted (no combination error; ADR-0038) | `options_spec_req_rto_003_*` | parser |
| **REQ-RTO-parser-100** | `REFERENCE` lexes as the `Reference` token | `lexer_spec_req_rto_100_*` | parser |
| **REQ-RTO-parser-101** | With the flag off, `REFERENCE` is demoted to `Identifier` | `xform_spec_req_rto_101_*` | parser |
| **REQ-RTO-parser-102** | With the flag on, `REFERENCE` stays the `Reference` keyword | `xform_spec_req_rto_102_*` | parser |
| **REQ-RTO-parser-103** | `REFERENCE` is a valid identifier in standard mode | `parser_spec_req_rto_103_*` | parser |
| **REQ-RTO-parser-200** | `r : REFERENCE TO INT;` yields a decl tagged `RefSyntax::ReferenceTo` | `parser_spec_req_rto_200_*` | parser |
| **REQ-RTO-parser-201** | `TYPE T : REFERENCE TO INT; END_TYPE` yields a decl tagged `ReferenceTo` | `parser_spec_req_rto_201_*` | parser |
| **REQ-RTO-parser-202** | `REF_TO` declarations are tagged `RefSyntax::RefTo` | `parser_spec_req_rto_202_*` | parser |
| **REQ-RTO-parser-210** | `r REF= x;` parses as a reference binding equivalent to `r := REF(x)` | `parser_spec_req_rto_210_*` | parser |
| **REQ-RTO-parser-220** | `ARRAY [..] OF REFERENCE TO T` parses and tags the element `RefSyntax::ReferenceTo` | `parser_spec_req_rto_220_*` | parser |
| **REQ-RTO-analyzer-300** | `REFERENCE TO T` resolves to `IntermediateType::Reference` | `analyzer_spec_req_rto_300_*` | analyzer |
| **REQ-RTO-analyzer-301** | Binding a reference to a mismatched target type is rejected (P2032) | `analyzer_spec_req_rto_301_*` | analyzer |
| **REQ-RTO-codegen-400** | Reading a `REF=`-bound reference via `^` yields the referenced value | `codegen_spec_req_rto_400_*` | codegen |
| **REQ-RTO-codegen-401** | Writing through `^` stores to the referenced variable | `codegen_spec_req_rto_401_*` | codegen |
| **REQ-RTO-codegen-402** | Dereferencing an unbound `REFERENCE TO` variable traps `NullDereference` | `codegen_spec_req_rto_402_*` | codegen |
| **REQ-RTO-codegen-420** | An `ARRAY [..] OF REFERENCE TO T` element can be bound and accessed | `codegen_spec_req_rto_420_*` | codegen |
| **REQ-RTO-plc2plc-600** | A `ReferenceTo`-tagged declaration renders as `REFERENCE TO <target>` | `plc2plc_spec_req_rto_600_*` | plc2plc |
| **REQ-RTO-plc2plc-601** | A `REF=` binding renders back as `REF=` | `plc2plc_spec_req_rto_601_*` | plc2plc |
| **REQ-RTO-plc2plc-602** | A `RefTo`-tagged declaration still renders as `REF_TO` (regression) | `plc2plc_spec_req_rto_602_*` | plc2plc |

### PR 2

| Req | Claim | Test fn | Crate |
|-----|-------|---------|-------|
| **REQ-RTO-codegen-500** | A bare read of a `REFERENCE`-typed variable auto-dereferences | `codegen_spec_req_rto_500_*` | codegen |
| **REQ-RTO-codegen-501** | A bare write to a `REFERENCE`-typed variable auto-dereferences | `codegen_spec_req_rto_501_*` | codegen |
| **REQ-RTO-analyzer-502** | The target of `REF=` is *not* auto-dereferenced | `analyzer_spec_req_rto_502_*` | analyzer |
| **REQ-RTO-codegen-503** | `__ISVALIDREF(r)` is FALSE for an unbound reference, TRUE once bound | `codegen_spec_req_rto_503_*` | codegen |
| **REQ-RTO-codegen-504** | Two references to one variable observe each other's writes | `codegen_spec_req_rto_504_*` | codegen |
| **REQ-RTO-analyzer-505** | `__ISVALIDREF` is recognized as a builtin only when `allow_reference_to` is set | `analyzer_spec_req_rto_505_*` | analyzer |
| **REQ-RTO-codegen-510** | Arithmetic on a bare `REFERENCE` operand uses the dereferenced value | `codegen_spec_req_rto_510_*` | codegen |

## Out of scope

- `POINTER TO` and the `ADR()`/`^` pointer model (a separate, explicitly-dereferenced type; its own future flag).
- `S=` / `R=` extended assignment operators.
- Any TwinCAT OOP features (methods, properties, interfaces) — tracked in `beckhoff-twincat-dialect.md`.

(`ARRAY [..] OF REFERENCE TO T` is **in scope for PR 1** — see the file map and
tasks — so array-typed reference use is not limited.)
