# Design: Compatibility Library Format

## Overview

This document specifies the **on-disk format** of a compatibility library and how
the compiler **discovers and installs** libraries. It is the format counterpart
to the behavioral design in
[Compatibility Libraries](compatibility-libraries.md): that document says *what a
library does*; this one says *what a library is on disk and how it gets to the
compiler*.

These decisions are made up front (not deferred) so the format is stable before
any library ships, the same way the
[bytecode container format](bytecode-container-format.md) is fixed before the VM
loads a file.

## Design Goals

1. **Self-describing package** — a library is a directory that carries everything
   the compiler needs to load it; no external index or registry.
2. **One format, two consumers** — the *same* on-disk files are embedded into the
   CLI compiler and served as plain text to the browser playground.
3. **Portable** — the package layout is a plain filesystem directory, so a future
   user/third-party library installs by placing its directory on a search path,
   using the identical format bundled libraries use.
4. **Grounded, not interpreted** — the manifest records the *public references*
   used to author the library (facts), never a legal classification.

## Package Layout

**REQ-LF-sources-001** A compatibility library is a directory whose name equals
the manifest `name`, containing exactly one `library.toml` and one or more `.st`
declaration files:

```
Tc2_Math/
├── library.toml
└── Tc2_Math.st        # one or more .st files (declarations, and any ST bodies)
```

The directory name is the library's identity: it is what a project reference or
`--library` names, matched exactly and case-sensitively (see
[Compatibility Libraries §Reference matching](compatibility-libraries.md)).

## Manifest

**REQ-LF-sources-002** The manifest is `library.toml` with these fields:

| Requirement | Field | Type | Required | Meaning |
|-------------|-------|------|----------|---------|
| **REQ-LF-sources-002** | `name` | string | yes | Library identity; equals the directory name and the vendor library name (e.g. `Tc2_Math`). |
| | `vendor` | string | yes | Originating vendor (e.g. `Beckhoff Automation GmbH`), or `IronPLC` for own libraries. |
| | `version` | string | yes | Semantic version. One version per library ships in the first increment. |
| **REQ-LF-sources-004** | `references` | array of string | yes | The **public references** the library was authored from — documentation URLs/citations. Facts, not a legal judgment. Non-empty. |
| | `[bindings]` | table | no | Per-POU implementation binding (below). Absent ⇒ every POU is `st`. |

The manifest deliberately carries **no** `license`, `derivation`, `reviewer`, or
`target`/`dialect` field:

- License: bundled libraries are own-authored and ship under the repository's MIT
  license; there is no per-library license (vendored third-party content is a
  *separate distribution mechanism* — see the behavioral design's *Non-Goals*).
- Derivation/tier: the risk tier is a human policy judgment (see the
  [authoring policy](../steering/compatibility-library-authoring.md)), not a field
  an author encodes.
- Reviewer: recorded by git history.
- Dialect/target: a library is activated explicitly, never by dialect.

Example:

```toml
name = "Tc2_Math"
vendor = "Beckhoff Automation GmbH"
version = "1.0.0"
references = [
  "https://infosys.beckhoff.com/content/1033/tcplclib_tc2_math/ — Global constants",
]

[bindings]
# PI is a constant — no binding entry needed.
# FLOOR   = "intrinsic:floor_lreal"   # native VM intrinsic
# SOME_FB = "declare-only"            # signature only; a call is a compile error
```

## Declarations and bindings

**REQ-LF-sources-003** Declarations are IEC 61131-3 Structured Text in the `.st`
files. Each POU's implementation binding is one of:

- **`st`** (default) — the POU has a Structured Text body in a `.st` file.
- **`intrinsic:<name>`** — the POU maps to the named VM intrinsic.
- **`declare-only`** — the POU has a signature but no body.

The binding grammar is defined here; the loader that *validates* it (an `st` POU
has a body, an `intrinsic:<name>` names an implemented intrinsic, a `declare-only`
POU is signature-only) and the compile-time failure for calling a `declare-only`
POU are specified in [Compatibility Libraries](compatibility-libraries.md).

## Installation and discovery

**REQ-LF-sources-005** Bundled compatibility libraries live under
`compiler/sources/resources/compat-libraries/` and are **embedded into the
compiler at build time**. There is no separate install step for bundled
libraries; they are always available to the CLI. The playground is served the
*same* files as plain text and loads them as additional sources.

**REQ-LF-sources-006** A library is resolved by exact, case-sensitive `name`. In
the first increment the resolver searches only the embedded bundled set. The
package format is a plain directory, so a future user/third-party library
installs by placing its package directory on a libraries search path
(`IRONPLC_LIBRARY_PATH`); that path is **defined but not enabled** in the first
increment, and when enabled resolves *after* the bundled set (bundled wins).

This keeps one format across bundled and installed libraries: "installing" is
only *where the directory lives*, never a different package shape.

## Versioning

One version per library name ships in the first increment; the manifest `version`
records it. A directory-per-version layout (to carry multiple versions of the
same library) is a defined future extension, not part of the first increment.
Reference version handling (the common `*` wildcard, pinned mismatches) is
specified in [Compatibility Libraries §Reference matching](compatibility-libraries.md).

## References

- [Compatibility Libraries](compatibility-libraries.md) — the behavioral design
  this format serves.
- [Compatibility Library Authoring policy](../steering/compatibility-library-authoring.md)
  — how libraries are authored and how provenance is recorded.
- [Bytecode Container Format](bytecode-container-format.md) — the format-spec
  rigor this document mirrors.
