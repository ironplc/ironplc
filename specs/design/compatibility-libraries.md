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

This replaces the earlier, stalled approach of adding `PI` behind a
`--allow-math-constants` flag (a per-symbol flag with no provenance and no way
to restrict it by dialect).

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

- Calling a POU that is only declared (no runnable body yet — see *Bodies*) is a
  **compile error**, never a runtime trap.
- An unsupported library configuration is diagnosed and rejected, not compiled
  into something with undefined behavior.

The exact mechanism for detecting and rejecting unsupported configurations is an
open question (see *Open Questions*); the requirement is that the failure mode is
"does not compile," never "compiles and misbehaves."

## Goals

- Support vendor constants (`PI`, `e`), functions (`Tc2_Math` numerics, OSCAT
  functions), and function blocks under their exact vendor names.
- Scale to arbitrarily many libraries with **zero** new per-symbol flags.
- Make availability restrictable by target: a non-standard name like `PI` is
  only in scope when a library that provides it is active.
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
- Collision resolution between simultaneously-active libraries — deferred (see
  *Future Goals*).
- Full runtime implementation of every vendor function in the first increment.
  Some libraries begin as declare-only (see *Bodies*), subject to *Safety first*.

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

**Never sniff, never guess.** The active library set comes *only* from an
explicit project-file reference or explicit CLI/playground activation. The
compiler never infers a library from source content. Guessing wrong would
silently change behavior, which the portability promise forbids.

**"Dormant by default" and "just works by default" are not in tension.**
Libraries are off for *bare, context-free source*, but on *automatically when a
project declares them*. The paved path is "compile the actual project," and
activation is **inferred from the project file** — never asked of the user. So
the default outcome of compiling a real TwinCAT project is that its libraries
light up.

### Flat names

Symbols inject **flat**, under their exact vendor names — this is required by the
portability invariant. Two rules govern qualifiers:

- IronPLC **accepts** a qualifier the source already wrote (e.g.
  `Tc2_Standard.TON`) and **never adds** one the source did not. Whether such a
  qualified access path is *supported by the parser today* is irrelevant to the
  design: if the access path is valid in a source environment, IronPLC must be
  able to reproduce it. The open question is only whether qualification is a
  *requirement* for some libraries or merely optional (see *Open Questions*).
- Resolving genuine collisions between two simultaneously-active libraries (or
  between a library and the base stdlib) is **deferred** — see *Future Goals*.
  The initial increment assumes an activated library provides names that do not
  collide with each other or with the base set.

### What a library is on disk

Each bundled library is a **manifest plus declarations**:

- **Manifest** — name, vendor, version, and target/dialect it belongs to. This is
  the provenance that makes a symbol restrictable by target.
- **Declarations** — real IEC 61131-3 Structured Text for anything with a
  runnable body (OSCAT functions, `PI` as a constant), and `extern`/intrinsic
  markers for native functions whose bodies IronPLC provides another way (see
  *Bodies*).

Defining libraries as data (manifest + ST) rather than Rust code is what lets a
library be added without a compiler change, lets third parties contribute
libraries, and lets the playground serve them as plain-text files.

### Bodies: runnable vs. declare-only

A POU in a library binds its implementation one of three ways:

- **ST body** — real Structured Text, compiled and run like user code. OSCAT is
  open source and semantically identical across environments, so it rides its
  real bodies for free.
- **VM intrinsic** — a native implementation in the VM. TwinCAT `Tc2_Math`
  numerics map here, reusing the trig/numeric intrinsics being built out
  separately. We *desire* the same numeric behavior as the vendor (rounding,
  edge cases); whether that is fully achievable is to be determined.
- **Declare-only** — the declaration exists so the analyzer can resolve a
  reference, but there is no runnable body yet. Per *Safety first*, **calling a
  declare-only POU is a compile error**, not a runtime trap. This lets a large
  library's declarations land (so unrelated code type-checks) ahead of full
  runtime support, without ever letting an unimplemented function slip through to
  execution.

### Referenced-but-unshipped libraries

If a project references a library IronPLC does not bundle, emit a diagnostic
that names the missing library, so the resulting `undefined symbol` errors are
explained rather than mysterious. Do not fail silently.

### Composition with dialects (the reverse direction)

IronPLC → vendor portability holds only if "compiles under the vendor target"
also means "uses nothing the vendor would reject." That is the existing
permissive-parse / reject-by-policy machinery
([ADR-0040](../adrs/0040-dialect-violations-diagnosed-in-policy-phase.md)).
Compatibility libraries **compose** with it: activating a vendor target makes
that vendor's libraries available *and* constrains the accepted language, so
"green under the vendor target" is the portability certificate for the reverse
direction.

## Worked Examples

### `PI` in a TwinCAT project

`d2r : LREAL := PI/180.0;` in a `.plcproj`-rooted project. The project references
the library that defines `PI`; IronPLC activates it, injecting
`VAR_GLOBAL CONSTANT PI : LREAL := 3.14159265358979;` into the composition. `PI`
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

### OSCAT

An OSCAT-based project references OSCAT; IronPLC activates the bundled OSCAT
library, whose functions and function blocks carry real ST bodies and compile
and run like user code. Where a body is not yet supported at runtime, the POU is
declare-only, so unrelated code type-checks while any *call* to an unimplemented
function is a compile error (per *Safety first*).

## Future Goals

- **Collision resolution / precedence.** When two activated libraries — or a
  library and the base stdlib — define the same flat name, decide resolution
  (e.g. reference-order precedence with source-written qualifiers pinning, and a
  diagnostic on genuine ambiguity), faithfully reproducing the host's behavior.
  The `FLOOR`-override case above depends on this.
- **Cross-library / cross-vendor mixing** as a supported configuration.
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

1. **Qualified access requirement.** If a qualified access path (e.g.
   `Tc2_Standard.TON`) is valid in a source environment, IronPLC must be able to
   reproduce it. The open question is whether such qualification is *required* by
   some libraries, or always optional — which affects how much of the qualifier
   path must land before those libraries are usable.
2. **Library reference identity and matching.** A `.plcproj` names a library with
   vendor, version, and author (e.g. `Tc2_Standard, * (Beckhoff Automation
   GmbH)`). What is the resolution from that reference to a bundled library?
   Matching should be **strict and case-sensitive** — better too strict than to
   silently bind the wrong library.
3. **Unsupported-configuration detection.** By what mechanism does the compiler
   recognize an unsupported library/flag configuration in order to reject it
   (per *Safety first*)?
4. **Manifest format.** Concrete on-disk shape of the manifest and how
   `extern`/intrinsic bodies are marked in bundled declarations.

## References

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
</content>
