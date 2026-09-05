# Remove the `ExtensionOrigin` concept

Tracking issue: [#1297](https://github.com/ironplc/ironplc/issues/1297)

## Motivation

`ExtensionOrigin` tags each non-standard construct with *which vendor* introduced
it. We don't care about the origin of an extension — only about **flags** and
**dialects**. Whether a construct is recognized, promoted to a keyword, or
flagged as unsupported already depends on the active dialect and the `--allow-*`
flags; origin drives nothing behavioral.

The glossary (`specs/steering/glossary.md`, *Extension*) already says the
"which vendors accept it" mapping belongs in the dialect table — i.e. the
`define_compiler_options!` dialect list in `compiler/parser/src/options.rs` — not
in a per-extension `extension_origins()`. `ExtensionOrigin` duplicates that
mapping at the wrong altitude.

## What `ExtensionOrigin` does today

- A single-variant enum (`BeckhoffCodesys`) with an `as_str()` label, in
  `compiler/dsl/src/extension.rs`.
- `LanguageExtension::extension_origins()` returns `&'static [ExtensionOrigin]`,
  implemented on `FunctionBlockOop` and `InterfaceDeclaration`
  (`compiler/dsl/src/common.rs`).
- Its only consumer, `RuleUnsupportedExtension::flag`
  (`compiler/analyzer/src/rule_unsupported_extension.rs`), interpolates the
  origin into the message of a `P9999 NotImplemented` diagnostic:
  `"{name} ({origins} extension) is recognized but not yet supported by IronPLC"`.

It is **not** used for keyword promotion: OOP keyword demotion is driven purely
by `options.allow_fb_inheritance` in
`compiler/parser/src/xform_demote_oop_keywords.rs`.

## Changes

### Code

1. `compiler/dsl/src/extension.rs`
   - Delete the `ExtensionOrigin` enum and its `as_str()` impl.
   - Delete the `extension_origin_as_str_*` unit test (the whole `tests` module
     becomes empty and is removed).
   - Remove `extension_origins()` from the `LanguageExtension` trait.
2. `compiler/dsl/src/common.rs`
   - Drop `ExtensionOrigin` from the `use crate::extension::{…}` import.
   - Delete the two `extension_origins()` impls (on `FunctionBlockOop` and
     `InterfaceDeclaration`).
3. `compiler/analyzer/src/rule_unsupported_extension.rs`
   - In `flag()`, drop the `origins` collection and the `({…} extension)`
     clause. Message becomes
     `"{extension_name} is recognized but not yet supported by IronPLC"`.

### Docs / design specs

4. `specs/design/beckhoff-twincat-dialect.md` — remove the "Extension Origin
   Model" section and the origin plumbing in the `LanguageExtension` section;
   describe keyword promotion as dialect/flag-gated; fix stale `P9004` → the
   real `P9999 NotImplemented`.
5. `specs/design/siemens-scl-dialect.md` — remove the "Extension Origin Model"
   section and origin references; relabel keyword tables to dialect framing.

### Kept

- The `LanguageExtension` trait itself (`extension_name` + `extension_span` feed
  the P9999 diagnostic).
- The P9999 diagnostic — only its wording changes.

## Verification

- The rule's tests assert only on the `"P9999"` code, not the message text, so
  they need no change.
- `cd compiler && just` (build, coverage ≥85%, clippy, fmt).
