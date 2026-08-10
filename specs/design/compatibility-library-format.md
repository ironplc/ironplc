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
| **REQ-LF-sources-008** | `implicit` | boolean | no (default `false`) | Marks a library the vendor environment provides to every project without a reference (built-in surface). An implicit bundled library activates automatically when a TwinCAT project (`.plcproj`) is discovered — see [Compatibility Libraries §Activation channels](compatibility-libraries.md). A non-boolean value is a manifest error. |

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
subdirectory. In the first increment every declaration is fully defined ST — for
`Tc2_System`, `PI` is a `VAR_GLOBAL CONSTANT`.

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

## Future

- **Bindings.** A per-version table keyed by the version — e.g.
  `[1.0.0.bindings]` — mapping a POU to a non-default implementation (a VM
  intrinsic, or declare-only). The fail-if-unimplemented rule (a call to a
  declare-only POU is a compile error) arrives with bindings; see
  [Compatibility Libraries](compatibility-libraries.md).

## References

- [Compatibility Libraries](compatibility-libraries.md) — the behavioral design
  this format serves.
- [Compatibility Library Authoring policy](../steering/compatibility-library-authoring.md)
  — how libraries are authored and how provenance is recorded.
