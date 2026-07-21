# Plan: TwinCAT `REFERENCE TO` Keyword Support

## Goal

Add support for Beckhoff TwinCAT / CODESYS `REFERENCE TO` reference types as a
distinct, separately-flagged alternative to the existing IEC 61131-3 `REF_TO`
syntax. The two are surface-level variants of the same underlying concept
(a strongly-typed reference implemented as a variable-table index), but their
*usage models differ* and they are mutually exclusive:

| Concern | IEC `REF_TO` (`--allow-ref-to`) | TwinCAT `REFERENCE TO` (`--allow-reference-to`) |
|---------|--------------------------------|------------------------------------------------|
| Declare | `r : REF_TO INT;` | `r : REFERENCE TO INT;` |
| Bind    | `r := REF(x);` | `r REF= x;` |
| Read    | `y := r^;` (explicit `^`) | `y := r;` (implicit dereference) |
| Write   | `r^ := 5;` (explicit `^`) | `r := 5;` (implicit dereference) |
| Validity| `r = NULL` | `__ISVALIDREF(r)` / `r = 0` |

The work is delivered in **two phases, one PR each**:

- **PR 1 — Front end & binding.** Flag, lexer keyword, parser productions for
  the `REFERENCE TO` type constructor and the `REF=` binding operator, AST
  tagging so the two syntaxes round-trip distinctly, and reuse of the entire
  existing `REF_TO` analyzer/codegen/VM backend. Access in this phase is via the
  existing explicit `^` operator (enough to prove end-to-end execution).
- **PR 2 — Implicit dereference & TwinCAT-faithful semantics.** An analyzer
  transform that makes bare uses of a `REFERENCE`-typed variable behave as an
  automatic dereference, plus `__ISVALIDREF`, so real TwinCAT source executes
  without explicit `^`.

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
2. **Mutual exclusivity** is enforced as an options-validation error: enabling
   both `--allow-reference-to` and `--allow-ref-to` (or Edition 3) is rejected,
   because the two dereference models (`REF()`/`^` vs `REF=`/implicit) make a
   bare reference ambiguous.
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
  — authored as the first task of PR 1. Holds the `**REQ-RTO-NNN**` requirement
  markers for both phases and the requirements→test traceability table. Per the
  design-requirement standard, every testable claim carries a REQ ID and every
  REQ ID has a corresponding spec-linked test (see below). This doc also
  reconciles the divergence from the dialect design above (supersedes
  `beckhoff-twincat-dialect.md` §2.1/§3.6).

### Spec conformance & requirements traceability

Every testable claim in `reference-to-twincat.md` gets a `**REQ-RTO-NNN**`
marker, and every marker has a spec-linked conformance test. `RTO` is an unused
prefix (existing prefixes: CF, IS, EN, PAB, STL, TOL, ARC, DST, SR, TL, VC).

**Enforcement approach.** This feature spans the `parser`, `dsl`, `analyzer`,
`codegen`, and `plc2plc` crates. Formal `#[spec_test]` enforcement via
`ironplc_spec_requirements_gen::generate()` is **per-crate** — each crate's
`build.rs` computes its `UNTESTED` set against *its own* `src/` + `tests/`, so a
single design doc can only be completeness-enforced by one crate. The closest
precedent, REQ-PAB (partial-access, also a cross-crate language feature), chose
the **naming-convention + design-doc traceability table** approach for exactly
this reason. This plan follows that precedent as the baseline:

- **Baseline (all REQ-RTO):** each requirement has a test named
  `{area}_spec_req_rto_{nnn}_{description}` (e.g.
  `parser_spec_req_rto_200_reference_to_var_decl_is_tagged`), living in its
  natural crate. The design doc's traceability table is the authoritative
  REQ→test map, kept in sync manually (the same guarantee REQ-PAB relies on).
- **Optional formal upgrade (codegen-hosted subset):** the `codegen` crate
  *already* wires `ironplc_spec_requirements_gen::generate()` and its generator
  scans `tests/`. The execution requirements (REQ-RTO-4xx/5xx) are reachable
  from `codegen` (it depends on `analyzer`→`parser`→`dsl` and the VM helpers).
  These MAY be upgraded to true `#[spec_test(REQ_RTO_NNN)]` enforcement by
  adding `reference-to-twincat.md` to `codegen/build.rs`'s `generate([...])`
  list and annotating the e2e tests in `codegen/src/spec_conformance.rs`. If
  this upgrade is taken, **all** REQ-RTO markers in that doc must have a
  codegen-hosted `#[spec_test]` (or the `all_spec_requirements_have_tests`
  meta-test fails); since round-trip (6xx) and lexer/parser (1xx/2xx) tests
  live in other crates, keep those either (a) as naming-convention tests with
  their markers in a *separate* companion doc not parsed by `codegen`, or
  (b) accept the baseline approach for the whole feature. **Recommendation:
  baseline for the whole feature** (matches REQ-PAB, no build-graph changes);
  revisit formal enforcement once the feature stabilizes.

## File map

### PR 1 — Front end & binding

| File | Change |
|------|--------|
| `specs/design/reference-to-twincat.md` | **New** — design doc with `**REQ-RTO-NNN**` markers (0xx–4xx, 6xx) and the requirements→test traceability table; supersedes `beckhoff-twincat-dialect.md` §2.1/§3.6. Committed first, before implementation code. |
| `compiler/parser/src/options.rs` | New `allow_reference_to` field via `define_compiler_options!`; add to `Codesys` dialect preset; **not** `Rusty` (Rusty already carries `REF_TO`). Add mutual-exclusivity note. Dialect tests (REQ-RTO-001/002). |
| `compiler/ironplc-cli/bin/main.rs` | `--allow-reference-to` clap arg; `|=` overlay in `compiler_options()`; mutual-exclusivity validation error when combined with `--allow-ref-to`/Edition 3. |
| `compiler/ironplc-cli/src/lsp.rs` | `allowReferenceTo` extraction in `extract_compiler_options()`; test. |
| `compiler/mcp/src/tools/common.rs` | Expose the new option key (mirrors other `allow_*` flags). |
| `compiler/parser/src/token.rs` | New `#[token("REFERENCE", ignore(case))] Reference`; `describe()` arm `"'REFERENCE'"`; lexer test. (`TO` already exists.) |
| `compiler/parser/src/xform_demote_reference_keyword.rs` | **New** — demote `Reference` → `Identifier` when `!allow_reference_to`. Separate module because it is vendor-flag-gated, not edition-gated. |
| `compiler/parser/src/lib.rs` | Register the new demotion transform in `tokenize_program()` before `check_tokens()`/`parse_library()`. |
| `compiler/dsl/src/common.rs` | Add `RefSyntax { RefTo, ReferenceTo }` enum; add `syntax: RefSyntax` field to `ReferenceDeclaration` and `ReferenceInitializer`. |
| `compiler/parser/src/parser.rs` | (a) `REFERENCE TO` productions paralleling the `RefTo` productions at `parser.rs:442` (type decl) and `:860` (var init decl), tagging nodes `RefSyntax::ReferenceTo`; existing `REF_TO` productions tag `RefSyntax::RefTo`. (b) `REF=` binding operator in assignment/statement context: recognize `Identifier("REF") + Equal` after the LHS and lower to the existing reference-assignment (`ExprKind::Ref`) form. |
| `compiler/plc2plc/src/renderer.rs` | `visit_reference_declaration` / `visit_reference_initializer` emit `REFERENCE TO` (and `REF=`) when `syntax == ReferenceTo`, else `REF_TO`. |
| `compiler/resources/test/reference_to.st` | **New** — `REFERENCE TO` declarations, `REF=` binding, explicit `^` access. |
| `compiler/plc2plc/resources/test/reference_to_rendered.st` | **New** — expected round-trip output. |
| `compiler/plc2plc/src/tests.rs` | Round-trip test using `CompilerOptions { allow_reference_to: true, .. }`. |
| `compiler/parser/src/tests.rs` | Keyword-safety regression (`REFERENCE` as identifier in standard mode); parser tests for the new productions. |
| `compiler/codegen/tests/it/end_to_end_reference_to.rs` | **New** — bind via `REF=`, read/write via explicit `^`, verify values (proves backend reuse). |
| `docs/explanation/enabling-dialects-and-features.rst` | Document `--allow-reference-to` and its exclusivity with `--allow-ref-to`. |
| `docs/reference/compiler/ironplcc.rst` | Add the flag to the Options section. |
| `docs/reference/language/data-types/derived/reference-types.rst` | Note the TwinCAT variant. |
| `specs/steering/syntax-support-guide.md` | Add `--allow-reference-to` to the flag table. |

### PR 2 — Implicit dereference & semantics

| File | Change |
|------|--------|
| `specs/design/reference-to-twincat.md` | Extend with the implicit-dereference `**REQ-RTO-5xx**` markers and their traceability rows. |
| `compiler/analyzer/src/xform_insert_implicit_deref.rs` | **New** — post-type-resolution transform that wraps reads of `REFERENCE`-typed variables in `ExprKind::Deref` and sets `deref: true` on assignment targets, skipping `REF=` targets and `__ISVALIDREF` arguments. |
| `compiler/analyzer/src/stages.rs` | Wire the transform into the pipeline after type resolution, before codegen. |
| `compiler/analyzer/src/rule_ref_to.rs` | Review which rules apply under implicit-deref semantics. Notably, arithmetic/ordering that `REF_TO` rejects (P2033/P2035) is *legal* for `REFERENCE` because it operates on the dereferenced value — ensure the transform runs first so these rules see `ref^`, not `ref`. Gate any `REF_TO`-only checks on `RefSyntax`. |
| `compiler/analyzer/src/…` (builtins) | `__ISVALIDREF(ref)` → lower to `ref <> NULL` (reuses existing null comparison). |
| `compiler/codegen/tests/it/end_to_end_reference_to.rs` | Extend: bare implicit read/write (`ref := 5;`, `y := ref;`), aliasing, `__ISVALIDREF`, uninitialized-reference trap. |
| `compiler/parser` / analyzer tests | Cover the non-deref contexts (REF= target, `__ISVALIDREF` arg are not auto-dereffed). |
| `docs/reference/language/data-types/derived/reference-types.rst` | Document the implicit-dereference access model. |

## Tasks

### Phase 1 (PR 1): Front end & binding

- [ ] **Author `specs/design/reference-to-twincat.md`** with the `**REQ-RTO-NNN**` markers (0xx–4xx, 6xx) and the traceability table below; commit it *before* implementation code (Planning/Design-requirement standard).
- [ ] Add `allow_reference_to` to `define_compiler_options!` (`options.rs`); enable it in the `Codesys` dialect preset, not `Rusty`; update the `from_dialect` / `FEATURE_DESCRIPTORS` count tests. Tests: `options_spec_req_rto_001_codesys_enables_reference_to`, `options_spec_req_rto_002_rusty_does_not_enable_reference_to`.
- [ ] Add `--allow-reference-to` clap arg and `|=` overlay in `ironplc-cli/bin/main.rs`.
- [ ] Add the mutual-exclusivity validation error (both `--allow-reference-to` and `--allow-ref-to`/Edition 3 set → reject with a clear message). Tests: `cli_spec_req_rto_010_reference_to_with_ref_to_is_rejected`, `cli_spec_req_rto_011_reference_to_with_edition3_is_rejected` (+ each-alone accepted).
- [ ] Add `allowReferenceTo` to LSP `extract_compiler_options()` + test.
- [ ] Expose the option key in `mcp/src/tools/common.rs`.
- [ ] Add the `REFERENCE` token to `token.rs` (+ `describe()` arm); confirm `TO` tokenizes separately. Test: `lexer_spec_req_rto_100_reference_lexes_as_reference_token`.
- [ ] Create `xform_demote_reference_keyword.rs` (demote when `!allow_reference_to`); register it in `lib.rs` `tokenize_program()`. Tests: `xform_spec_req_rto_101_reference_demoted_when_flag_off`, `xform_spec_req_rto_102_reference_kept_when_flag_on`.
- [ ] Add `RefSyntax { RefTo, ReferenceTo }` and the `syntax` field to `ReferenceDeclaration` / `ReferenceInitializer` (`dsl/src/common.rs`); update all constructors/pattern matches (parser, renderer, analyzer, codegen) to set/handle the tag; existing `REF_TO` paths set `RefSyntax::RefTo`. Test: `parser_spec_req_rto_202_ref_to_is_tagged_ref_to`.
- [ ] Add `REFERENCE TO` parser productions (type decl + var init) tagging `RefSyntax::ReferenceTo`. Tests: `parser_spec_req_rto_200_reference_to_var_decl_is_tagged`, `parser_spec_req_rto_201_reference_to_type_decl_is_tagged`.
- [ ] Add the `REF=` binding operator in assignment context, lowering to the existing `ExprKind::Ref` reference-assignment. Test: `parser_spec_req_rto_210_ref_assign_parses_as_reference_binding`.
- [ ] Update the renderer to emit `REFERENCE TO` / `REF=` based on `RefSyntax`. Tests: `plc2plc_spec_req_rto_600_reference_to_declaration_renders`, `plc2plc_spec_req_rto_601_ref_assign_renders`, `plc2plc_spec_req_rto_602_ref_to_still_renders`.
- [ ] Keyword-safety regression: `REFERENCE` usable as an identifier in standard mode. Test: `parser_spec_req_rto_103_reference_is_identifier_in_standard_mode`.
- [ ] Add reference type resolution + bind type-check tests. Tests: `analyzer_spec_req_rto_300_reference_to_resolves_to_reference_type`, `analyzer_spec_req_rto_301_reference_bind_type_mismatch_is_rejected`.
- [ ] Add plc2plc round-trip fixtures (`reference_to.st` → `reference_to_rendered.st`) and the round-trip test.
- [ ] Add end-to-end execution tests (bind via `REF=`, access via explicit `^`). Tests: `codegen_spec_req_rto_400_read_through_reference`, `codegen_spec_req_rto_401_write_through_reference`, `codegen_spec_req_rto_402_unbound_reference_deref_traps`.
- [ ] Update docs (`enabling-dialects-and-features.rst`, `ironplcc.rst`, `reference-types.rst`) and the flag table in `syntax-support-guide.md`.
- [ ] `cd compiler && just` — all checks (compile, coverage ≥85%, clippy, fmt) pass.

### Phase 2 (PR 2): Implicit dereference & semantics

- [ ] Extend `specs/design/reference-to-twincat.md` with the `**REQ-RTO-5xx**` markers (implicit-deref behavior + non-deref contexts) and their traceability rows; commit before implementation code.
- [ ] Implement `xform_insert_implicit_deref.rs`: wrap reads of `REFERENCE`-typed variables in `Deref`; set `deref: true` on `REFERENCE`-typed assignment targets; skip `REF=` targets and `__ISVALIDREF` arguments.
- [ ] Wire the transform into `stages.rs` after type resolution and before codegen.
- [ ] Reconcile `rule_ref_to.rs`: ensure the transform runs before the rules so arithmetic/ordering on `REFERENCE` values (legal, because auto-dereferenced) is not wrongly rejected; gate any `REF_TO`-only checks on `RefSyntax`. Test: `codegen_spec_req_rto_510_arithmetic_on_reference_uses_deref_value`.
- [ ] Add `__ISVALIDREF(ref)` lowering to `ref <> NULL`. Test: `codegen_spec_req_rto_503_isvalidref_reflects_binding`.
- [ ] Extend end-to-end tests: bare implicit read/write and aliasing. Tests: `codegen_spec_req_rto_500_bare_read_auto_dereferences`, `codegen_spec_req_rto_501_bare_write_auto_dereferences`, `codegen_spec_req_rto_504_aliasing_observed_through_implicit_deref`.
- [ ] Prove the non-deref contexts are not auto-dereferenced. Test: `analyzer_spec_req_rto_502_ref_assign_target_is_not_dereferenced`.
- [ ] Update `reference-types.rst` with the implicit-dereference access model.
- [ ] `cd compiler && just` — all checks pass.

## Requirements traceability

The authoritative copy lives in `specs/design/reference-to-twincat.md`; this
table mirrors it so the plan is self-contained. Each REQ has a spec-linked test
named `{area}_spec_req_rto_{nnn}_{description}` (see enforcement approach above).

### PR 1

| Req | Claim | Test | Crate |
|-----|-------|------|-------|
| **REQ-RTO-001** | The `codesys` dialect enables `allow_reference_to` | `options_spec_req_rto_001_*` | parser |
| **REQ-RTO-002** | The `rusty` dialect does *not* enable `allow_reference_to` | `options_spec_req_rto_002_*` | parser |
| **REQ-RTO-010** | Enabling both `--allow-reference-to` and `--allow-ref-to` is rejected | `cli_spec_req_rto_010_*` | ironplc-cli |
| **REQ-RTO-011** | Enabling both `--allow-reference-to` and Edition 3 is rejected | `cli_spec_req_rto_011_*` | ironplc-cli |
| **REQ-RTO-100** | `REFERENCE` lexes as the `Reference` token | `lexer_spec_req_rto_100_*` | parser |
| **REQ-RTO-101** | With the flag off, `REFERENCE` is demoted to `Identifier` | `xform_spec_req_rto_101_*` | parser |
| **REQ-RTO-102** | With the flag on, `REFERENCE` stays the `Reference` keyword | `xform_spec_req_rto_102_*` | parser |
| **REQ-RTO-103** | `REFERENCE` is a valid identifier in standard mode | `parser_spec_req_rto_103_*` | parser |
| **REQ-RTO-200** | `r : REFERENCE TO INT;` yields a decl tagged `RefSyntax::ReferenceTo` | `parser_spec_req_rto_200_*` | parser |
| **REQ-RTO-201** | `TYPE T : REFERENCE TO INT; END_TYPE` yields a decl tagged `ReferenceTo` | `parser_spec_req_rto_201_*` | parser |
| **REQ-RTO-202** | `REF_TO` declarations are tagged `RefSyntax::RefTo` | `parser_spec_req_rto_202_*` | parser |
| **REQ-RTO-210** | `r REF= x;` parses as a reference binding equivalent to `r := REF(x)` | `parser_spec_req_rto_210_*` | parser |
| **REQ-RTO-300** | `REFERENCE TO T` resolves to `IntermediateType::Reference` | `analyzer_spec_req_rto_300_*` | analyzer |
| **REQ-RTO-301** | Binding a reference to a mismatched target type is rejected (P2032) | `analyzer_spec_req_rto_301_*` | analyzer |
| **REQ-RTO-400** | Reading a `REF=`-bound reference via `^` yields the referenced value | `codegen_spec_req_rto_400_*` | codegen |
| **REQ-RTO-401** | Writing through `^` stores to the referenced variable | `codegen_spec_req_rto_401_*` | codegen |
| **REQ-RTO-402** | Dereferencing an unbound `REFERENCE TO` variable traps `NullDereference` | `codegen_spec_req_rto_402_*` | codegen |
| **REQ-RTO-600** | A `ReferenceTo`-tagged declaration renders as `REFERENCE TO <target>` | `plc2plc_spec_req_rto_600_*` | plc2plc |
| **REQ-RTO-601** | A `REF=` binding renders back as `REF=` | `plc2plc_spec_req_rto_601_*` | plc2plc |
| **REQ-RTO-602** | A `RefTo`-tagged declaration still renders as `REF_TO` (regression) | `plc2plc_spec_req_rto_602_*` | plc2plc |

### PR 2

| Req | Claim | Test | Crate |
|-----|-------|------|-------|
| **REQ-RTO-500** | A bare read of a `REFERENCE`-typed variable auto-dereferences | `codegen_spec_req_rto_500_*` | codegen |
| **REQ-RTO-501** | A bare write to a `REFERENCE`-typed variable auto-dereferences | `codegen_spec_req_rto_501_*` | codegen |
| **REQ-RTO-502** | The target of `REF=` is *not* auto-dereferenced | `analyzer_spec_req_rto_502_*` | analyzer |
| **REQ-RTO-503** | `__ISVALIDREF(r)` is FALSE for an unbound reference, TRUE once bound | `codegen_spec_req_rto_503_*` | codegen |
| **REQ-RTO-504** | Two references to one variable observe each other's writes | `codegen_spec_req_rto_504_*` | codegen |
| **REQ-RTO-510** | Arithmetic on a bare `REFERENCE` operand uses the dereferenced value | `codegen_spec_req_rto_510_*` | codegen |

## Out of scope

- `POINTER TO` and the `ADR()`/`^` pointer model (a separate, explicitly-dereferenced type; its own future flag).
- `S=` / `R=` extended assignment operators.
- `ARRAY [..] OF REFERENCE TO T` (can be added later alongside the existing `ARRAY OF REF_TO`).
- Any TwinCAT OOP features (methods, properties, interfaces) — tracked in `beckhoff-twincat-dialect.md`.
