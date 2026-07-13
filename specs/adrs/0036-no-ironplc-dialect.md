# ADR-0036: IronPLC Does Not Define Its Own Dialect

status: proposed
date: 2026-07-12

## Context and Problem Statement

IronPLC accepts Structured Text under several configurations: the IEC 61131-3
editions (`iec61131-3-ed2`, `iec61131-3-ed3`) and vendor-compatible dialects
(`rusty`, `codesys`). These are expressed as [`Dialect`] presets plus a set of
per-feature `--allow-*` flags on [`CompilerOptions`]. Every one of these
configurations corresponds to something real — a published IEC edition, or the
language a real vendor toolchain accepts.

There is recurring pressure to make IronPLC's *default* behavior more lenient in
order to smooth over common user errors. A concrete example: C-style comments
(`//`, `/* */`) are one of the most common errors playground users hit, and it
is tempting to simply enable them by default so the error disappears.

Doing that would create a problem. A default that accepts C-style comments (or
any other vendor extension) is no longer strict IEC 61131-3, and it matches no
single vendor either. It is a new, IronPLC-specific set of accepted syntax — a
*de-facto IronPLC dialect*. Code that relies on it would be valid in IronPLC and
invalid everywhere else, which is exactly the outcome IronPLC exists to avoid.

The question this ADR settles: **does IronPLC define its own dialect?**

## Decision Drivers

* **Portability** — code that IronPLC accepts should be loadable in some real
  toolchain (a standards-conformant tool, or the vendor whose dialect it targets)
* **Honesty of configuration** — a selected dialect should mean exactly what its
  name says, with no hidden leniency layered on top
* **Discoverability of fixes** — when a construct is rejected, the user should be
  pointed at a real, selectable resolution (fix the code, or pick a dialect that
  supports it) rather than silently accommodated
* **Consistency with ADR-0012** — that ADR already forbids mixing vendor
  extensions within a file precisely because it "would create a dialect that
  doesn't exist anywhere else"

## Considered Options

* **Define no IronPLC dialect** — every accepted configuration maps to a real
  IEC edition or a real vendor dialect; defaults stay strict; rejected constructs
  are resolved by fixing the code or selecting an existing dialect
* **Default-enable common extensions** — turn on frequently-used vendor
  extensions (e.g. C-style comments) in the default configuration to reduce
  friction

## Decision Outcome

Chosen option: **Define no IronPLC dialect.**

IronPLC does not invent a dialect. The complete set of configurations is:

* **IEC 61131-3 Edition 2** (`iec61131-3-ed2`) — the strict default.
* **IEC 61131-3 Edition 3** (`iec61131-3-ed3`).
* **Vendor-compatible dialects** (`rusty`, `codesys`) — each matching what that
  real toolchain accepts.

The `--allow-*` feature flags exist to *compose* and *describe* these real
dialects (a dialect preset is exactly a named bundle of flags), not to let users
assemble a novel accepted-syntax set that corresponds to no real target. The
default is strict IEC 61131-3 Edition 2, and no vendor extension is enabled by
default.

When IronPLC rejects a non-standard construct, the resolution is always to
either (a) change the code to standard IEC 61131-3, or (b) select an existing
dialect that supports the construct. IronPLC never adds a third path of
"IronPLC accepts this even though nothing else does."

### How this shaped the C-style comment handling

The C-style comment case is the motivating example and follows directly from
this decision:

* The strict default keeps rejecting `//` and `/* */` (problem code P0004) —
  they are not IEC 61131-3.
* The rejection is made *actionable* rather than *silently removed*: the P0004
  diagnostic now carries a help note telling the user to convert the comment to
  `(* *)` syntax **or** select a dialect that supports C-style comments.
* The playground exposes all real dialects so that "select a dialect that
  supports it" is a concrete, one-click action.

Notably, the fix did **not** default-enable C-style comments, because that would
have created the very IronPLC dialect this ADR rules out.

### Consequences

* Good, because any file IronPLC accepts is loadable in a real toolchain — the
  portability guarantee holds by construction
* Good, because a dialect name means exactly what it says; there is no hidden
  default leniency to reason about
* Good, because rejected constructs guide users toward real, selectable
  resolutions, which is more educational than silent acceptance
* Good, because it generalizes the ADR-0012 principle ("don't create a dialect
  that exists nowhere else") from within-file mixing to defaults and every other
  configuration surface
* Neutral, because common conveniences (like C-style comments) are still one
  dialect selection away — they are gated, not forbidden
* Bad, because the strict default surfaces more first-encounter errors than a
  lenient default would; this is mitigated by making each such error actionable

## More Information

### Relationship to ADR-0012 (Accept Vendor Dialect Files As-Is)

ADR-0012 established that a single file belongs to exactly one dialect and that
mixing vendor extensions "would mean IronPLC is creating a new dialect that
doesn't exist anywhere else — the opposite of accept-as-is." This ADR states the
general principle behind that rule: IronPLC never creates a dialect of its own,
whether by mixing extensions within a file or by enabling extensions in a
default configuration.

### Relationship to ADR-0022 (Edition 3 Compiler Flag)

ADR-0022 gates Edition 3 features behind an explicit opt-in so that Edition 2
code is validated against Edition 2. That opt-in model is consistent with this
ADR: editions and dialects are explicit selections, and the default commits to a
single real target (strict Edition 2) rather than a permissive superset.
