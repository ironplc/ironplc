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
  `REF_TO` backend* to produce executable code. Before starting PR 2, reconcile
  the two: either fold this into the dialect design or supersede §2.1/§3.6 with
  a short design addendum. Per the design-requirement standard, the PR 2
  implicit-dereference behavior should be captured with **REQ IDs** in a
  `specs/design/` addendum before its implementation code lands.

## File map

### PR 1 — Front end & binding

| File | Change |
|------|--------|
| `compiler/parser/src/options.rs` | New `allow_reference_to` field via `define_compiler_options!`; add to `Codesys` dialect preset; **not** `Rusty` (Rusty already carries `REF_TO`). Add mutual-exclusivity note. |
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
| `specs/design/…` | Design addendum with REQ IDs for implicit-dereference behavior (see design-doc note above). |
| `compiler/analyzer/src/xform_insert_implicit_deref.rs` | **New** — post-type-resolution transform that wraps reads of `REFERENCE`-typed variables in `ExprKind::Deref` and sets `deref: true` on assignment targets, skipping `REF=` targets and `__ISVALIDREF` arguments. |
| `compiler/analyzer/src/stages.rs` | Wire the transform into the pipeline after type resolution, before codegen. |
| `compiler/analyzer/src/rule_ref_to.rs` | Review which rules apply under implicit-deref semantics. Notably, arithmetic/ordering that `REF_TO` rejects (P2033/P2035) is *legal* for `REFERENCE` because it operates on the dereferenced value — ensure the transform runs first so these rules see `ref^`, not `ref`. Gate any `REF_TO`-only checks on `RefSyntax`. |
| `compiler/analyzer/src/…` (builtins) | `__ISVALIDREF(ref)` → lower to `ref <> NULL` (reuses existing null comparison). |
| `compiler/codegen/tests/it/end_to_end_reference_to.rs` | Extend: bare implicit read/write (`ref := 5;`, `y := ref;`), aliasing, `__ISVALIDREF`, uninitialized-reference trap. |
| `compiler/parser` / analyzer tests | Cover the non-deref contexts (REF= target, `__ISVALIDREF` arg are not auto-dereffed). |
| `docs/reference/language/data-types/derived/reference-types.rst` | Document the implicit-dereference access model. |

## Tasks

### Phase 1 (PR 1): Front end & binding

- [ ] Add `allow_reference_to` to `define_compiler_options!` (`options.rs`); enable it in the `Codesys` dialect preset; update the `from_dialect` / `FEATURE_DESCRIPTORS` count tests.
- [ ] Add `--allow-reference-to` clap arg and `|=` overlay in `ironplc-cli/bin/main.rs`.
- [ ] Add the mutual-exclusivity validation error (both `--allow-reference-to` and `--allow-ref-to`/Edition 3 set → reject with a clear message). Test both the rejected and each-alone cases.
- [ ] Add `allowReferenceTo` to LSP `extract_compiler_options()` + test.
- [ ] Expose the option key in `mcp/src/tools/common.rs`.
- [ ] Add the `REFERENCE` token to `token.rs` (+ `describe()` arm + lexer test); confirm `TO` tokenizes separately.
- [ ] Create `xform_demote_reference_keyword.rs` (demote when `!allow_reference_to`) with tests; register it in `lib.rs` `tokenize_program()`.
- [ ] Add `RefSyntax { RefTo, ReferenceTo }` and the `syntax` field to `ReferenceDeclaration` / `ReferenceInitializer` (`dsl/src/common.rs`); update all constructors/pattern matches (parser, renderer, analyzer, codegen) to set/handle the tag; existing `REF_TO` paths set `RefSyntax::RefTo`.
- [ ] Add `REFERENCE TO` parser productions (type decl + var init) tagging `RefSyntax::ReferenceTo`.
- [ ] Add the `REF=` binding operator in assignment context, lowering to the existing `ExprKind::Ref` reference-assignment.
- [ ] Update the renderer to emit `REFERENCE TO` / `REF=` based on `RefSyntax`.
- [ ] Add keyword-safety regression: `REFERENCE` usable as an identifier in standard mode.
- [ ] Add parser tests for the new productions and `REF=`.
- [ ] Add plc2plc round-trip test (`reference_to.st` → `reference_to_rendered.st`).
- [ ] Add end-to-end execution test (bind via `REF=`, access via explicit `^`, verify values).
- [ ] Update docs (`enabling-dialects-and-features.rst`, `ironplcc.rst`, `reference-types.rst`) and the flag table in `syntax-support-guide.md`.
- [ ] `cd compiler && just` — all checks (compile, coverage ≥85%, clippy, fmt) pass.

### Phase 2 (PR 2): Implicit dereference & semantics

- [ ] Write a `specs/design/` addendum (REQ IDs) specifying implicit-dereference behavior and the non-deref contexts; reconcile with `beckhoff-twincat-dialect.md` §2.1/§3.6.
- [ ] Implement `xform_insert_implicit_deref.rs`: wrap reads of `REFERENCE`-typed variables in `Deref`; set `deref: true` on `REFERENCE`-typed assignment targets; skip `REF=` targets and `__ISVALIDREF` arguments.
- [ ] Wire the transform into `stages.rs` after type resolution and before codegen.
- [ ] Reconcile `rule_ref_to.rs`: ensure the transform runs before the rules so arithmetic/ordering on `REFERENCE` values (legal, because auto-dereferenced) is not wrongly rejected; gate any `REF_TO`-only checks on `RefSyntax`.
- [ ] Add `__ISVALIDREF(ref)` lowering to `ref <> NULL`.
- [ ] Extend end-to-end tests: bare implicit read/write, aliasing, `__ISVALIDREF`, uninitialized-reference trap.
- [ ] Add tests proving the non-deref contexts are not auto-dereferenced.
- [ ] Update `reference-types.rst` with the implicit-dereference access model.
- [ ] `cd compiler && just` — all checks pass.

## Out of scope

- `POINTER TO` and the `ADR()`/`^` pointer model (a separate, explicitly-dereferenced type; its own future flag).
- `S=` / `R=` extended assignment operators.
- `ARRAY [..] OF REFERENCE TO T` (can be added later alongside the existing `ARRAY OF REF_TO`).
- Any TwinCAT OOP features (methods, properties, interfaces) — tracked in `beckhoff-twincat-dialect.md`.
