# Design: TwinCAT `REFERENCE TO` Reference Types

## Overview

This document describes support for the Beckhoff TwinCAT / CODESYS
`REFERENCE TO` reference type as a separately-flagged alternative to the
IEC 61131-3 `REF_TO` syntax already implemented in IronPLC (see
[ref-to.md](ref-to.md)). `REFERENCE TO` and `REF_TO` are surface variants of
the *same* underlying concept — a strongly-typed reference implemented as a
variable-table index — but with different usage models:

| Concern | IEC `REF_TO` (`--allow-ref-to`) | TwinCAT `REFERENCE TO` (`--allow-reference-to`) |
|---------|--------------------------------|------------------------------------------------|
| Declare | `r : REF_TO INT;` | `r : REFERENCE TO INT;` |
| Bind    | `r := REF(x);` | `r REF= x;` |
| Read    | `y := r^;` (explicit `^`) | `y := r;` (implicit dereference) |
| Write   | `r^ := 5;` (explicit `^`) | `r := 5;` (implicit dereference) |
| Validity| `r = NULL` | `__ISVALIDREF(r)` / `r = 0` |

The reference **backend is reused wholesale**: references are type-erased to
`u64` variable-table indices, codegen emits `LOAD_INDIRECT`/`STORE_INDIRECT`,
and the VM traps on null (V4004). None of that depends on the surface keyword,
so `REFERENCE TO` maps onto the existing AST
(`ReferenceDeclaration`, `ReferenceInitializer`, `IntermediateType::Reference`,
`ExprKind::{Ref, Deref, Null}`) and needs **no new backend**.

This document supersedes
[beckhoff-twincat-dialect.md](beckhoff-twincat-dialect.md) §2.1 and §3.6, which
treated `REFERENCE TO` as parse-only (a distinct `TypeSpec::ReferenceTo`
reported as unsupported). Here `REFERENCE TO` reuses the `REF_TO` backend to
produce executable code.

## Delivery

The feature is delivered in two phases, one PR each:

- **PR 1 — Front end & binding (this phase).** Flag, lexer keyword, parser
  productions for the `REFERENCE TO` type constructor and the `REF=` binding
  operator, AST tagging so the two syntaxes round-trip distinctly, and reuse of
  the whole existing `REF_TO` analyzer/codegen/VM backend. Access in this phase
  is via the existing explicit `^` operator (enough to prove end-to-end
  execution). Requirements `0xx`–`4xx` and `6xx`.
- **PR 2 — Implicit dereference & TwinCAT-faithful semantics.** An analyzer
  transform that makes bare uses of a `REFERENCE`-typed variable behave as an
  automatic dereference, plus `__ISVALIDREF`. Requirements `5xx`.

## Gating & coexistence

`REFERENCE TO` is gated behind a new `--allow-reference-to` vendor flag,
following the established token-demotion pattern. A new `REFERENCE` keyword
token is demoted to `Identifier` unless `--allow-reference-to` is set — exactly
how `REF_TO`/`REF`/`NULL` are demoted today. The always-present grammar
productions never fire when the keyword is demoted.

`REF_TO` and `REFERENCE TO` are **not** made mutually exclusive. Per
[ADR-0038](../adrs/0038-no-restrictions-on-flag-combinations.md), the compiler
does not restrict `--allow-*` flag combinations; preference is expressed through
dialect presets. Only the CODESYS dialect bundles `REFERENCE TO`, and no dialect
bundles both. Coexistence stays well-defined because each declaration carries a
`RefSyntax` tag (`RefTo` vs `ReferenceTo`); the PR-2 implicit-dereference
transform keys on `RefSyntax::ReferenceTo`, so `REF_TO` variables are never
implicitly dereferenced even when both flags are set.

## AST tagging

`ReferenceDeclaration` and `ReferenceInitializer` are shared by both syntaxes.
A `syntax: RefSyntax` discriminant records which keyword produced each node so
the renderer can reproduce the original keyword and binding operator:

```rust
pub enum RefSyntax {
    RefTo,        // IEC 61131-3 REF_TO
    ReferenceTo,  // TwinCAT / CODESYS REFERENCE TO
}
```

For `ARRAY [..] OF REFERENCE TO T`, the DSL `ArraySubranges.ref_to` field
changes from `bool` to `Option<RefSyntax>` (`None` = non-reference element,
`Some(_)` = reference element tagged with its surface syntax) so the two array
element syntaxes round-trip distinctly. The tag is only needed up to the
renderer; the analyzer and codegen collapse it to a bool
(`ref_to.is_some()`) at their boundaries, leaving the type system and backend
unchanged.

## `REF=` binding

The TwinCAT binding operator `r REF= x;` is recognized in assignment context
and lowered to the existing reference-assignment form `r := REF(x)` — a normal
assignment whose value expression is `ExprKind::Ref`. No new AST node or opcode
is required; the reference is bound exactly as the IEC form binds it.

---

## Requirements

Each requirement below carries a bold `REQ-RTO-<slug>-NNN` marker, where the
slug names the crate that owns (hosts) the conformance test. `RTO` is an unused
area prefix. Every marker has a spec-linked test named
`{area}_spec_req_rto_{nnn}_{description}` (the slug lives in the
`#[spec_test(REQ_RTO_<slug>_NNN)]` attribute, not the function name), enforced
per-crate via the crate-slug mechanism from
[ADR-0037](../adrs/0037-mandatory-crate-slug-in-requirement-ids.md).

### Options & dialects (parser)

**REQ-RTO-parser-001** The `codesys` dialect preset enables
`allow_reference_to`.

**REQ-RTO-parser-002** The `rusty` dialect preset does *not* enable
`allow_reference_to` (Rusty already carries `REF_TO`).

**REQ-RTO-parser-003** Setting both `allow_reference_to` and `allow_ref_to`
together is accepted — the compiler rejects no `--allow-*` combination
(ADR-0038).

### Lexer & keyword demotion (parser)

**REQ-RTO-parser-100** The text `REFERENCE` lexes as a single `Reference`
keyword token (distinct from `REF`, which `REFERENCE` shares a prefix with;
longest-match wins).

**REQ-RTO-parser-101** With `allow_reference_to` off, the `Reference` token is
demoted to `Identifier` so programs may use `REFERENCE` as an identifier.

**REQ-RTO-parser-102** With `allow_reference_to` on, the `Reference` token is
kept as the keyword.

**REQ-RTO-parser-103** `REFERENCE` is a valid identifier in standard mode (flag
off): a program declaring a variable named `REFERENCE` parses.

### Parser productions (parser)

**REQ-RTO-parser-200** `r : REFERENCE TO INT;` yields a
`ReferenceInitializer` tagged `RefSyntax::ReferenceTo`.

**REQ-RTO-parser-201** `TYPE T : REFERENCE TO INT; END_TYPE` yields a
`ReferenceDeclaration` tagged `RefSyntax::ReferenceTo`.

**REQ-RTO-parser-202** A `REF_TO` declaration is tagged `RefSyntax::RefTo`
(regression: existing syntax keeps its tag).

**REQ-RTO-parser-210** `r REF= x;` parses as a reference binding equivalent to
`r := REF(x)` — an assignment whose value is `ExprKind::Ref`.

**REQ-RTO-parser-220** `ARRAY [..] OF REFERENCE TO T` parses and tags the
element `Some(RefSyntax::ReferenceTo)`.

### Type resolution & checking (analyzer)

**REQ-RTO-analyzer-300** `REFERENCE TO T` resolves to
`IntermediateType::Reference`, reusing the `REF_TO` resolution path.

**REQ-RTO-analyzer-301** Binding a `REFERENCE TO` variable to a mismatched
target type is rejected (P2032), reusing the `REF_TO` type-compatibility rule.

### Execution (codegen)

**REQ-RTO-codegen-400** Reading a `REF=`-bound `REFERENCE TO` variable via `^`
yields the referenced value.

**REQ-RTO-codegen-401** Writing through `^` to a `REF=`-bound `REFERENCE TO`
variable stores to the referenced variable.

**REQ-RTO-codegen-402** Dereferencing an unbound `REFERENCE TO` variable traps
`NullDereference` (V4004).

**REQ-RTO-codegen-420** An `ARRAY [..] OF REFERENCE TO T` element can be bound
(`REF=`) and accessed (`^`).

### Round-trip rendering (plc2plc)

**REQ-RTO-plc2plc-600** A `ReferenceTo`-tagged declaration renders as
`REFERENCE TO <target>`.

**REQ-RTO-plc2plc-601** A `REF=` binding renders back as `REF=`.

**REQ-RTO-plc2plc-602** A `RefTo`-tagged declaration still renders as `REF_TO`
(regression).

---

## Requirements traceability

| Req | Claim | Test fn | Crate |
|-----|-------|---------|-------|
| **REQ-RTO-parser-001** | `codesys` enables `allow_reference_to` | `options_spec_req_rto_001_*` | parser |
| **REQ-RTO-parser-002** | `rusty` does not enable `allow_reference_to` | `options_spec_req_rto_002_*` | parser |
| **REQ-RTO-parser-003** | Both flags coexist (ADR-0038) | `options_spec_req_rto_003_*` | parser |
| **REQ-RTO-parser-100** | `REFERENCE` lexes as `Reference` | `lexer_spec_req_rto_100_*` | parser |
| **REQ-RTO-parser-101** | Flag off → `REFERENCE` demoted | `xform_spec_req_rto_101_*` | parser |
| **REQ-RTO-parser-102** | Flag on → `REFERENCE` kept | `xform_spec_req_rto_102_*` | parser |
| **REQ-RTO-parser-103** | `REFERENCE` identifier in standard mode | `parser_spec_req_rto_103_*` | parser |
| **REQ-RTO-parser-200** | `REFERENCE TO` var decl tagged | `parser_spec_req_rto_200_*` | parser |
| **REQ-RTO-parser-201** | `REFERENCE TO` type decl tagged | `parser_spec_req_rto_201_*` | parser |
| **REQ-RTO-parser-202** | `REF_TO` tagged `RefTo` | `parser_spec_req_rto_202_*` | parser |
| **REQ-RTO-parser-210** | `REF=` parses as reference binding | `parser_spec_req_rto_210_*` | parser |
| **REQ-RTO-parser-220** | `ARRAY OF REFERENCE TO` tagged | `parser_spec_req_rto_220_*` | parser |
| **REQ-RTO-analyzer-300** | `REFERENCE TO T` resolves to Reference | `analyzer_spec_req_rto_300_*` | analyzer |
| **REQ-RTO-analyzer-301** | Reference bind type mismatch rejected | `analyzer_spec_req_rto_301_*` | analyzer |
| **REQ-RTO-codegen-400** | Read through `^` yields value | `codegen_spec_req_rto_400_*` | codegen |
| **REQ-RTO-codegen-401** | Write through `^` stores value | `codegen_spec_req_rto_401_*` | codegen |
| **REQ-RTO-codegen-402** | Unbound deref traps NullDereference | `codegen_spec_req_rto_402_*` | codegen |
| **REQ-RTO-codegen-420** | `ARRAY OF REFERENCE TO` element access | `codegen_spec_req_rto_420_*` | codegen |
| **REQ-RTO-plc2plc-600** | `REFERENCE TO` declaration renders | `plc2plc_spec_req_rto_600_*` | plc2plc |
| **REQ-RTO-plc2plc-601** | `REF=` binding renders | `plc2plc_spec_req_rto_601_*` | plc2plc |
| **REQ-RTO-plc2plc-602** | `REF_TO` still renders (regression) | `plc2plc_spec_req_rto_602_*` | plc2plc |

## Out of scope (this document)

- Implicit dereference and `__ISVALIDREF` — PR 2, requirements `5xx` (added to
  this document in that phase).
- `POINTER TO` and the `ADR()`/`^` pointer model.
- `S=` / `R=` extended assignment operators.
- TwinCAT OOP features (methods, properties, interfaces).
