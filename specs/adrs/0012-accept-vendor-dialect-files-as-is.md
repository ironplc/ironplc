# Accept Vendor Dialect Files As-Is

status: proposed
date: 2026-02-27
amended: 2026-09-11 (Implementation Status added; status unchanged)
amended: 2026-09-11 (the dialect recorded as declared by the user rather than
inferred from the file extension, and its granularity as a default rather than
a ceiling; status unchanged — see the amendment note under *More Information*)

> **Terminology note (added later):** This ADR predates
> [`specs/steering/glossary.md`](../steering/glossary.md), which now draws a
> firm line between a *dialect* (the syntax the parser accepts) and a *vendor*
> (a product/runtime). This ADR's text uses "vendor" loosely in both senses and
> is preserved as written; for current terminology, defer to the glossary.

## Context and Problem Statement

IEC 61131-3 defines the standard syntax for Structured Text, but no major PLC vendor ships a strict implementation. Every vendor extends the language with proprietary syntax: Siemens SCL adds `#` variable prefixes, `REGION`/`END_REGION` blocks, and curly-brace pragmas; Beckhoff TwinCAT adds object-oriented features (`INTERFACE`, `METHOD`, `PROPERTY`, `EXTENDS`), `POINTER TO`/`REFERENCE TO` types, and `VAR_INST` sections; other vendors make similar additions. These extensions are not cosmetic — they appear on virtually every line of real-world vendor-authored code.

Users have existing PLC projects written for specific vendor platforms. When they point IronPLC at those projects, every vendor-specific construct produces a parse error, making IronPLC useless for their existing code. The question is: should IronPLC require users to convert or clean up their files before analysis, or should it accept vendor-specific files exactly as they are?

## Decision Drivers

* **Zero-friction adoption** — the single most important factor for tool adoption is that it works on existing code without modification; requiring preprocessing or format conversion is a barrier that most users will not cross
* **Ecosystem breadth** — PLC code exists in Siemens TIA Portal projects (.scl), Beckhoff TwinCAT projects (.TcPOU/.TcGVL/.TcDUT), CODESYS projects, and others; supporting only standard IEC 61131-3 excludes the majority of real-world code
* **Fidelity of diagnostics** — error positions must point into the user's original file, not into a preprocessed intermediate; users should never see errors referencing code they didn't write
* **Incremental value** — parsing vendor syntax does not require fully implementing vendor semantics; IronPLC can parse and ignore vendor constructs it doesn't analyze yet, providing value through the standard-compliant analysis it already has
* **Maintenance cost** — each vendor dialect adds lexer tokens, parser grammar rules, and test fixtures; this cost must be bounded and manageable

## Considered Options

* **Accept as-is** — extend the parser to recognize vendor-specific syntax natively, controlled by dialect configuration
* **Require conversion** — expect users to export standard IEC 61131-3 ST (or PLCopen XML) from their vendor tool before using IronPLC
* **Preprocessing pipeline** — accept vendor files but strip/transform vendor extensions in a preprocessing pass before the standard parser sees them
* **Best-effort with warnings** — parse standard constructs and emit warnings for unrecognized vendor syntax, skipping what can't be parsed

## Decision Outcome

Chosen option: "Accept as-is", because requiring users to modify or convert their files before IronPLC can read them means IronPLC provides no value on real-world projects. The preprocessing option appears simpler but destroys source position fidelity and creates a fragile translation layer. The best-effort option produces noisy, unreliable results that erode trust.

The principle is: **IronPLC must be able to parse any file that the vendor's own toolchain accepts, producing zero parse errors on syntactically valid vendor code.** Semantic analysis of vendor-specific constructs is a separate, incremental concern — the parser accepts the syntax first, and semantic support follows over time.

### No mixing of vendor dialects within a file

A single file belongs to exactly one vendor dialect. IronPLC does not allow mixing Siemens and Beckhoff constructs (or any other combination of vendor-specific extensions) in the same file. Only the dialect that covers a file is enabled while parsing it.

A workspace (project) may contain files from different vendors — for example, some `.scl` files and some `.TcPOU` files — and each file is parsed against its own dialect. But within any single file, only one vendor's extensions are valid.

Which dialect covers which file is *declared*, never inferred from the file's
extension — see
[File format from the extension, dialect declared by the user](#file-format-from-the-extension-dialect-declared-by-the-user).
Today one declaration covers a whole compilation; declaring a dialect per file
is the planned exception for projects that combine platforms, and it is
described there.

The reason is **round-trip fidelity**: a file written for Siemens TIA Portal should remain valid Siemens SCL, and a file written for Beckhoff TwinCAT should remain valid TwinCAT ST. If IronPLC accepted a hybrid file that used constructs from multiple vendors, that file would not be loadable in *any* vendor's toolchain. Accepting mixed-dialect files would mean IronPLC is creating a new dialect that doesn't exist anywhere else — the opposite of "accept as-is."

### How this applies to current vendor dialects

| Vendor | File formats | Key extensions to parse |
|--------|-------------|----------------------|
| Siemens (SCL) | `.scl` | `#var` prefix, `REGION`/`END_REGION`, `{ pragma }`, `"quoted names"`, `VAR_STAT`, `VERSION`, `DATA_BLOCK`, `ORGANIZATION_BLOCK` |
| Beckhoff (TwinCAT) | `.TcPOU`, `.TcGVL`, `.TcDUT` (XML) | `INTERFACE`, `METHOD`, `PROPERTY`, `EXTENDS`, `IMPLEMENTS`, access modifiers, `POINTER TO`/`REFERENCE TO`, `VAR_INST`, `UNION`, `{attribute}` pragmas |
| Standard | `.st`, `.iec`, `.xml` (PLCopen) | Baseline IEC 61131-3 — already supported |

### File format from the extension, dialect declared by the user

Two separate things must be settled before a vendor file can be read, and only
one of them comes from the file name.

**The file format** — how the source text is wrapped — is detected from the
extension (`FileType::from_path`), or from the content when there is no name to
read (the playground, the MCP server):

- `.st`, `.iec` → plain Structured Text
- `.TcPOU`, `.TcGVL`, `.TcDUT`, `.TcIO` → Beckhoff TwinCAT XML wrapper; the ST
  inside the CDATA sections is handed to the ordinary parser
- `.xml` → PLCopen XML
- `.scl` → Siemens SCL (not implemented; see *Implementation Status*)

**The dialect** — which syntax the parser accepts — is *declared* by the user,
as a named preset (`--dialect twincat`) or as individual `--allow-*` flags, and
defaults to strict IEC 61131-3 Edition 2 when neither is given. Declaring a
dialect enables the appropriate set of lexer extensions and parser grammar
rules; extensions are additive, never replacing standard behavior. *How much*
one declaration covers is a separate question from who makes it, and is
addressed under [granularity](#granularity-one-declaration-per-run-by-default)
below.

Reading a TwinCAT project is therefore both together:

```shell
ironplcc check --dialect twincat MySolution.sln
```

The extension gets the Structured Text out of the XML; the flag says which
syntax to hold it to.

#### Why the extension does not imply a dialect

The file's container records where the code was *stored*, not which language
rules its author wants it *checked against*. Deriving the dialect from the
extension conflates the two, and costs three things:

1. **It would remove the strictness check that makes IronPLC worth running in a
   vendor environment.** Holding a `.TcPOU` to strict Edition 2 is a legitimate
   and deliberately supported request: a team that wants portable, standard code
   while working in TwinCAT XAE can point IronPLC at its solution with no dialect
   flag and be told exactly which constructs would not survive a move to another
   toolchain. No vendor toolchain offers that check on its own files. Inferring
   `twincat` from `.TcPOU` would silently take it away — accepting everything and
   reporting nothing is the wrong answer for that user. The dialect is a property
   of the *question being asked*, and the user asks it.
2. **It would make the accepted syntax invisible.** What a run accepted should be
   readable from the invocation, not reconstructed from a table of extensions.
   This is the same commitment as [ADR-0022](0022-edition-3-compiler-flag.md)
   (Edition 3 is an explicit opt-in) and
   [ADR-0036](0036-no-ironplc-dialect.md) (editions and dialects are explicit
   selections, and nothing lenient is on by default).
3. **Extensions do not map cleanly onto dialects anyway.** `.st` is what every
   CODESYS-family tool and most textbooks write; picking one dialect as its
   meaning would be a guess. Only the TwinCAT extensions name a single vendor,
   and they name it because they are a *format*, not because they fix a syntax.

The cost is accepted: a user pointing IronPLC at vendor code for the first time
can see errors on syntax their own toolchain accepts, until they pass the flag.
Accordingly, the accept-as-is promise in this ADR should be read as: **IronPLC
must be able to parse any file that the vendor's own toolchain accepts, when the
user selects that vendor's dialect.** The promise is about the parser's
capability, not about guessing intent from a file name. The gap is closed by
making the flag discoverable rather than by inferring it — a rejected extension
is diagnosed with help text telling the user to select a dialect that supports
it ([ADR-0036](0036-no-ironplc-dialect.md)), and the TwinCAT source-format
reference passes `--dialect twincat` in its first example.

#### Granularity: one declaration per run, by default

One declaration covering the whole run is the **default**, not a ceiling.

Today the user declares a dialect once and it covers every source file in the
compilation. Even now that is not the whole story *inside* a compilation:
bundled compatibility library bodies are ST compiled alongside user source
([ADR-0042](0042-library-functions-over-compiler-intrinsics.md)) and are parsed
against strict Edition 2 whatever the user declares, so one compilation already
holds more than one dialect configuration.

For user source, IronPLC expects to grow a way to declare a dialect for
individual files, so that a project combining files written for different
platforms is one compilation rather than several. This is an exception, and
should read as one: most projects target a single platform and will never reach
for it, and the single per-run declaration stays the common path and the
default. [ADR-0049](0049-behavior-policies-selected-at-compile-time.md) already
depends on this — it compiles behavior policies into the bytecode rather than
configuring them on the VM precisely because selection has to be per file,
"which per-file dialects and mixed compositions require."

What finer granularity does not change is *who* decides. A per-file dialect is
declared too — named by the user, in the invocation or in project configuration
— never inferred from the file's extension, which continues to say only what the
format is. Everything under *Why the extension does not imply a dialect* holds
unchanged at file granularity: a `.TcPOU` declared as Edition 2 is checked as
Edition 2, whether that declaration covers one file or the whole run. Nor does
it weaken *No mixing of vendor dialects within a file* — each file is still
parsed against exactly one dialect; what may differ is which one, file to file.

### Consequences

* Good, because users can open any Siemens or Beckhoff project and immediately get value from IronPLC's analysis on the standard-compliant portions of their code
* Good, because error positions always point into the user's original source file — no intermediate representations or preprocessed copies
* Good, because the approach is incremental — parsing a vendor construct and representing it in the AST is the first step; semantic analysis can follow independently
* Good, because splitting format detection from dialect declaration keeps each one honest — the format is inferred from the extension, where the answer is deterministic and uninteresting, and the accepted syntax stays a visible, explicit property of what the user declared
* Good, because a vendor's files can be held to a stricter dialect than the vendor's own toolchain enforces — checking a TwinCAT project against strict Edition 2 to find what is not portable is a supported use, and one no vendor toolchain offers
* Good, because the existing parser architecture (logos lexer + hand-written recursive descent) naturally supports additive token types and grammar rules without architectural changes
* Bad, because each vendor dialect adds maintenance surface — new tokens, grammar rules, AST nodes, and test fixtures
* Bad, because a user reading vendor code for the first time must know to pass `--dialect`; without it the first file produces errors on syntax the vendor accepts, which is the accept-as-is promise failing at the point of first contact — mitigated by help text on each rejection pointing at dialect selection, and by the TwinCAT source-format reference leading with the flag
* Bad, because users may expect semantic analysis of vendor-specific constructs (e.g., type checking `POINTER TO` dereferences) once parsing succeeds — clear messaging about "parsed but not yet analyzed" is needed
* Neutral, because dialect interactions are avoided by design — each file is parsed against exactly one dialect, so there is no combinatorial complexity within a file; a compilation may hold files covered by different dialects (today for compatibility library bodies, later by per-file declaration), and each of those files is still single-dialect
* Neutral, because the lexer and parser already support one vendor extension mechanism (TwinCAT XML wrappers, OSCAT comment removal, `allow_c_style_comments` option) — this decision formalizes and extends the existing pattern

### Confirmation

For each vendor dialect added, verify:
1. **Parse-clean on real projects** — take 3+ open-source projects from that vendor ecosystem and confirm zero parse errors on all files, with that vendor's dialect selected
2. **Position fidelity** — confirm that all diagnostic positions point into the original source file, not into any intermediate
3. **No standard regression** — confirm that enabling a vendor dialect does not change the parse result of any standard IEC 61131-3 file
4. **Incremental semantic value** — confirm that existing semantic analysis (type checking, variable resolution, etc.) still runs on the standard-compliant portions of vendor files

## Implementation Status (as of 2026-09-11)

This ADR is still `proposed`, and that is accurate: one of the two vendor
dialects it names is built and the other is not. Recorded here so a reader does
not take the table above as a description of what IronPLC reads today. The gap
that remains for the dialect that *is* built is tracked by
[issue #1685](https://github.com/ironplc/ironplc/issues/1685); the other gap
that issue raised — this ADR describing a dialect detection the code does not
perform — is closed by the amendment noted under *More Information*, which
corrects the record rather than the code.

What landed:

* **Beckhoff TwinCAT, all but `PROPERTY`.** `.TcPOU`, `.TcGVL` and `.TcDUT` are recognized
  by `FileType::from_path`, and so is `.TcIO` — TwinCAT's `INTERFACE` object
  type, which this ADR's table does not list. The XML wrapper is parsed by
  `sources/src/xml`, and the ST inside goes through the ordinary parser with the
  TwinCAT extensions enabled. `INTERFACE`, `METHOD`, `EXTENDS`, `IMPLEMENTS`,
  `POINTER TO`, `REFERENCE TO` and `{attribute}` pragmas all have flags and a
  `twincat` preset that bundles them.
* **The principle itself is in force and is cited as policy.** ADR-0036 depends
  on it — the reason IronPLC defines no dialect of its own is that every flag
  bundle must describe a real toolchain.
* **The format/dialect split**, as the amended *File format from the extension,
  dialect declared by the user* section above now describes it: `FileType`
  picks the parser from the extension, and `--dialect` / `--allow-*` declare the
  syntax. `ironplcc`, the LSP server, the MCP server, the VS Code extension
  (`ironplc.dialect`) and the playground all expose the declaration, and the
  TwinCAT source-format reference leads with the flag.
* Position fidelity (Confirmation item 2), substantially: `sources/src/xml/position.rs`
  maps a position in the ST body back through the CDATA to a line and column in
  the original `.TcPOU`, so a diagnostic points at the vendor's file rather than
  at an intermediate. It is not uniform — some transformed nodes still fall back
  to a file-level span carrying no position — so the item is not fully closed.

What did not:

* **`PROPERTY` is not built** — the one TwinCAT POU construct missing from
  the row above. `PROPERTY` is not a keyword at any dialect
  setting — it lexes as an identifier, there is no `PropertyDeclaration` AST
  node, no `<Property>` element handler in `sources/src/xml`, and no
  `--allow-*` flag for it. A `.TcPOU` carrying a property is rejected with
  `P0002` under `--dialect twincat`. ADR-0041 decides the dispatch semantics
  it would need; tracked by
  [issue #1692](https://github.com/ironplc/ironplc/issues/1692).
* **Siemens SCL is not built.** `.scl` is not a `FileType`, so the compiler does
  not read Siemens files at all. Every construct in the Siemens row of the table
  above — `#var`, `REGION`, `"quoted names"`, `VAR_STAT`, `DATA_BLOCK`,
  `ORGANIZATION_BLOCK` — is unimplemented. `specs/design/siemens-scl-dialect.md`
  is a design, not a description.
* **Per-file dialect declaration, for user source.** One declaration covers the
  whole compilation: `CompilerOptions` is held once per project
  (`project/src/project.rs`) and every source is parsed against it. The one
  place a second configuration already exists is bundled compatibility library
  bodies, which `sources/src/libraries` parses with `CompilerOptions::default()`
  — strict Edition 2 — whatever the user declared. Per-file declaration is an
  expected extension rather than a gap in this decision, so it does not hold the
  status at `proposed`.
* **Confirmation item 1 has no harness.** No test takes 3+ open-source projects
  from a vendor ecosystem and asserts zero parse errors. The nearest thing is the
  OSCAT corpus case in `parser/src/tests/corpus.rs`, which is a single vendor-
  neutral `.st` file. Without that harness, "parses everything the vendor's
  toolchain accepts" is an aspiration rather than a measured property, for
  TwinCAT as much as for Siemens.

The decision stands; the work is unfinished. The parse-clean corpus is the one
piece that would let this flip to `accepted` for the dialect that exists.

## Pros and Cons of the Options

### Accept As-Is (chosen)

Extend the parser to natively recognize vendor-specific syntax, controlled by the dialect the user selects (`--dialect` / `--allow-*`), with the file format detected from the extension.

* Good, because the user experience is seamless — open a file, get results
* Good, because diagnostic positions are always accurate — the parser reads the original file directly
* Good, because the architecture matches what the project already does for TwinCAT XML and OSCAT comments
* Good, because vendor constructs that don't affect standard analysis can be parsed and represented as opaque AST nodes (no semantic implementation required initially)
* Bad, because the parser grows in complexity with each dialect — must be managed through clear module boundaries
* Bad, because testing requires real vendor project files as fixtures

### Require Conversion

Expect users to export standard IEC 61131-3 from their vendor tool.

* Good, because the parser stays simple — only standard syntax
* Good, because there's no dialect complexity in the codebase
* Bad, because most vendor tools do not have a "export as standard IEC 61131-3" feature — Siemens TIA Portal exports .scl (which is SCL, not standard ST), TwinCAT exports .TcPOU (which is TwinCAT XML, not PLCopen XML)
* Bad, because this is a hard adoption barrier — users must learn a conversion process before they get any value
* Bad, because round-tripping through conversion loses vendor-specific information that users need
* Bad, because it contradicts the "works on your existing code" value proposition that makes development tools compelling

### Preprocessing Pipeline

Accept vendor files but transform them into standard IEC 61131-3 in a preprocessing pass (e.g., strip `REGION`/`END_REGION`, remove `#` prefixes, convert pragmas to comments).

* Good, because the core parser stays clean — only sees standard syntax
* Good, because preprocessing is conceptually simple for some constructs (strip `REGION`, remove `#`)
* Bad, because position mapping from preprocessed text to original file is fragile and error-prone — the TwinCAT XML parser already demonstrates this complexity (200+ lines for CDATA position adjustment), and that's for a structured XML format, not arbitrary text transformations
* Bad, because not all vendor constructs are strippable — `EXTENDS`, `METHOD`, `PROPERTY`, `POINTER TO` change the grammar structure, not just add removable markers
* Bad, because the preprocessing pass itself is a parser for vendor syntax — so the total complexity is higher than native parsing (you build two parsers: the preprocessor and the standard parser)
* Bad, because preprocessor bugs produce confusing errors — the user sees an error about code they didn't write, at a position that doesn't correspond to their file

### Best-Effort with Warnings

Parse what the standard parser can handle, emit warnings for unrecognized constructs, and attempt error recovery.

* Good, because no upfront investment in vendor dialects is needed
* Good, because some value is provided immediately
* Bad, because real vendor files have vendor-specific constructs on nearly every line (Siemens `#` prefix, Beckhoff pragmas) — "best-effort" means hundreds of warnings per file, which is worse than useless
* Bad, because error recovery after an unrecognized construct often cascades into false errors on subsequent valid code — one `REGION` block at the top of a file can make the entire file unparseable
* Bad, because users lose trust in a tool that produces noise — a wall of warnings is worse than a clear "not supported" message
* Bad, because there's no clear path to improvement — each recovered error is an ad-hoc heuristic rather than a deliberate grammar extension

## More Information

### Why vendor dialects are not "edge cases"

A survey of open-source PLC code on GitHub reveals that the vast majority of IEC 61131-3 code is written for specific vendor platforms:

- **Siemens TIA Portal** is the most widely used PLC programming environment globally. All SCL code uses `#` prefixes, pragmas, and double-quoted names. The [SASE-Space/open-process-library](https://github.com/SASE-Space/open-process-library) is a representative example.
- **Beckhoff TwinCAT** has the largest open-source IEC 61131-3 community. Nearly all TwinCAT code uses `METHOD`, `PROPERTY`, pragmas, and `EXTENDS`. Libraries like the TwinCAT BSD samples demonstrate pervasive use of OOP extensions.
- **CODESYS**-based platforms (Schneider, ABB, Wago, and others) share many of the same extensions as TwinCAT (CODESYS is the upstream IDE).

Standard-only IEC 61131-3 Structured Text is predominantly found in textbooks and standards documents, not in production code. An IEC 61131-3 tool that only handles the standard is an academic exercise.

### Relationship to existing architecture

IronPLC already uses a dialect-like approach in several places:

| Existing mechanism | What it does | Parallel |
|---|---|---|
| `CompilerOptions::allow_c_style_comments` | Controls whether `//` comments are accepted | Dialect-specific syntax toggle |
| `preprocessor.rs` (OSCAT comments) | Strips vendor-specific comment patterns | Vendor-specific preprocessing |
| `twincat_parser.rs` | Parses Beckhoff XML wrapper format | Vendor-specific file format handling |
| `FileType` enum | Routes to different parsers by extension | File-format detection by extension (the dialect is selected separately) |

This ADR formalizes these existing patterns into a deliberate strategy rather than letting them accumulate ad-hoc.

### Scope boundary: parsing vs. semantic analysis

This decision covers **parsing** — the ability to read vendor files without errors and produce an AST. It does not commit to **semantic analysis** of all vendor constructs. The implementation strategy is:

1. **Parse**: Recognize vendor syntax and represent it in the AST (possibly as opaque/unanalyzed nodes)
2. **Analyze incrementally**: Add semantic analysis for vendor constructs over time, starting with the ones that have the most impact on standard analysis (e.g., `EXTENDS` affects type hierarchies, which affects type checking)
3. **Report clearly**: When a parsed-but-not-analyzed construct affects analysis results, emit a clear diagnostic (e.g., "P9XXX: IronPLC parsed this METHOD declaration but does not yet analyze method calls")

This separation ensures users get value immediately (parse-clean files, diagnostics on standard portions) while the project incrementally grows its vendor-specific analysis capabilities.

### Amendment, 2026-09-11: dialect selection is explicit, not inferred

The section now titled *File format from the extension, dialect declared by the
user* was originally titled *Dialect detection strategy* and said that "the
parser determines the dialect from the file extension and (where applicable)
file content", mapping `.scl` to Siemens, `.TcPOU`/`.TcGVL`/`.TcDUT` to
Beckhoff, and `.st`/`.iec` to standard IEC 61131-3 "by default; optionally
configurable". The *No mixing* section said the same, and the consequences
claimed dialect detection "requires no user configuration for the common case".

That mechanism was never built, and on review it is not the one IronPLC wants.
What was built separates the two questions: the extension picks the *file
format* and the user declares the *dialect*. Keeping the dialect an explicit
choice is what allows a user to hold vendor-housed files to a stricter dialect
than the vendor's own toolchain enforces — checking a TwinCAT solution against
strict Edition 2 to find what is not portable. Inferring the dialect from the
extension would make that impossible to ask for, and it is one of the few checks
IronPLC can offer that a vendor's own tools cannot. The record has been corrected
to describe the design that exists and to say why the user picks the dialect.

The per-file granularity the original section assumed is *not* what changed,
and is preserved: a workspace may hold files covered by different dialects, and
each file is parsed against exactly one. What changed is that the dialect
covering a file is declared rather than derived from its extension, and that
today one declaration covers a whole run — a default, not a ceiling, as the
granularity subsection now records.

The decision outcome is untouched: accept-as-is stands, and no mixing within a
file stands. Only the mechanism section, the consequences that rested on it, and
the cross-references to it were amended, per the
[ADR amendment rules](../steering/development-standards.md#amending-an-adr).
This closes the second of the two gaps raised in
[issue #1685](https://github.com/ironplc/ironplc/issues/1685); the first —
no corpus harness measuring the accept-as-is promise — remains open, and is
why the status stays `proposed`.
