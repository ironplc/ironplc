# Design: Compatibility Libraries

## Overview

This document describes a mechanism for supporting vendor-defined constants,
functions, and function blocks — such as TwinCAT's `PI`, `Tc2_Math` functions,
and the OSCAT library — **without** baking any of them into the compiler as
keywords or per-symbol feature flags.

The core idea: IronPLC ships a collection of **compatibility libraries**. Each
is a named, dormant bundle of declarations (constants, function signatures,
function block types) using the *exact* names the originating vendor uses. A
library is activated out of band — either because a project file references it,
or because the user activated it explicitly on the command line. Activation
injects the library's declarations into the compilation unit, where they resolve
by the same machinery user source does.

> **Requirement markers.** This document carries `REQ-CL-*` requirement markers
> (area `CL` = compatibility libraries) for each testable claim, per the
> [design requirement](../steering/development-standards.md). They become
> build-enforced only once a crate's `build.rs` lists this doc; the
> [implementation plan](../plans/2026-08-04-compatibility-libraries.md) wires
> each owning crate and adds the matching `#[spec_test]` conformance tests
> (using `#[ignore]` for claims not yet implemented). Until then the markers are
> inert.

## The Portability Promise

The value proposition — and the invariant that constrains this entire design —
is bidirectional, zero-diff, same-behavior portability:

1. A source file written for TwinCAT (or CODESYS, or OSCAT) compiles **and runs
   identically** in IronPLC with **no edits**.
2. A file that compiles in IronPLC targeting a given vendor goes **back** to that
   vendor with **no edits**.
3. **The source text is sacred.** IronPLC may not require, inject, or rename
   anything inside a POU to make compatibility work.

Two consequences follow directly, and both rule out mechanisms that were
considered earlier:

- **No name prefixing.** TwinCAT code says `FLOOR`, not `Tc2_Math.FLOOR`. If
  IronPLC injected a namespace qualifier, the in-memory representation would
  diverge from the source, and `plc2plc` would render the qualifier back out —
  breaking round-trip byte-fidelity. Therefore compatibility symbols are always
  injected **flat**, under their exact vendor names.
- **No required in-source directive.** In TwinCAT the set of referenced
  libraries lives in the *project* (`.plcproj` / `.tsproj`), never in the POU
  body. IronPLC must not require a source-level directive (e.g. a `{library}`
  pragma) that vendor files do not carry. Therefore library activation is
  **entirely out of band** — it never touches POU text.

### Scope of the promise

The promise applies to the **paved path**: activating a library (or a dialect
that references one) and staying within it. It is explicitly **not** a promise
that:

- Arbitrary combinations of `--allow-*` flags cohere. Per
  [ADR-0038](../adrs/0038-no-restrictions-on-flag-combinations.md), flag
  combinations are unrestricted; a user can enable flags that no real vendor
  environment combines and get code that compiles here but nowhere else.
- Mixing libraries — **especially across vendors** — works. A single real
  project targets one environment; combining, say, a TwinCAT library and a
  Siemens library is not a supported configuration and carries no guarantee.

Both are the unpaved path. The guarantee is: *stay on the paved path and it just
works by default.*

### Safety first

Where compatibility cannot be delivered faithfully, the compiler must **refuse
to compile** rather than silently produce code whose behavior it cannot
reproduce. This is a safety requirement, not a convenience one. Concretely:

- An unsupported library configuration is diagnosed and rejected, not compiled
  into something with undefined behavior; the failure mode is fixed — "does not
  compile," never "compiles and misbehaves."

The first increment ships only constants (`Tc2_Math`'s `PI`), so there are no
callable library POUs that could be unimplemented. The concrete enforcement — a
call to a declare-only POU is a compile error — arrives with *bindings* (see
*Future Goals*).

## Goals

- Support vendor constants (`PI`, `e`), functions (`Tc2_Math` numerics, OSCAT
  functions), and function blocks under their exact vendor names.
- Scale to arbitrarily many libraries with **zero** new per-symbol flags.
- Restrict availability by activation: a non-standard name like `PI` is only in
  scope when a library that provides it has been explicitly activated. Activation
  is independent of dialect (see *Dialect and vendor are different concepts*).
- Auto-activate from a real vendor project file, so a genuine TwinCAT project
  drops in unchanged, *including its own statement of which libraries it uses*.
- Preserve bidirectional round-trip fidelity (`plc2plc` renders user source
  unchanged; injected library declarations are never emitted as user source).
- **Support the playground.** Libraries are additional plain-text files that can
  be served and loaded, so the browser playground can activate a library and
  compile against it, not just the CLI.

## Non-Goals

- A general package manager, remote fetch, or dependency resolution across the
  network. Libraries are bundled with the compiler (or added locally).
- **Moving compiler-only intrinsics into libraries.** Names with a `__` prefix
  (e.g. `__SYSTEM_UP_TIME`, `__ISVALIDREF`) mark things that *only the compiler
  can provide*. That is precisely the functional line between what already
  exists (compiler-provided) and what this design adds (libraries). Compiler-only
  intrinsics stay compiler-provided.
- **Making library code work regardless of compiler flags.** The user must set
  the right flags / target for the library they are using; the compiler does not
  try to make a library work under an arbitrary flag set. (In-source pragmas to
  express this may come in the future, but are not a goal now.)
- **Shipping Tier C (vendored third-party) libraries through this mechanism.** It
  bundles only own-authored Tier A/B content; redistributing vendored third-party
  source (e.g. OSCAT) is a *separate distribution mechanism* with its own
  licensing, out of scope here (see *Licensing, provenance, and clean-room
  authoring*).
- Collision resolution between simultaneously-active libraries — deferred (see
  *Future Goals*).
- Bindings, non-ST implementations (VM intrinsics), and the declare-only state —
  deferred with the bindings mechanism (see *Future Goals*). The first increment
  ships only fully-defined ST.

## Current State

Three existing facts make this cheap to build:

1. **Multi-library merge already works.** `analyze(sources: &[&Library])`
   concatenates every parsed `Library` into one via `library.extend()` in
   `analyzer/src/stages.rs::resolve_types()`. Injecting "a library of
   declarations the user did not write" is not new machinery — it is one more
   `&Library` in the slice.
2. **The stdlib is already seeded, and already conditional.**
   `resolve_types()` builds the type/function/symbol environments from Rust
   builder tables (`get_all_stdlib_functions`, `get_all_stdlib_function_blocks`),
   and already seeds conditionally — e.g. `SIZEOF` only when `allow_sizeof`.
   "Conditionally add named things to the environment" is an established pattern.
   (Note: `__`-prefixed compiler intrinsics like `__SYSTEM_UP_TIME` are seeded
   the same way but are *not* candidates for libraries — see *Non-Goals*.)
3. **Project discovery already reads vendor project files.**
   `sources/src/discovery/` detects TwinCAT `.plcproj` and reads
   `<Compile Include>` file references. Reading the project's *library*
   references is an extension of an existing capability, not a new subsystem.

What is missing today: there are no named constants at all (`PI` resolves as an
undeclared variable → `VariableUndefined`), and there is no notion of a named,
provenance-tagged bundle of declarations that can be turned on as a unit.

## Design

### Compatibility libraries

A compatibility library is a bundled, named collection of declarations with
provenance metadata. IronPLC ships many; all are **dormant by default**. A
library provides some mix of:

- **Constants** — e.g. `PI`, `e`, expressed as ordinary
  `VAR_GLOBAL CONSTANT PI : LREAL := 3.14159265358979;`. This folds at compile
  time and satisfies [ADR-0024](../adrs/0024-function-local-reinit-via-init-template.md)
  (initializers must constant-fold) with no new keyword and no codegen change.
- **Function signatures** — e.g. TwinCAT `FLOOR` declared with an `LREAL`
  parameter (matching Beckhoff, which differs from the base IEC signature).
- **Function block types** — e.g. vendor-specific FBs.

Testable behavior:

- **REQ-CL-analyzer-001** A compatibility library is dormant by default: its
  declarations are in scope only when the library is activated.
- **REQ-CL-analyzer-003** When a math compatibility library is active, `PI`
  resolves as a constant and folds at compile time, so it is usable in a `VAR`
  initializer (e.g. `d2r : LREAL := PI/180.0;`).
- **REQ-CL-analyzer-004** A user declaration shadows an activated library
  declaration of the same name.

### Activation channels

A dormant library is activated **only** out of band, through one of:

1. **Project file reference.** When IronPLC compiles a discovered vendor project
   (e.g. a `.plcproj`), it reads the project's declared library references and
   activates the matching bundled libraries — pulling them into the composition
   (the merged compilation unit). The statement of "which libraries this project
   uses" is *the vendor's own*, read from the project; the user writes nothing
   new.
2. **Explicit command-line activation.** `--library <name>[@<version>]`
   (repeatable) for source that has no project context.

In the playground, the same libraries are served as plain-text files and loaded
as additional sources, so a library can be activated in the browser too.

Neither channel modifies POU source, preserving the portability invariant.

- **REQ-CL-sources-001** The compiler reads the set of referenced libraries from
  a discovered `.plcproj` project file's declared library references and
  activates the matching bundled libraries.
- **REQ-CL-sources-006** An explicit activation request (e.g. the
  `--library <name>` CLI option) activates the named bundled library for source
  that has no project context.
- **REQ-CL-playground-001** The playground activates a library by loading it from
  the plain-text library files served alongside the app.

**Never sniff, never guess.** The active library set comes *only* from an
explicit project-file reference or explicit CLI/playground activation. Guessing
wrong would silently change behavior, which the portability promise forbids.

- **REQ-CL-sources-005** The active library set derives only from explicit
  activation (a project-file reference or explicit CLI/playground activation);
  the compiler never infers a library from POU source content.

**"Dormant by default" and "just works by default" are not in tension.**
Libraries are off for *bare, context-free source*, but on *automatically when a
project declares them*. The paved path is "compile the actual project," and
activation is **inferred from the project file** — never asked of the user. So
the default outcome of compiling a real TwinCAT project is that its libraries
light up.

### Flat names

Symbols inject **flat**, under their exact vendor names — this is required by the
portability invariant.

- **REQ-CL-analyzer-002** An activated library's symbols resolve under their
  exact vendor names (flat), with no compiler-injected namespace qualifier.

Two rules govern qualifiers:

- IronPLC **accepts** a qualifier the source already wrote (e.g.
  `Tc2_Standard.TON`) and **never adds** one the source did not. Whether such a
  qualified access path is *supported by the parser today* is irrelevant to the
  design: if the access path is valid in a source environment, IronPLC must be
  able to reproduce it. Whether qualification is a *requirement* for some
  libraries (rather than merely optional) is deferred — see *Future Goals*.
- Resolving genuine collisions between two simultaneously-active libraries (or
  between a library and the base stdlib) is **deferred** — see *Future Goals*.
  The initial increment assumes an activated library provides names that do not
  collide with each other or with the base set.

### What a library is on disk

The on-disk package layout (a directory named for the library, a `library.toml`
manifest, and per-version subdirectories of `.st` declarations) and the manifest
schema are specified in
[Compatibility Library Format](compatibility-library-format.md). Libraries are
installed on disk and read at runtime, not embedded in the compiler. The
behavioral requirements the loader enforces are:

- **REQ-CL-sources-002** A library's manifest declares its identity — `name`,
  `vendor`, `default_version` — and the loader rejects a manifest missing any
  required field.
- **REQ-CL-sources-007** The manifest records the **public references** the
  library was authored from (a non-empty `references` list). This is a factual
  record of inputs, not a legal classification; the reviewer and the clean-room
  spec are recorded by git history (see the
  [authoring policy](../steering/compatibility-library-authoring.md)).

A library is activated explicitly, never by dialect (see *Dialect and vendor are
different concepts*). Defining libraries as data (manifest + ST) rather than Rust
code is what lets a library be added without a compiler change and lets the
playground serve them as plain-text files.

### Licensing, provenance, and clean-room authoring

Distributing these libraries — including AI-generated shims — must not create
copyright or license problems. Because much of the code is AI-generated, we
cannot prove a model "never saw" an original in training, so clean provenance is
demonstrated by a **factual, durable record** rather than an unprovable claim:

- The manifest lists the **public references** the library was authored from
  (**REQ-CL-sources-007**) — inputs, not a legal judgment.
- The clean-room spec authored from those references is committed and **merged as
  its own non-squashed git entry**, and the reviewer is recorded by git history.

The risk tiers (Tier A facts/math-dictated; Tier B clean-room interface shim) and
the allowed/forbidden inputs are a **human policy** in the
[authoring policy](../steering/compatibility-library-authoring.md), not fields in
the manifest. **Tier C — vendored third-party source** (e.g. OSCAT) is *not*
distributed through this mechanism; it is a separate distribution mechanism (see
*Non-Goals*). The machine-checkable part — the manifest is well-formed and
records its references — is a conformance test; whether the references are honest
and complete is confirmed at review. The test checks the record's *shape*; the
reviewer checks its *truth*.

### Bodies (first increment) and the future bindings mechanism

The first increment ships only fully-defined ST — for `Tc2_Math`, `PI` as a
`VAR_GLOBAL CONSTANT`. There is no partial/declare-only state and no non-ST
implementation.

A later increment adds **bindings** (a per-version manifest table; see
[Compatibility Library Format §Future](compatibility-library-format.md)) so a POU
can map to a **VM intrinsic** — for native functions like `Tc2_Math` numerics,
where we *desire* the same numeric behavior as the vendor (feasibility TBD) — or
be **declare-only**. Per *Safety first*, once that exists a *call* to a
declare-only POU is a **compile error** (a dedicated problem code, not a runtime
trap), so a large library's surface can land ahead of its bodies without letting
an unimplemented function reach execution.

### Referenced-but-unshipped libraries

- **REQ-CL-sources-004** If a project references a library IronPLC does not
  bundle, the compiler emits a diagnostic that names the missing library (rather
  than failing silently), so the resulting `undefined symbol` errors are
  explained.

### Reference matching

- **REQ-CL-sources-003** Resolution from a project's library reference to a
  bundled library is by strict, case-sensitive **name** match — better too strict
  than to silently bind the wrong library.

Version policy: a `*` version in the reference (the common `PlaceholderReference`
case) matches the single bundled version. A pinned version that differs from the
bundled one still resolves but emits a warning, since IronPLC ships one version
per library — mismatched behavior, if any, is surfaced rather than silently
accepted or hard-failed.

### Round-trip fidelity

- **REQ-CL-plc2plc-001** `plc2plc` emits the user's source unchanged; declarations
  injected by an activated library are never rendered as user source.

### Dialect and vendor are different concepts

A **dialect** governs which *syntax and language rules* the compiler accepts
(permissive-parse / reject-by-policy,
[ADR-0040](../adrs/0040-dialect-violations-diagnosed-in-policy-phase.md)). A
**library** governs which *declarations* are in scope. They are independent axes,
selected independently: a dialect **never** activates a library, and a library
never changes the accepted syntax.

- **REQ-CL-analyzer-006** Selecting a dialect does not activate any compatibility
  library; library activation comes only from the channels in *Activation
  channels*.

The reverse-direction portability guarantee is a property of the **dialect**
alone: if code compiles under a vendor's dialect (its language subset), it uses
nothing that vendor's language would reject. Libraries ride alongside but do not
provide that guarantee.

## Worked Examples

### `PI` in a TwinCAT project

`d2r : LREAL := PI/180.0;` in a `.plcproj`-rooted project. The project references
`Tc2_Math` (the TwinCAT library whose global constants include `PI`); IronPLC
activates it, injecting `VAR_GLOBAL CONSTANT PI : LREAL := 3.14159265358979;`
into the composition. `PI`
resolves as a constant symbol, folds at compile time, and the initializer
compiles. No flag, no keyword, no source edit. `plc2plc` renders the user's
`d2r` declaration unchanged; the injected `PI` is not user source and is not
emitted.

### `FLOOR` with the TwinCAT signature

TwinCAT's `Tc2_Math.FLOOR` takes an `LREAL` parameter, differing from the base
IEC `FLOOR`. Serving the TwinCAT signature under its exact name is the goal.
Because this is a case where a library name collides with the base stdlib, the
*resolution* of that collision depends on the deferred collision/precedence work
(see *Future Goals*); it is called out here as a concrete motivating case for
that future work rather than something the first increment resolves.

### OSCAT (Tier C — via a separate mechanism)

OSCAT is a **Tier C** library: its source is license-encumbered, so it is **not**
bundled through this mechanism (see *Licensing, provenance, and clean-room
authoring*). It is retained here only to illustrate the body model — an
OSCAT-style library's functions would carry real ST bodies, with any
not-yet-supported POU marked `declare-only` so unrelated code type-checks while a
*call* to it is a compile error. Delivering OSCAT itself is a separate
distribution mechanism, out of scope here.

## Future Goals

- **Collision resolution / precedence.** When two activated libraries — or a
  library and the base stdlib — define the same flat name, decide resolution
  (e.g. reference-order precedence with source-written qualifiers pinning, and a
  diagnostic on genuine ambiguity), faithfully reproducing the host's behavior.
  The `FLOOR`-override case above depends on this.
- **Cross-library / cross-vendor mixing** as a supported configuration.
- **Bindings** — a per-version manifest table (see
  [Compatibility Library Format §Future](compatibility-library-format.md)) mapping
  a POU to a **VM intrinsic** or **declare-only**, plus the fail-if-unimplemented
  rule (a call to a declare-only POU is a compile error). Omitted from the first
  increment, which ships only fully-defined ST.
- **Accept source-written namespace qualifiers** (`Tc2_Standard.TON`), mapping the
  qualifier to its library. Needed only if some library requires the qualified
  form; no known Tier A/B library does.
- **In-source pragmas** to express flag/library intent, if ever needed.

## Alternatives Considered

- **Per-symbol feature flag (`--allow-math-constants`).** The earlier PI
  proposal. Rejected: does not scale (a flag per constant/function), carries no
  provenance, and cannot be restricted by target.
- **Name prefixing / forced qualification (`Tc2_Math.FLOOR`).** Rejected: breaks
  the portability invariant — the representation diverges from vendor source and
  `plc2plc` would emit qualifiers the source never had.
- **Required in-source directive (`{library 'Tc2_Math'}`).** Rejected as a
  *required* channel: vendor POU files do not carry it, so requiring it is a
  source edit. May survive only as an optional IronPLC-native convenience that is
  never required for vendor code.
- **Capability groups in Rust (generalize the `SIZEOF` seeding).** Viable for
  constants and native intrinsics, but libraries would be *code* not *data*, and
  it does not scale to source libraries like OSCAT (hundreds of ST bodies would
  have to be hand-ported to Rust). Retained only as the implementation substrate
  for intrinsic-backed native functions.

## Open Questions

Prior open questions are now decided:

- **Manifest format** and installation — specified in
  [Compatibility Library Format](compatibility-library-format.md). The
  **fail-if-unimplemented rule** — *Bodies and the fail-if-unimplemented rule*.
- **Provenance** — the manifest records the public references used; the reviewer
  and clean-room spec are recorded by git history, not manifest fields
  (*Licensing, provenance, and clean-room authoring*).
- **Tier C** (vendored third-party, e.g. OSCAT) is **not** shipped through this
  mechanism — it is a separate distribution mechanism (see *Non-Goals*).
- **Dialects never auto-activate libraries** — dialect and vendor are independent
  axes (**REQ-CL-analyzer-006**).
- **Qualified access** — the first increment injects **flat names only**;
  accepting a source-written `Namespace.Symbol` qualifier is deferred until a
  library requires it (see *Future Goals*). None known do.
- **Version matching** and the **`.plcproj` shape** — *Reference matching* and the
  *Appendix*.

No open questions remain for the first increment.

## Implementation

The on-disk package format and installation/discovery are specified separately in
[Compatibility Library Format](compatibility-library-format.md) (`REQ-LF-*`). The
[implementation plan](../plans/2026-08-04-compatibility-libraries.md) delivers
`.plcproj` library-list reading and the `Tc2_Math` library (defining `PI`) in its
early phases and wires each `REQ-CL-*` / `REQ-LF-*` marker to a `#[spec_test]`.

## Appendix: `.plcproj` library-reference shapes

Grounded in real TwinCAT projects (see *References*). A `.plcproj` is an MSBuild
project (`xmlns="http://schemas.microsoft.com/developer/msbuild/2003"` — the same
namespace the existing discovery already reads `<Compile Include>` from). Library
references live in `<ItemGroup>` as one of two element types.

**`PlaceholderReference`** — the common, version-flexible form. `Include` is the
placeholder name; `DefaultResolution` is `"<Name>, <Version> (<Vendor>)"` where
the version is usually the `*` wildcard; `Namespace` is the qualifier the source
may write (`Tc2_Math.FLOOR`):

```xml
<PlaceholderReference Include="Tc2_Math">
  <DefaultResolution>Tc2_Math, * (Beckhoff Automation GmbH)</DefaultResolution>
  <Namespace>Tc2_Math</Namespace>
</PlaceholderReference>
```

**`LibraryReference`** — a concrete, pinned reference. `Include` is
`"<Name>,<Version>,<Vendor>"`:

```xml
<LibraryReference Include="Tc2_Utilities,3.3.7.0,Beckhoff Automation GmbH">
  <Namespace>Tc2_Utilities</Namespace>
</LibraryReference>
```

Implications for this design:

- The **`<Namespace>`** element *is* the qualifier→library map, supplied by the
  project. That grounds Open Question 1: for project-driven activation we do not
  have to infer the namespace — the project states it.
- A `<SystemLibrary>true</SystemLibrary>` child marks CODESYS/visualization
  system libraries (e.g. `Recipe Management`, `VisuElems`). These are not
  vendor-authored ST we would bundle; the first increment skips them.
- The version is commonly `*`, so **REQ-CL-sources-003** name matching is the
  load-bearing rule; version matching is the residual open question above.

## References

- [Real `.plcproj` referencing `Tc2_Math`](https://github.com/evanmj/StructuredTextVideoSeries/blob/master/EP2-/Untitled1/Untitled1.plcproj)
  and [one with a pinned `LibraryReference`](https://github.com/hiroMTB/n5TC/blob/master/sample/PTP/PTP/TwinCAT_NC_Sample_PTP_Move/TwinCAT_NC_Sample_PTP_Move.plcproj)
  — grounding for the appendix.
- [Beckhoff InfoSys: PLC libraries and placeholders](https://infosys.beckhoff.com/content/1033/tc3_plc_intro/41891384434359666059.html)
- [ADR-0024: Function-local re-init via init template](../adrs/0024-function-local-reinit-via-init-template.md)
  — initializers must constant-fold; `PI`-as-constant complies.
- [ADR-0036: No IronPLC dialect](../adrs/0036-no-ironplc-dialect.md)
- [ADR-0038: No restrictions on flag combinations](../adrs/0038-no-restrictions-on-flag-combinations.md)
  — bounds the portability promise.
- [ADR-0040: Dialect violations diagnosed in policy phase](../adrs/0040-dialect-violations-diagnosed-in-policy-phase.md)
  — the reverse-direction guarantee.
- [IEC 61131-3 Compliance steering](../steering/iec-61131-3-compliance.md)
  — permissive parsing, configurable validation; vendor extensions are additive.
- [Syntax Support Guide](../steering/syntax-support-guide.md)
  — `--allow-*` flag and dialect machinery.
