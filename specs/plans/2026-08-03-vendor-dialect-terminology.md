# Split "vendor" vs. "dialect" terminology (issue #1295)

## Motivation

Issue [#1295](https://github.com/ironplc/ironplc/issues/1295) proposes removing
the word "vendor" globally in favor of "dialect"/"extension". `vendor` appears
~427 times across ~100 files.

The issue is **half right**. Most occurrences use "vendor" as a loose synonym
for *dialect/syntax* ("vendor extension", "vendor dialect", "vendor-specific"),
and those genuinely should go. But its acceptance criterion — `git grep -i
vendor` returns essentially nothing — would also erase a concept the project
actually needs and already uses correctly: **vendor** as the *product / runtime
/ library* (CODESYS, Beckhoff, Siemens), including the existing "Vendor
Compatibility Shims" documentation. A dialect is syntax you parse against; a
vendor is a runtime you emulate. We will have **vendor compatibility
libraries**, never "dialect compatibility libraries".

The deeper reason "vendor" drifted into meaning "dialect" is that **the project
never defined either word**. There is no glossary; definitions live implicitly
and inconsistently across ADR-0012, ADR-0036, `options.rs` doc-comments, and the
dialect docs. A rename alone will not stay fixed, because nothing stops the next
PR from re-coining the term (as PR #1287 did with `VendorExtension`).

## Decision

Reframe #1295 from **"remove vendor"** to **"disambiguate vendor vs. dialect"**:

1. **Glossary first.** `specs/steering/glossary.md` is the authoritative
   definition of dialect, vendor, extension, edition, flag, and vendor
   compatibility library. This lands before the rename so the rename has a
   standard to conform to, and wires into the steering pointer pattern
   (CLAUDE.md, CURSOR.md, `.kiro/steering/`) so it is load-bearing.
2. **Rename the *syntax-sense* uses** of "vendor" to dialect/extension.
3. **Keep the *product/runtime-sense* uses** of "vendor".
4. **Preserve** external schema names and annotate (do not rewrite) ADR history.

Canonical replacements (from the glossary):

- "vendor extension" → **"dialect extension"** / **"extension"**
- "vendor-extension flag" / "vendor flag" → **"dialect flag"** / **"`--allow-*`
  flag"**
- "vendor dialect" → **"dialect"**
- "vendor-specific (syntax)" → **"non-standard (syntax)"** / **"dialect-specific"**
- `VendorExtension` (trait) → **`DialectExtension`**; `extension_origins` /
  `ExtensionOrigin` keep their names (origins legitimately name vendors).

## Inventory (buckets)

Counts are approximate (`git grep -in vendor`, 427 lines total). The bucket, not
the exact count, drives the treatment.

### A. EXCLUDE — external, preserve verbatim (~6 lines)

Defined by the PLCopen / IEC TC6 XML standard, not IronPLC vocabulary.

- `compiler/resources/schemas/tc6_xml_v201.xsd` — `vendor`, `vendorElement`,
  "Vendor specific".
- `integrations/vscode/syntaxes/plcopen-xml.tmLanguage.json` — grammar mirroring
  the XSD element name.

### B. KEEP — "vendor" = product / runtime / library (~30 lines)

Correct usage under the glossary; leave as-is (light copyedits only).

- `docs/explanation/system-clock-and-uptime.rst` — **"Vendor Compatibility
  Shims"** section (the canonical vendor-library concept).
- ADR-0012 / ADR-0036 body prose about "vendor toolchain", "vendor files",
  "vendor platforms", "vendor ships…", "the vendor's own toolchain".
- Scattered "vendor files", "vendor-compatible", "vendor tool", "vendor
  ecosystem/environments/project" in docs and design specs.

*Judgement call:* "vendor-compatible dialect" (rusty/codesys) is **keep** — it
correctly names a dialect *compatible with a vendor*.

### C. RENAME — "vendor" = syntax (the bulk, ~350+ lines)

Rename to dialect/extension per the canonical table above.

- **Rust code — symbols (do first; they ripple into docs):**
  - `compiler/dsl/src/extension.rs` — trait `VendorExtension` → `DialectExtension`;
    module/trait doc-comments ("vendor-specific language extensions",
    "vendor dialects").
  - `compiler/dsl/src/common.rs` — `impl VendorExtension for
    FunctionBlockOop` / `InterfaceDeclaration` and doc-comments.
  - `compiler/analyzer/src/rule_unsupported_extension.rs` — `use` + `dyn
    VendorExtension` + doc-comments.
  - `compiler/parser/src/options.rs` — macro metavariable `$vendor_field`,
    "vendor-extension" doc-comments, test helpers `enabled_vendor_flags` /
    `assert_enabled_vendor_flags` / `*_vendor_flags` test names.
  - Test names carrying "vendor": `main.rs` `…each_vendor_flag_cli_form…`,
    `lsp.rs` `…enables_ref_to_and_vendor_flags`, `mcp/tools/common.rs`
    `…rusty_dialect_then_vendor_flags_enabled`.
  - Comments in `compiler/parser/src/{parser.rs,token.rs}`,
    `compiler/analyzer/**` (`// vendor extension`), `compiler/plc2plc/**`,
    `compiler/dsl/src/{fold,visitor}.rs`.
- **Docs:**
  - `docs/includes/requires-vendor-extension.rst` → **file rename** to
    `requires-dialect-extension.rst` (or `requires-extension.rst`) + update all
    `.. include::` references; reword its body ("This is a vendor extension…").
  - `docs/explanation/enabling-dialects-and-features.rst` — "vendor extensions"
    throughout.
  - `docs/reference/extension-library/index.rst` — "vendor extension functions"
    → "extension functions"; fix the mislabeling where `__SYSTEM_UP_TIME` is
    implied to be a vendor feature (it is an IronPLC convention).
  - `docs/reference/compiler/ironplcc.rst`, problem-code pages
    (P4028/P4036/P4037/P4041–P4045), `docs/reference/editor/settings.rst`,
    extension-library sub-pages.
- **Specs:** `specs/design/**` (`beckhoff-twincat-dialect.md`,
  `siemens-scl-dialect.md`, `dialect-token-transforms.md`, …),
  `specs/steering/{syntax-support-guide.md,iec-61131-3-compliance.md}`.
- **VS Code:** `integrations/vscode/package.json` setting descriptions.

### D. ADRs — annotate, do not rewrite (history)

`specs/adrs/0012-accept-vendor-dialect-files-as-is.md` (title + 44 refs),
`0036`, `0038`, `0040`, and dated `specs/plans/*.md` are decision history.
Leave their text; the title `0012-accept-vendor-dialect-files-as-is` stays.
Optionally add a one-line note at the top of ADR-0012 pointing to the glossary
for current terminology. Much of ADR-0012's "vendor" usage is already the
*product* sense (bucket B) and is correct regardless.

## Sequencing

1. Land the glossary + steering pointers (this branch).
2. Rename Rust symbols (bucket C code), starting with `VendorExtension` →
   `DialectExtension`. `cd compiler && just` must stay green.
3. Sweep docs/specs (bucket C docs), including the `requires-vendor-extension`
   file rename; rebuild docs.
4. Copyedit bucket B for consistency; annotate ADR-0012.
5. Verify against the revised acceptance criteria.

## Acceptance criteria (revised from #1295)

- `specs/steering/glossary.md` exists and is referenced from CLAUDE.md,
  CURSOR.md, and `.kiro/steering/`.
- No `vendor` remains where it means *syntax/dialect*: no "vendor extension",
  "vendor dialect", "vendor-extension flag", or `VendorExtension` symbol in
  code, docs, diagnostics, or CLI help.
- Every surviving `vendor` refers to a **product / runtime / library** (bucket
  B) or an **external schema name** (bucket A), and is intentional.
- No user-facing docs, diagnostics, CLI help, or public Rust API describe
  *syntax* as "vendor".
- CI passes (`cd compiler && just`) and docs rebuild.

## Notes / risks

- **"Extension" is overloaded** (syntax feature vs. Extension Library vs. VS
  Code extension). Prefer "dialect extension" for the syntax sense to keep them
  separable; the glossary flags this.
- **Overlap with #1298** (`specs/plans/2026-08-03-rename-allow-fb-inheritance-flag.md`):
  that plan deliberately deferred the `VendorExtension` trait rename ("a
  separate, still-accurate concept"). This plan owns that rename; coordinate so
  the two don't collide on `options.rs` / `rule_unsupported_extension.rs`.
- The `ExtensionOrigin` enum and `extension_origins()` legitimately enumerate
  **vendors** (`BeckhoffCodesys`) — that is the product sense and stays.
