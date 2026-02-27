# Accept Vendor Dialect Files As-Is

status: proposed
date: 2026-02-27

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

### How this applies to current vendor dialects

| Vendor | File formats | Key extensions to parse |
|--------|-------------|----------------------|
| Siemens (SCL) | `.scl` | `#var` prefix, `REGION`/`END_REGION`, `{ pragma }`, `"quoted names"`, `VAR_STAT`, `VERSION`, `DATA_BLOCK`, `ORGANIZATION_BLOCK` |
| Beckhoff (TwinCAT) | `.TcPOU`, `.TcGVL`, `.TcDUT` (XML) | `INTERFACE`, `METHOD`, `PROPERTY`, `EXTENDS`, `IMPLEMENTS`, access modifiers, `POINTER TO`/`REFERENCE TO`, `VAR_INST`, `UNION`, `{attribute}` pragmas |
| Standard | `.st`, `.iec`, `.xml` (PLCopen) | Baseline IEC 61131-3 — already supported |

### Dialect detection strategy

The parser determines the dialect from the file extension and (where applicable) file content:

- `.scl` → Siemens SCL dialect
- `.TcPOU`, `.TcGVL`, `.TcDUT` → Beckhoff TwinCAT (XML wrapper; ST inside CDATA already uses TwinCAT parser)
- `.st`, `.iec` → Standard IEC 61131-3 by default; optionally configurable
- `.xml` → PLCopen XML

Dialect selection enables the appropriate set of lexer extensions and parser grammar rules. The standard dialect remains the default — vendor extensions are additive, not replacing standard behavior.

### Consequences

* Good, because users can open any Siemens or Beckhoff project and immediately get value from IronPLC's analysis on the standard-compliant portions of their code
* Good, because error positions always point into the user's original source file — no intermediate representations or preprocessed copies
* Good, because the approach is incremental — parsing a vendor construct and representing it in the AST is the first step; semantic analysis can follow independently
* Good, because dialect detection from file extensions is simple, deterministic, and requires no user configuration for the common case
* Good, because the existing parser architecture (logos lexer + hand-written recursive descent) naturally supports additive token types and grammar rules without architectural changes
* Bad, because each vendor dialect adds maintenance surface — new tokens, grammar rules, AST nodes, and test fixtures
* Bad, because users may expect semantic analysis of vendor-specific constructs (e.g., type checking `POINTER TO` dereferences) once parsing succeeds — clear messaging about "parsed but not yet analyzed" is needed
* Bad, because dialect interactions create combinatorial complexity — a file might use constructs from multiple dialects if the user mixes conventions (rare in practice since files come from specific vendor tools)
* Neutral, because the lexer and parser already support one vendor extension mechanism (TwinCAT XML wrappers, OSCAT comment removal, `allow_c_style_comments` option) — this decision formalizes and extends the existing pattern

### Confirmation

For each vendor dialect added, verify:
1. **Parse-clean on real projects** — take 3+ open-source projects from that vendor ecosystem and confirm zero parse errors on all files
2. **Position fidelity** — confirm that all diagnostic positions point into the original source file, not into any intermediate
3. **No standard regression** — confirm that enabling a vendor dialect does not change the parse result of any standard IEC 61131-3 file
4. **Incremental semantic value** — confirm that existing semantic analysis (type checking, variable resolution, etc.) still runs on the standard-compliant portions of vendor files

## Pros and Cons of the Options

### Accept As-Is (chosen)

Extend the parser to natively recognize vendor-specific syntax, controlled by dialect configuration detected from file extension.

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
| `ParseOptions::allow_c_style_comments` | Controls whether `//` comments are accepted | Dialect-specific syntax toggle |
| `preprocessor.rs` (OSCAT comments) | Strips vendor-specific comment patterns | Vendor-specific preprocessing |
| `twincat_parser.rs` | Parses Beckhoff XML wrapper format | Vendor-specific file format handling |
| `FileType` enum | Routes to different parsers by extension | Dialect detection by extension |

This ADR formalizes these existing patterns into a deliberate strategy rather than letting them accumulate ad-hoc.

### Scope boundary: parsing vs. semantic analysis

This decision covers **parsing** — the ability to read vendor files without errors and produce an AST. It does not commit to **semantic analysis** of all vendor constructs. The implementation strategy is:

1. **Parse**: Recognize vendor syntax and represent it in the AST (possibly as opaque/unanalyzed nodes)
2. **Analyze incrementally**: Add semantic analysis for vendor constructs over time, starting with the ones that have the most impact on standard analysis (e.g., `EXTENDS` affects type hierarchies, which affects type checking)
3. **Report clearly**: When a parsed-but-not-analyzed construct affects analysis results, emit a clear diagnostic (e.g., "P9XXX: IronPLC parsed this METHOD declaration but does not yet analyze method calls")

This separation ensures users get value immediately (parse-clean files, diagnostics on standard portions) while the project incrementally grows its vendor-specific analysis capabilities.
