# Design: Compatibility Library Format

## Overview

This document specifies the **on-disk format** of a compatibility library: the
package layout, the manifest, and how versions are organized. It is the format
counterpart to the behavioral design in
[Compatibility Libraries](compatibility-libraries.md).

## Non-affiliation

A compatibility library reproduces a vendor's *interface* (names, signatures) for
interoperability, and its manifest may name that vendor as a fact. This implies
no relationship with the vendor:

> IronPLC is an independent open-source project. It is not affiliated with,
> endorsed by, or sponsored by any third party.

The `vendor` field is nominative — it records whose interface the library mirrors
— not a claim of endorsement.

## Design Goals

1. **Self-describing package** — a library is a directory that carries everything
   the compiler needs to load it; no external index or registry.
2. **One format, two consumers** — the same on-disk files are read by the CLI
   compiler and served as plain text to the browser playground.
3. **Portable** — the package is a plain filesystem directory.

## Package Layout

**REQ-LF-sources-001** A compatibility library is a directory named for the
library, containing a `library.toml` manifest and one subdirectory per version,
each named by the version and holding that version's `.st` declaration files:

```
Tc2_System/
├── library.toml
└── 1.0.0/
    └── Tc2_System.st        # one or more .st files
```

The directory name is the library's identity — what a project reference or
`--library` names, matched exactly and case-sensitively (see
[Compatibility Libraries §Reference matching](compatibility-libraries.md)).

## Manifest

**REQ-LF-sources-002** The manifest is `library.toml` with these fields:

| Requirement | Field | Type | Required | Meaning |
|-------------|-------|------|----------|---------|
| **REQ-LF-sources-002** | `name` | string | yes | Library identity; equals the directory name and the vendor library name (e.g. `Tc2_System`). |
| | `vendor` | string | yes | Whose interface the library mirrors (e.g. `Beckhoff Automation GmbH`), or `IronPLC` for own libraries. Nominative — see *Non-affiliation*. |
| | `default_version` | string | yes | The version used when a reference does not pin one; names one of the version subdirectories. |
| **REQ-LF-sources-004** | `references` | array of string | yes | The **public references** the library was authored from — documentation URLs/citations. Facts, not a legal judgment. Non-empty. |

Example:

```toml
name = "Tc2_System"
vendor = "Beckhoff Automation GmbH"
default_version = "1.0.0"
references = [
  "https://infosys.beckhoff.com/english.php?content=../content/1033/tcplclib_tc2_system/31084171.html&id= — PI global constant",
]
```

## Declarations

Declarations are IEC 61131-3 Structured Text in the `.st` files under a version's
subdirectory. By default every declaration is fully defined ST — for
`Tc2_System`, `PI` is a `VAR_GLOBAL CONSTANT` — and a POU's ST body is its
implementation. A POU whose implementation is *not* an ordinary ST body is
declared the same way but marked in the manifest's *Bindings* table (below).

## Bindings

A **binding** maps a POU in a specific version to a non-default
implementation. Bindings exist for the two cases
[ADR-0042](../adrs/0042-library-functions-over-compiler-intrinsics.md)
carves out of the ST-body default: semantics that cannot be expressed in IEC
61131-3 source (backed by an *unnamed* native VM builtin), and declarations
whose implementation has not landed yet (*declare-only*).

**REQ-LF-sources-005** A manifest may carry a per-version bindings table,
keyed by the version, mapping a POU name to a binding. The version key must
be quoted — `["1.0.0".bindings]`; the unquoted form `[1.0.0.bindings]` is
three nested TOML tables and is rejected. A binding value is one of exactly
two forms:

- `{ intrinsic = "<name>" }` — calls to the POU lower to the named native VM
  builtin. The name is an internal compiler/VM identifier, not a callable
  name: the builtin adds no name to any scope, and the library is the only
  way to reach it (ADR-0042 rule 3).
- `"declare-only"` — the POU's declared signature exists so the library's
  surface can land ahead of its implementation; *calling* the POU is a
  compile error (see
  [Compatibility Libraries](compatibility-libraries.md) *fail-if-unimplemented*).

```toml
["1.0.0".bindings]
LTRUNC = { intrinsic = "trunc_lreal" }
LMOD   = { intrinsic = "fmod_lreal" }
```

**REQ-LF-sources-006** A malformed bindings entry — a version key whose
value is not a table, a `bindings` value that is not a table, a binding
value that is neither the `intrinsic` inline-table form nor the string
`"declare-only"`, or an unquoted dotted version key producing nested tables
— is rejected with `P6010` anchored on the manifest file. Shape validation
covers **every** version table in the manifest, not only the
`default_version`, so the unquoted-key mistake cannot hide in an inactive
version.

**REQ-LF-sources-007** A bound or declare-only POU still appears in its
version's `.st` files with its full interface (name, parameters, return
type) and a body of exactly `;` (an empty statement). The declaration is
what makes `check` and type resolution work unchanged; the `;` body is
never the implementation — codegen either lowers calls to the bound builtin
or rejects calls to a declare-only POU (see
[Compatibility Libraries](compatibility-libraries.md)).

## Versioning

Each version of a library is a subdirectory named by the version.
`default_version` in the manifest selects the one used when a reference does not
pin a version. Reference version handling (the common `*` wildcard, pinned
mismatches) is specified in
[Compatibility Libraries §Reference matching](compatibility-libraries.md).

## Installation

Libraries are **installed on disk** as directories in this format and read by the
compiler at runtime — they are *not* embedded into the compiler binary. The same
files are served as plain text to the playground.

The install location is `resources/libs/` **beside the compiler executable**
(i.e. `<bindir>/resources/libs/<LibraryName>/…`); the loader
(`installed_libraries_root`) reads from there and falls back to the crate source
tree only for development and test builds. Every installer therefore ships
`resources/libs/` next to the binaries: the Linux/macOS tarball carries it, the
Windows NSIS installer writes it under `bin\resources\libs`, and the Homebrew
formula keeps the binaries and `resources/libs` together in `libexec` and
symlinks the executables onto the `PATH` (`current_exe()` resolves the symlink
back to `libexec`, so the loader finds the libraries beside the real binary).
The discovery/search mechanism beyond this fixed location is **not part of this
format** and is defined separately when needed.

## References

- [Compatibility Libraries](compatibility-libraries.md) — the behavioral design
  this format serves.
- [Compatibility Library Authoring policy](../steering/compatibility-library-authoring.md)
  — how libraries are authored and how provenance is recorded.
