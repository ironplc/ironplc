# Plan: `EXTENDS`/`IMPLEMENTS` and Minimal `INTERFACE` Support

## Goal

Parse the `EXTENDS`/`IMPLEMENTS` clause on `FUNCTION_BLOCK` headers and a
minimal `INTERFACE ... END_INTERFACE` declaration, so that:

1. `FUNCTION_BLOCK FB_AdvancedMotor EXTENDS FB_Motor IMPLEMENTS I_Drivable`
   parses instead of erroring on `EXTENDS`.
2. A variable declared with an interface type (`pDrv : I_Drivable;`) resolves
   to a known type instead of failing semantic analysis with "type not
   declared," provided the interface itself is declared somewhere in the
   project.

This is issue #1199's item 2 ("EXTENDS/IMPLEMENTS and interface-typed
variables"), the second-highest blocker in the 158-file survey after pragma
headers (see [2026-07-18-twincat-pragma-skipping.md](2026-07-18-twincat-pragma-skipping.md),
already landed). It implements a scoped slice of
[specs/design/beckhoff-twincat-dialect.md](../design/beckhoff-twincat-dialect.md)
sections 1.3–1.4, under [ADR-0012](../adrs/0012-accept-vendor-dialect-files-as-is.md).

## Non-goals (deferred to later PRs)

- **`METHOD`/`END_METHOD`, `PROPERTY`/`END_PROPERTY` bodies.** Verified against
  the current `twincat_parser.rs`: it only extracts a POU's own top-level
  `Declaration`/`Implementation` CDATA and silently ignores any `<Method>` or
  `<Property>` child elements. That means method *bodies* are not actually
  blocking real files today — only the `EXTENDS`/`IMPLEMENTS` header clause and
  the interface type name itself are. Method/property support is real,
  separate work (calling a method, checking an override signature, `GET`/`SET`
  bodies) and is out of scope here.
- **Inheritance/override semantics.** `extends`/`implements` are parsed and
  stored as metadata only. No checking that the base FB exists, that method
  signatures are compatible, or that `IMPLEMENTS` is actually satisfied.
- **Method/Property signatures inside `INTERFACE`.** Since method bodies are
  out of scope (see above), and TwinCAT stores each interface member as a
  separate `<Method>` or `<Property>` child element (not inline text) exactly
  like function blocks do, the interface's own `Declaration` CDATA is just the
  header line (e.g. `INTERFACE I_Drivable` or
  `INTERFACE I_Drivable EXTENDS I_Base`). Nothing else needs to be parsed
  inside `INTERFACE ... END_INTERFACE` for this PR.

  **Verified against real project files** (14 real `.TcIO` interface files
  from a private, real-world TwinCAT codebase): confirmed. `<Itf>` elements
  overwhelmingly contain `<Property>` children (far more common than
  `<Method>` — e.g. `I_BaseAxis.TcIO` has 13 properties and 1 method) with
  their own nested `<Get>`/`<Set>`, plus occasional bare `<Method>` siblings.
  All are structurally identical to how `FUNCTION_BLOCK` already handles
  `<Method>`/`<Property>` siblings today (ignored). The `Itf`'s own
  `<Declaration>` CDATA is confirmed to be exactly the bare header line, e.g.
  `INTERFACE I_Focus EXTENDS I_BaseAxis` or just `INTERFACE I_BaseAxis` with
  no base — nothing more.
- **`THIS^`/`SUPER^`, access modifiers (`PUBLIC`/`PRIVATE`/...), `ABSTRACT`/
  `FINAL`/`OVERRIDE`.** Later phases of the design doc.

## Why this is bigger than the pragma PR

Pragmas were pure trivia — parsed and thrown away, touching only the
lexer/parser layer. `INTERFACE` is a new top-level declaration, which means a
new `LibraryElementKind::InterfaceDeclaration` variant. Grepping for existing
exhaustive matches over `LibraryElementKind` found at minimum:

- `compiler/analyzer/src/xform_toposort_declarations.rs` (exhaustive `match`)
- `compiler/analyzer/src/xform_resolve_constant_expressions.rs`
- `compiler/analyzer/src/xform_resolve_expr_types.rs`
- `compiler/codegen/src/compile.rs`
- `compiler/mcp/src/tools/compile.rs`
- `compiler/parser/src/declarations.rs`
- `compiler/parser/src/parser.rs`
- `compiler/sources/src/xml/transform.rs` (PLCopen XML — interfaces don't
  exist there, needs a deliberate "not applicable" handling decision)

`cargo build` will surface the exact list precisely (Rust's exhaustiveness
checking doesn't miss any), but this is the realistic starting scope — not a
2-file change like the pragma flag was.

To avoid a half-supported feature silently producing wrong results
(inheritance that looks like it works but isn't checked; an interface that
resolves as a type but can't actually be called through), this PR adopts the
`VendorExtension`/`P9004` pattern from the design doc's Phase 0, scoped down
to just these two constructs:

- `ExtensionOrigin` enum (`BeckhoffCodesys` variant only needed for now)
- `VendorExtension` trait
- `P9004 UnsupportedExtension` problem code
- `rule_unsupported_extension.rs` semantic rule, with `visit_*` overrides for
  `FunctionBlockDeclaration` (when `extends`/`implements` is `Some`/non-empty)
  and `InterfaceDeclaration`

This gives users a clear "recognized but not yet supported" diagnostic
instead of either a hard parse error (today) or silent, wrong behavior (if we
skipped this and just let inheritance/interfaces resolve without any
signal). It's the same infrastructure the design doc already specifies for
this exact purpose, just scoped to two constructs instead of the full OOP set.

## Design

### Dialect flag

One new flag, `allow_oop_extensions`, enabled for `[Rusty, Codesys]` — same
placement as `allow_pragmas` and for the same reason: `EXTENDS`/`IMPLEMENTS`/
`INTERFACE` are CODESYS-core OOP extensions (ADR-0012 already classifies this
as `BeckhoffCodesys` origin), not TwinCAT-only, so they belong on the existing
CODESYS-compatible dialect rather than a new one. Per
`syntax-support-guide.md`'s "group related extensions under one flag"
guidance, `EXTENDS`, `IMPLEMENTS`, and `INTERFACE`/`END_INTERFACE` all live
behind this single flag rather than three.

### Tokens: demotion pattern (not promotion)

The actual codebase convention (`xform_demote_edition3_keywords.rs`,
documented in `syntax-support-guide.md`) is **demotion**: a keyword is always
lexed as its specific token type via `#[token(...)]`, then demoted back to
`Identifier` when the relevant flag is off. This is the opposite of what
`beckhoff-twincat-dialect.md` originally proposed (promotion: identifier by
default, promoted to keyword only when enabled) — that design doc predates
the demotion convention established for `LTIME`/`REF_TO`/etc. This plan
follows the codebase's actual established pattern, not the older doc.

New tokens: `Extends`, `Implements`, `Interface`, `EndInterface`. All four
demoted to `Identifier` when `!options.allow_oop_extensions`, in a new
`xform_demote_oop_keywords.rs` (or added to the existing
`xform_demote_edition3_keywords.rs` — decide based on how it reads once
written; they're logically the same kind of transform).

**Prerequisite regression test** (per the design doc's Phase 0 and general
good practice before adding keywords that shadow identifiers): a function
block using `EXTENDS`, `IMPLEMENTS`, `INTERFACE`, `END_INTERFACE` as variable
names, parsed in standard/default dialect, must still succeed. Add this
*before* the token/demotion changes so it's meaningful.

### Grammar

`compiler/parser/src/parser.rs`:

```
function_block_declaration =
  FUNCTION_BLOCK name
  (EXTENDS type_name)?
  (IMPLEMENTS type_name ++ ',')?
  <existing var-decls / body / END_FUNCTION_BLOCK>
```

```
interface_declaration =
  INTERFACE name
  (EXTENDS type_name ++ ',')?
  END_INTERFACE
```

(Interfaces can extend *multiple* interfaces in TwinCAT, unlike function
blocks which extend at most one.)

**Verified against real project files**: `IMPLEMENTS I_Hydraulics, I_Brake`
and `IMPLEMENTS I_RaDecTelescope, I_Telescope` both appear in real FB headers,
confirming the comma-separated `IMPLEMENTS` list is real and necessary. No
real example of multi-interface `EXTENDS` on an `INTERFACE` was found in the
sample set (single-base or none only — e.g. `INTERFACE I_Focus EXTENDS
I_BaseAxis`), but Beckhoff's own docs describe interface-to-interface
multi-extension as valid, so the grammar still allows a comma-separated list
there for correctness even though it's unexercised by this sample.

New top-level alternative in `library_element_declaration()`.

### AST (`compiler/dsl/src/common.rs`)

```rust
pub struct FunctionBlockDeclaration {
    // ...existing fields...
    pub extends: Option<TypeName>,
    pub implements: Vec<TypeName>,
}

pub struct InterfaceDeclaration {
    pub name: Id,
    pub extends: Vec<TypeName>,
    pub span: SourceSpan,
}

pub enum LibraryElementKind {
    // ...existing variants...
    InterfaceDeclaration(InterfaceDeclaration),
}
```

### New file extension: `.TcIO`

**Verified against real project files — this was missed in the original plan
and is a hard requirement, not a nice-to-have.** TwinCAT interfaces are not
stored in `.TcPOU` files at all; they use a dedicated `.TcIO` extension (e.g.
`I_Focus.TcIO`, `I_BaseAxis.TcIO`). `compiler/sources/src/file_type.rs`
today maps only `tcpou`/`tcgvl`/`tcdut` to `FileType::TwinCat` — a `.TcIO`
file currently falls through to `FileType::Unknown` and is never even handed
to the parser. Two one-line fixes needed there:

- `FileType::from_path`: add `Some(ext) if ext.eq_ignore_ascii_case("tcio") => FileType::TwinCat,`
- `FileType::extensions()`: add `"TcIO"` to the `TwinCat` arm's slice

`FileType::from_content` (the extension-less heuristic used by the
playground) needs no change — it already matches on the `TcPlcObject`
substring generically, which `.TcIO` files also have as their root element.

`docs/reference/compiler/source-formats/twincat.rst` currently lists only
`.TcPOU`/`.TcGVL`/`.TcDUT` under "File Extensions" and needs `.TcIO` added.

### `twincat_parser.rs`: recognize `<Itf>` and add `INTERFACE` closing keyword

Two concrete changes, both confirmed against real files:

1. The root-element lookup (`compiler/sources/src/parsers/twincat_parser.rs`,
   ~line 76, currently `matches!(n.tag_name().name(), "POU" | "GVL" | "DUT")`)
   needs `"Itf"` added. Interfaces extract the same way as POUs (`Declaration`
   CDATA + optional `Implementation`), reusing the existing
   CDATA/position-adjustment code path unchanged.
2. `closing_keyword()` (~line 249) infers the synthetic closing keyword from
   the `Declaration` text's leading keyword (`FUNCTION_BLOCK` →
   `END_FUNCTION_BLOCK`, etc.). Needs one more branch: `INTERFACE` →
   `END_INTERFACE` (checked before `INTERFACE` could be confused with
   anything else — no prefix collision risk here, unlike
   `FUNCTION`/`FUNCTION_BLOCK`).

**Already confirmed safe, no change needed**: `extract_pou_implementation`
(~line 265) already returns `Ok((String::new(), None))` when `<Implementation>`
is absent entirely — which is the normal case for `<Itf>` elements (confirmed:
none of the 14 sample interface files with children had a top-level
`<Implementation>` element). No fix needed there.

### Type registration

Interfaces need to register as a known type name so `VAR pDrv : I_Drivable;`
doesn't trip "type not declared." The minimal-footprint approach: treat
`InterfaceDeclaration` as a nominal type in the same registration path
data-type declarations use for name lookup (likely
`symbol_environment.rs`/`type_environment.rs` — exact integration point to be
confirmed while implementing, since interfaces aren't `DataTypeDeclarationKind`
and don't have fields/size, so they may need a distinct "opaque named type"
representation rather than reusing the struct/enum machinery). Full field/
method resolution through an interface reference is explicitly out of scope
(would return P9004 via the `VendorExtension` visitor).

### Codegen / plc2plc / other exhaustive matches

Every other site touching `LibraryElementKind` needs a decision, not just a
compile fix:

- **plc2plc renderer**: render `EXTENDS`/`IMPLEMENTS` and `INTERFACE` back out
  (needed for round-trip tests).
- **codegen**: an `InterfaceDeclaration` or an `extends`-bearing FB reaching
  codegen should produce a clear, existing-style error (not a panic) — most
  likely already naturally blocked by the `P9004` diagnostic from the
  semantic rule running first and stopping the pipeline before codegen, but
  needs confirming.
- **MCP tools** (`project_manifest`, `types_all`, etc.): decide whether
  interfaces show up in listings (probably yes, as a new kind, even if
  otherwise inert) or are omitted for now.

## File Map (starting point — expect additions once `cargo build` surfaces
the full exhaustive-match list)

| File | Change |
|------|--------|
| `compiler/dsl/src/core.rs` (or new `extension.rs`) | `ExtensionOrigin` enum, `VendorExtension` trait |
| `compiler/problems/resources/problem-codes.csv` + `docs/reference/compiler/problems/P9004.rst` | New `P9004 UnsupportedExtension` |
| `compiler/analyzer/src/rule_unsupported_extension.rs` | New semantic rule |
| `compiler/parser/src/options.rs` | New `allow_oop_extensions` flag (`[Rusty, Codesys]`) |
| `compiler/parser/src/token.rs` | `Extends`, `Implements`, `Interface`, `EndInterface` tokens |
| `compiler/parser/src/xform_demote_oop_keywords.rs` (new, or folded into existing demote module) | Demotion transform + prerequisite regression test |
| `compiler/parser/src/parser.rs` | `EXTENDS`/`IMPLEMENTS` clause on FB; `interface_declaration()` rule |
| `compiler/dsl/src/common.rs` | `FunctionBlockDeclaration.extends`/`.implements`; `InterfaceDeclaration` struct; `LibraryElementKind::InterfaceDeclaration` |
| `compiler/sources/src/file_type.rs` | Recognize `.TcIO` extension (new — interfaces are a separate file type, not `.TcPOU`) |
| `compiler/sources/src/parsers/twincat_parser.rs` | Recognize `<Itf>` root element; `closing_keyword()` gets `INTERFACE` → `END_INTERFACE` branch |
| `docs/reference/compiler/source-formats/twincat.rst` | Add `.TcIO` to the file extensions list |
| `compiler/analyzer/src/xform_toposort_declarations.rs` | Handle new variant |
| `compiler/analyzer/src/symbol_environment.rs` / `type_environment.rs` | Register interface names as known types |
| `compiler/analyzer/src/xform_resolve_constant_expressions.rs`, `xform_resolve_expr_types.rs` | Handle new variant (likely no-op) |
| `compiler/codegen/src/compile.rs` | Handle new variant (reject cleanly) |
| `compiler/plc2plc/src/renderer.rs` | Render `EXTENDS`/`IMPLEMENTS`/`INTERFACE` |
| `compiler/mcp/src/tools/compile.rs` + others discovered | Handle new variant |
| `compiler/sources/src/xml/transform.rs` | Decide PLCopen XML handling (interfaces likely N/A there) |
| `docs/explanation/enabling-dialects-and-features.rst`, `docs/reference/compiler/ironplcc.rst`, `specs/steering/syntax-support-guide.md` | Document `--allow-oop-extensions` |

## Testing Strategy

- Keyword-safety regression test (see above), added first.
- Parser tests: FB with `EXTENDS` only, `IMPLEMENTS` only, both together,
  multiple interfaces in `IMPLEMENTS`; bare `INTERFACE`/`END_INTERFACE`;
  `INTERFACE ... EXTENDS ... END_INTERFACE`; all of the above rejected under
  the default dialect.
- Semantic test: a program declaring `VAR x : I_Foo; END_VAR` where `I_Foo`
  is declared via `INTERFACE I_Foo END_INTERFACE` resolves without a
  "type not declared" diagnostic, but does emit `P9004` for the `INTERFACE`
  declaration itself (and for the FB's `EXTENDS`/`IMPLEMENTS`, separately).
- `twincat_parser.rs` test: an `<Itf>`-rooted, `.TcIO`-style XML fixture
  (synthetic, modeled on the confirmed real structure — not copied from any
  proprietary project) parses cleanly, both with and without `EXTENDS`.
- `file_type.rs` test: `.TcIO` path/content maps to `FileType::TwinCat`.
- Regression: existing TwinCAT XML tests and standard ST tests unaffected.
- plc2plc round-trip test including `EXTENDS`/`IMPLEMENTS`/`INTERFACE`.

## Tasks

- [x] Write plan
- [x] Verify against real project files (14 `.TcIO` interface files + 60
      `.TcPOU` files using `EXTENDS`/`IMPLEMENTS` from a real TwinCAT
      codebase). Findings folded into this plan: confirmed `.TcIO` is a
      distinct, currently-unrecognized file extension; confirmed `<Itf>` root
      shape and bare-header `Declaration` content; confirmed multi-interface
      `IMPLEMENTS` is real (comma-separated); confirmed absent
      `<Implementation>` is already handled gracefully, no fix needed there.
- [x] Keyword-safety regression test (EXTENDS/IMPLEMENTS/INTERFACE/END_INTERFACE
      as variable names, standard dialect, must still parse)
- [x] `ExtensionOrigin` enum + `VendorExtension` trait (DSL crate)
- [x] `P9004 UnsupportedExtension` problem code (CSV + doc page)
- [x] `allow_oop_extensions` flag in `options.rs` (+ update descriptor-count
      tests, same as the pragma PR needed)
- [x] `Extends`/`Implements`/`Interface`/`EndInterface` tokens + demotion
      transform
- [x] Grammar: `EXTENDS`/`IMPLEMENTS` clause on `function_block_declaration()`
- [x] Grammar + AST: `interface_declaration()`, `InterfaceDeclaration`,
      `LibraryElementKind::InterfaceDeclaration`
- [x] `file_type.rs`: recognize `.TcIO` extension
- [x] `twincat_parser.rs`: recognize `<Itf>` root; add `INTERFACE` branch to
      `closing_keyword()`
- [x] Thread the new `LibraryElementKind` variant through every exhaustive
      match `cargo build` surfaces (toposort, symbol/type environment,
      codegen, plc2plc renderer, MCP tools, XML transform)
- [x] Register interface names as resolvable types in symbol/type environment
- [x] `rule_unsupported_extension.rs` with `visit_*` for `extends`/`implements`-
      bearing FBs and `InterfaceDeclaration`
- [x] All tests from Testing Strategy
- [x] Update docs (`enabling-dialects-and-features.rst`, `ironplcc.rst`,
      `syntax-support-guide.md`, new `P9004.rst`)
- [x] Run full CI pipeline (`cd compiler && just`)
- [x] Push branch to fork (no PR against `ironplc/ironplc` without explicit
      go-ahead, per standing instruction)

## Implementation Notes (decisions resolved during implementation)

- **Type registration** (the open question in the Design section): resolved
  by registering `InterfaceDeclaration` as `IntermediateType::Structure {
  fields: vec![] }` rather than adding a new `IntermediateType` variant.
  Adding a variant would have rippled into `is_primitive`/`size_in_bytes`/
  `alignment_bytes`/codegen field-offset matches — a much bigger blast
  radius than needed, since interfaces have no runtime representation in
  this PR. Confirmed safe: `P9004` on the `InterfaceDeclaration` itself
  always blocks codegen for the whole project (both `ironplc-cli/src/cli.rs`
  and the MCP `compile` tool check `context.has_diagnostics()`/error
  severity before invoking codegen), so the placeholder representation is
  never actually reached at runtime.
- **`VendorExtension` on `FunctionBlockDeclaration`**: implemented
  unconditionally (every `FunctionBlockDeclaration` has the impl available),
  but `rule_unsupported_extension`'s visitor only *calls* it when
  `extends.is_some() || !implements.is_empty()`. This resolves the tension
  between "the trait means this type is always an extension" (true for
  `InterfaceDeclaration`) and "only sometimes" (true for
  `FunctionBlockDeclaration`) — the trait supplies diagnostic metadata: the
  visitor decides when to use it.
- Actual codegen/plc2plc-renderer/MCP-tool ripple from adding
  `LibraryElementKind::InterfaceDeclaration` was smaller than the file map
  predicted: `codegen/src/compile.rs`, `mcp/src/tools/compile.rs`,
  `xform_resolve_constant_expressions.rs`, and `xform_resolve_expr_types.rs`
  all use `if let`/wildcard matches, not exhaustive ones, so they needed no
  changes — `cargo build` only surfaced `xform_toposort_declarations.rs` and
  the LSP semantic-token match as real compile errors. `plc2plc/src/renderer.rs`
  needed a `visit_interface_declaration` override (added) since the render
  path is Visitor-based, not an exhaustive match, so it compiled without the
  override but would have silently dropped `EXTENDS`/`IMPLEMENTS`/
  `INTERFACE` from rendered output without it — caught by writing the
  round-trip test before assuming it worked.
