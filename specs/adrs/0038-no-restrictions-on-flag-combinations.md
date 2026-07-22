# ADR-0038: Feature Flags Are Freely Composable; Dialects Express Preferred Combinations

status: proposed
date: 2026-07-21

## Context and Problem Statement

IronPLC gates non-standard syntax behind `--allow-*` feature flags on
[`CompilerOptions`], and bundles the flags that correspond to a real target into
named [`Dialect`] presets (`iec61131-3-ed2`, `iec61131-3-ed3`, `rusty`,
`codesys`). Flags are overlaid on a dialect preset independently (`options.field
|= cli.field`).

Adding `--allow-reference-to` (Beckhoff/CODESYS `REFERENCE TO`) alongside the
existing `--allow-ref-to` (IEC `REF_TO`) surfaced a question: these are two
different reference-usage models (`REF()`/`^` explicit dereference vs
`REF=`/implicit dereference), and no single real dialect enables both. Should
the compiler **reject** enabling both at once with a validation error?

Generalized: **does the compiler validate `--allow-*` flag *combinations* and
error on combinations that correspond to no real dialect?**

## Decision Drivers

* **Consistency with the options model** — `CompilerOptions` is a flat set of
  independent booleans overlaid on a dialect preset. Combination validation
  would introduce cross-field coupling into an otherwise orthogonal set.
* **Consistency with ADR-0036** — flags exist to *compose and describe* real
  dialects; the endorsed unit of "a real target" is the dialect. ADR-0036's
  enforcement mechanism is strict defaults + actionable diagnostics + exposing
  real dialects — never combination-rejection.
* **Maintenance cost** — a combination-validation matrix grows quadratically as
  flags are added and must be revisited on every new flag.
* **Correctness does not require it** — where two features might seem to
  conflict, the design should stay well-defined under coexistence rather than
  rely on forbidding the combination. For `REF_TO`/`REFERENCE TO`, the
  `RefSyntax` tag recorded on each declaration makes dereference behavior
  per-declaration, so both can appear in one program unambiguously.

## Considered Options

* **A — No combination restrictions.** Any set of `--allow-*` flags is permitted.
  Preferred/real combinations are expressed exclusively through `Dialect`
  presets and documentation. Features must remain well-defined under coexistence.
* **B — Validate and reject "incompatible" combinations.** The compiler errors
  when a disallowed pair is set together (e.g. `--allow-ref-to` +
  `--allow-reference-to`).

## Decision Outcome

Chosen option: **A — No combination restrictions.**

The compiler treats `--allow-*` flags as independent, freely-composable toggles
and does not reject any combination. "Preferred" combinations — the ones that
correspond to a real toolchain — are expressed **only** through `Dialect`
presets: `REFERENCE TO` is bundled into the CODESYS/Beckhoff-facing dialect,
`REF_TO` into Edition 3 / `rusty` / `codesys`, and no dialect bundles both.
A user who sets both flags by hand assembles a configuration that matches no
real target; that is their choice, exactly as with any other non-dialect flag
combination, and the compiler neither blesses nor blocks it.

A corollary binds feature design: when two flag-gated features could interact,
the design must stay unambiguous when both are enabled, rather than delegating
disambiguation to a mutual-exclusivity error. The `REFERENCE TO` design
satisfies this by tagging each reference declaration with its surface syntax
(`RefSyntax::RefTo` vs `RefSyntax::ReferenceTo`); the implicit-dereference
transform keys on `RefSyntax::ReferenceTo`, so `REF_TO` variables are never
implicitly dereferenced even when both flags are active.

### Consequences

* Good, because the options model stays a flat, orthogonal set of booleans with
  no combination matrix to maintain as flags proliferate.
* Good, because it is consistent with ADR-0036: the dialect is the unit of a
  real target; flags are the composition primitive; preference is expressed by
  which dialect bundles which flags.
* Good, because users exploring the language can compose flags freely without
  fighting the compiler.
* Neutral, because a nonsensical combination is not caught by the compiler — the
  guardrail is "select a dialect," not "the compiler forbids it." The burden of
  staying well-defined under coexistence shifts onto each feature's design.
* Bad, because the compiler emits no signal that an unusual pairing (e.g.
  `REF_TO` + `REFERENCE TO`) corresponds to no real dialect; mitigated by the
  dialect presets and documentation making the intended combinations discoverable.

## More Information

### Relationship to ADR-0036 (No IronPLC Dialect)

ADR-0036 establishes that meaningful configurations correspond to real IEC
editions or real vendor dialects, that flags exist to compose those dialects,
and that the default stays strict. This ADR settles the narrower, mechanism-level
question ADR-0036 left implicit: when a hand-assembled flag combination
corresponds to no real dialect, the compiler **permits** it rather than
**rejecting** it. Preference lives in the dialect presets, not in validation
logic. The two ADRs are complementary — ADR-0036 defines *what the real targets
are*; this ADR defines *how preference among combinations is expressed* (dialects,
not errors).

### Relationship to ADR-0012 (Accept Vendor Dialect Files As-Is)

ADR-0012 holds that a file belongs to exactly one dialect and that IronPLC should
not endorse mixing vendor extensions into a dialect that exists nowhere. That is a
statement about what IronPLC *ships and endorses*, expressed through the dialect
presets — not a mandate to add compiler errors that police user-supplied flag
combinations. This ADR is consistent: mixing is discouraged by *not bundling* the
extensions into a shared dialect, not by rejecting the combination at compile time.
