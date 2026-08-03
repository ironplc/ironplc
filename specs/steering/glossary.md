# Glossary

This file is the **single authoritative definition** of IronPLC's core
vocabulary — the words the project uses to talk about *what code it accepts*
and *what platforms it is compatible with*. It exists so that these terms mean
exactly one thing across code, docs, ADRs, diagnostics, and CLI help.

Treat it as load-bearing:

- If a term below contradicts how a piece of code, doc, or PR uses the word,
  the glossary wins — fix the usage, or change the glossary deliberately (not
  casually) if the definition is wrong.
- Before introducing a new noun for an existing concept, check whether one of
  these terms already covers it. Do not coin a synonym.
- When a genuinely new concept appears, add it here **first**, then use it.

Keep this file about *meaning*, not implementation. It names concepts and their
relationships; it does not track types, line numbers, or specifics that belong
in code or design docs.

---

## The core distinction: dialect vs. vendor

These two words are the reason this file exists. They are **not synonyms**, and
history shows they drift together unless pinned down.

> A **dialect** is something you *parse against*.
> A **vendor** is something you *emulate the runtime of*.
>
> A vendor has both a dialect **and** a library.
> A dialect has neither source files nor a runtime of its own.

Everything else follows from that split.

---

## Terms

### Dialect

A named set of **syntax** that IronPLC will parse without error — an
[edition](#edition) baseline plus a bundle of [extensions](#extension). A
dialect describes *the shape of the text the parser accepts*; it says nothing
about runtime behavior or libraries.

Each dialect corresponds to something real — a published IEC edition, or the
syntax a real toolchain accepts (see [ADR-0036](../adrs/0036-no-ironplc-dialect.md):
IronPLC never invents a dialect of its own). A file belongs to exactly one
dialect (see [ADR-0012](../adrs/0012-accept-vendor-dialect-files-as-is.md)).

*Canonical uses:* "the CODESYS dialect", "select a dialect", "dialect
extension", "per-file dialect detection", "the `--dialect` flag".

*Do not say:* "vendor dialect" (redundant — a dialect is already the syntax a
target accepts; just say "dialect", or name the dialect).

### Edition

A published version of the IEC 61131-3 standard — **Edition 2** (2003) or
**Edition 3** (2013). Editions are additive: a later edition includes the
earlier one's features. An edition is the strict-standard baseline a dialect
builds on; it is not vendor-specific.

### Extension

A single **non-standard syntax feature** — something beyond strict IEC 61131-3
that a dialect may enable, gated by an `--allow-*` [flag](#flag). "Extension"
always refers to *syntax the parser recognizes*, never to a runtime library.

*Canonical uses:* "dialect extension", "the `--allow-sizeof` extension", "this
extension is not part of the standard".

*Do not say:* "vendor extension" — an extension is defined by the *syntax* it
adds, not by a company. Name it a "dialect extension" or just an "extension",
and put the "which vendors accept it" mapping in the dialect table.

> **Beware the overload.** "Extension" is used in three unrelated senses in this
> project: (1) a syntax feature, as defined here; (2) the **Extension Library**
> (runtime functions/function blocks/variables such as `SIZEOF`); and (3) the
> **VS Code extension**. When ambiguous, qualify it: *dialect extension*,
> *extension library*, *editor extension*.

### Flag

An `--allow-*` command-line (and LSP/MCP) option that enables one
[extension](#extension) on top of the selected dialect. A dialect preset is
exactly a named bundle of flags. Flags only ever *enable* features; they never
disable what a dialect includes.

*Do not say:* "vendor flag" / "vendor-extension flag" — say "dialect flag",
"extension flag", or just "`--allow-*` flag".

### Vendor

A real PLC **product or company** whose platform a program targets — e.g.
CODESYS, Beckhoff TwinCAT, Siemens, RuSTy. A vendor is a concrete thing in the
world with its own toolchain, its own accepted syntax (its dialect), and its own
runtime library.

"Vendor" is the correct word whenever the subject is the *product/company or its
runtime*: `vendor toolchain`, `vendor files` (files authored for a vendor's
tool), `vendor platform`, `vendor-compatible` (a dialect compatible with a
vendor), `the vendor's library`.

*Do not* stretch "vendor" to mean syntax. If the sentence is about what the
parser accepts, the word is [dialect](#dialect) or [extension](#extension).

### Vendor compatibility library (vendor shim)

A library of IEC 61131-3 POUs (functions, function blocks) that **emulates a
vendor's runtime library** so that code written for that vendor compiles and
behaves correctly under IronPLC — for example, a `TIME()` function that returns
system uptime the way CODESYS's does.

This is a **vendor** concept, not a dialect one: it is about *runtime behavior
and libraries*, not syntax. There is deliberately no such thing as a "dialect
compatibility library" — a dialect has no runtime to emulate.

### Extension Library

IronPLC's own set of runtime functions, function blocks, and variables that go
beyond the standard (e.g. `SIZEOF`, `__SYSTEM_UP_TIME`). Distinct from a
[vendor compatibility library](#vendor-compatibility-library-vendor-shim): the
Extension Library is IronPLC's, and some of its members (e.g. the
`__SYSTEM_UP_TIME` globals) are IronPLC runtime conventions rather than any
vendor's feature — so they are *not* "vendor extensions" and must not be
described as such.

### Standard (IEC 61131-3)

The published IEC 61131-3 language, in a given [edition](#edition). "Standard"
syntax is what strict `iec61131-3-ed2` / `iec61131-3-ed3` accept with no
extensions. Anything an `--allow-*` flag enables is, by definition,
*non-standard*.

---

## How the terms compose

- A **vendor** (Beckhoff TwinCAT) is compatible with IronPLC through two
  separate things: a **dialect** (the syntax IronPLC parses for it) and,
  optionally, a **vendor compatibility library** (POUs emulating its runtime).
- A **dialect** = an **edition** baseline + a set of **extensions**, each turned
  on by a **flag**.
- A **standard** program uses no extensions; it loads in any conformant tool.
  A program that uses extensions loads in the vendor(s) whose dialect includes
  them — never in a hypothetical "IronPLC dialect" ([ADR-0036](../adrs/0036-no-ironplc-dialect.md)).

---

## Naming rules (quick reference)

| Say this | Not this | Because |
|---|---|---|
| dialect | vendor dialect | "dialect" already means the accepted syntax |
| dialect extension / extension | vendor extension | an extension is defined by syntax, not a company |
| dialect flag / `--allow-*` flag | vendor flag, vendor-extension flag | the flag gates syntax |
| non-standard syntax | vendor syntax | it's about the standard, not a vendor |
| vendor toolchain / vendor files / vendor library | *(keep — correct)* | these really are about the product/runtime |
| vendor compatibility library / vendor shim | dialect compatibility library | a dialect has no runtime to emulate |

**Preserved verbatim (external, do not rename):** names defined by the PLCopen /
IEC TC6 XML schema — `vendor`, `vendorElement`, "Vendor specific" — in
`compiler/resources/schemas/tc6_xml_v201.xsd` and the grammar that mirrors it.
These are externally-defined identifiers, not IronPLC vocabulary.

---

## Maintaining this file

- **Add a term the moment it crystallizes.** If a session or PR coins a new
  concept, define it here before it spreads.
- **Challenge conflicts on sight.** When wording in code or docs contradicts a
  definition here, that is a bug to fix, not a variation to tolerate.
- **New syntax feature?** It is a *dialect extension*; describe it by the syntax
  it adds and record which dialects enable it in the dialect table. See
  [Syntax Support Guide](syntax-support-guide.md).
