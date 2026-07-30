# Design: Library Support

## Overview

IronPLC's standard library (TON, CTU, string functions, …) is hand-written Rust
baked into the compiler (`analyzer/src/stages.rs`, `analyzer/src/stdlib.rs`).
**That Rust-baked standard library stays.** This design adds a complementary
mechanism: **compatibility libraries** — IronPLC-authored IEC 61131-3 **source**
that the compiler reads and compiles through the same pipeline as user code,
shipped and installed alongside the compiler.

The purpose is to provide implementations **equivalent to the base libraries of
other runtimes** (e.g. Beckhoff TwinCAT and CODESYS), so a program written for
one of those runtimes compiles against IronPLC-authored source instead of
requiring new Rust and a per-function `--allow-*` flag for every missing function
(e.g. `LTRUNC`/`LMOD`, [#1217](https://github.com/ironplc/ironplc/pull/1217),
part of #1199).

**This is deliberately not a complete, general library system, and not a
replacement for the Rust-baked standard library.** It is a targeted way to ship
runtime-equivalent library content as source, opted into per library. IronPLC
provides only its **own** source; it does **not** redistribute third-party vendor
libraries (Beckhoff `Tc2_*`, etc.) — a licensing non-starter.

### Scope

**In scope:** shipping IronPLC-authored `.st` libraries, each selected
**explicitly** (by a compiler flag and/or a library reference in project
configuration), loaded from disk, compiled by the normal pipeline, verified by
CI, and installed with the compiler. The first library defines the constant `PI`.

**Out of scope (future designs):**

- A complete/general library system (versioning, dependency resolution, a full
  reference model). Libraries here are a flat set of named files, opted into
  individually.
- Functions that require an operation the language cannot express (a float→float
  truncation, etc.) and the compiler/VM primitives they need — `LTRUNC`/`LMOD`
  are deferred to a follow-on design that builds on this foundation.

### Goals

- Ship IronPLC-authored compatibility libraries as `.st` source the compiler
  reads.
- Let a user **opt into** a library by name (compiler flag and/or project-config
  reference); dialects do not select libraries.
- Install the library files alongside the compiler so users can see and read
  them.
- Verify in CI that every shipped library file compiles.
- Ship a first library that defines `PI` (the right resolution for #1219).

### Approach

- Libraries ship with the toolchain but are **opted into explicitly**, the way a
  TwinCAT/CODESYS project adds a library reference or a C build links a library —
  not implicitly global. Those runtimes offer many libraries and you opt into the
  specific ones you use.
- Library code is **ordinary source** compiled by the same front end. `analyze()`
  merges selected libraries with user code into one unit, and the reachability
  pass prunes everything unused — **including global constants** — so a library
  can define `PI` and it is emitted only when a program uses it.

## Architecture

### Library source

**REQ-LIB-sources-100** IronPLC ships library source under `compiler/library/`.
Each immediate subdirectory of `compiler/library/` is one library, identified by
its directory name.

**REQ-LIB-sources-102** Library source files are `.st` files, parsed by the same
parser as user source, with no library-only grammar.

**REQ-LIB-sources-180** The first shipped library defines the constant `PI` (an
`LREAL` global constant), providing the resolution for #1219 in library source
rather than a compiler flag.

Keeping the files as real `.st` source (rather than Rust string literals) makes
them reviewable, diffable, editable, **visible to users after install**, and —
critically — compilable by the same CI that builds user programs (see
[Build-time compile check](#build-time-compile-check)).

### Selecting a library

**REQ-LIB-sources-130** A library is included in a compilation only when
explicitly selected — by a compiler flag naming the library, and/or by a library
reference embedded in project configuration (e.g. a TwinCAT `.plcproj`
`<LibraryReference>`). Dialects do not select libraries, and no library is
included by default.

Selection is resolved in the CLI/`sources` layer. The **parser crate does not
learn about libraries**; if library selection ever needs option plumbing,
`CompilerOptions` should be restructured so the parser holds only its
syntax-affecting subset and the CLI owns the rest.

### Compiling a library — per-file compiler flags via pragma

A library may need to be compiled with compiler flags that differ from the
user's. The immediate case: defining `PI` needs a **global constant declaration
block** (a top-level `VAR_GLOBAL CONSTANT`, gated by `--allow-top-level-var-global`
today). Rather than compile all libraries under a fixed flag set, a library
source file declares the flags it needs via a **pragma**.

**REQ-LIB-parser-200** A source file may enable specific compiler flags for its
own compilation via a pragma. Initially this covers allowing a global constant
declaration block (needed to define `PI`). The mechanism is a general,
file-scoped flag directive — it is not library-specific.

This separates two kinds of rule, and the pragma feeds both: **parse-time rules**
a pragma switches on while parsing, and **analysis rules** that operate on what
was written. Start with only the flag `PI` needs; grow the recognized set as
further libraries require it, toward a general mechanism.

### Semantic integration

**REQ-LIB-analyzer-300** POUs, types, and global variables exported by a selected
library resolve in user code as if declared in the compilation, using the
existing merge performed by `analyze()`.

**REQ-LIB-analyzer-330** A user declaration whose name collides with a library
export is a duplicate-definition error, reported with the existing
duplicate-definition diagnostic — the same as colliding with any other
declaration.

### Code generation and linking

**REQ-LIB-codegen-400** After merging selected libraries with user code,
declarations not reachable from a PROGRAM root — POUs **and global constants** —
are pruned and omitted from the output container. This extends the existing
`xform_toposort_declarations` reachability pass to global constants, so an unused
library `PI` is not emitted.

**REQ-LIB-codegen-410** A call to a library function is compiled as a `CALL` to
the compiled library POU, using the same path as a call to a user-defined
function.

### Build-time compile check

**REQ-LIB-sources-190** Every source file under `compiler/library/` parses,
analyzes, and compiles to a container without error — each compiled with the
flags its pragma declares. CI fails if any shipped library file does not compile.

Implemented as an integration test that runs the real `check()` + `compile()`
entry points over the `compiler/library/` tree (directory input is already
supported; see the existing `check(&[resource_path("set")], …)` test in
`ironplc-cli/src/cli.rs`) and asserts success. It runs as part of `just`
(`compile`/`coverage`) so a change that breaks a library file fails CI the same
as any other test. It is the guardrail the current Rust-baked stdlib lacks.

## Delivery: installed alongside the binary

Library files are shipped as files and installed next to the compiler, then
loaded from disk at runtime. This keeps the library **visible and readable to
users** and makes the set **extensible** — adding or adjusting a library is a
file change, not a compiler rebuild.

**REQ-LIB-sources-110** The compiler locates its library directory relative to
the installed executable, independent of the current working directory and of the
user's project directory.

Because the library is read from disk as ordinary files, its elements carry
normal file-based `FileId`s — no special builtin tagging is needed, and
diagnostics and debug info point at the real library file on disk.

The library tree is staged into each install layout the way `bom.cdx.json` is
staged today:

- `compiler/justfile` `_package-macos` / `_package-linux`: `cp -r` the library
  tree into `target/<target>/release/` and add it to the `tar` argument list
  (alongside `ironplcc ironplcvm ironplcmcp bom.cdx.json`).
- `compiler/setup.nsi`: a new section `SetOutPath "$INSTDIR\lib"` +
  `File /r "..\library\*"`, mirroring the existing `$INSTDIR\examples` section.
- `compiler/install.sh`: after extraction, copy the library directory into
  `${INSTALL_DIR}/lib` (the script currently extracts only named binaries).
- `compiler/homebrew/Formula/ironplc.rb`: `(share/"ironplc").install Dir["lib/*"]`.

The existing release workflows (`partial_compiler.yaml`,
`partial_upload_release_artifacts.yaml`) manipulate whole artifacts and need no
change.

## Requirement ownership summary

One area code, `LIB`, partitioned by owning crate:

| Block | Slug (crate) | Concern |
|-------|--------------|---------|
| `1xx` | `sources` | library location, explicit selection, disk delivery, runtime path resolution, `PI` content, compile check |
| `2xx` | `parser` | per-file compiler-flag pragma |
| `3xx` | `analyzer` | library exports in scope, duplicate-definition on collision |
| `4xx` | `codegen` | reachability-pruned linking of POUs and global constants |

Final slug placement for any requirement is confirmed when its slice wires the
doc into that crate's `build.rs`.

## Implementation Breakdown

The work splits into independently reviewable PRs. Because the spec-conformance
orphan guard treats a doc as "enforced" only once a `build.rs` lists it — and
then requires **every** slug used in the doc to be claimed by a listing crate —
enforcement is turned on once, up front.

- **PR 1 — this document.** Design only; not wired into any `build.rs`.
- **PR 2 — enforcement bootstrap.** Wire `library-support.md` into the `build.rs`
  of every owning crate (`sources`, `parser`, `analyzer`, `codegen`) and add
  `#[ignore]`d `#[spec_test]` stubs for every requirement (mirrors how
  `reference-to-twincat.md` established enforcement). Each later PR removes
  `#[ignore]` as it implements.
- **PR 3 — per-file flag pragma** (`parser`): the pragma that lets a file enable
  the flag `PI` needs (a global constant block); REQ-LIB-parser-200. Prerequisite
  for the `PI` library.
- **PR 4 — first library (`PI`) + compile check** (`sources`): create
  `compiler/library/<name>/` defining `PI` with the pragma from PR 3, and the
  REQ-LIB-sources-190 compile-check test; REQ-LIB-sources-100/102/180/190. Locks
  in the guardrail and gives a concrete first library.
- **PR 5 — delivery + packaging** (`sources`): install the library alongside the
  binary and resolve it at runtime relative to the executable; update all
  installers; REQ-LIB-sources-110.
- **PR 6 — selection + pipeline integration** (`sources` + `analyzer` +
  `codegen`): opt-in selection by compiler flag and/or project-config reference;
  merge selected libraries into the compilation; exports resolve;
  duplicate-definition on collision; reachability pruning extended to global
  constants; REQ-LIB-sources-130, REQ-LIB-analyzer-300/330,
  REQ-LIB-codegen-400/410.
- **PR 7+ — additional libraries + docs.** Add further compatibility libraries as
  needed and user documentation. (No porting of the Rust-baked stdlib — it
  remains.)

Rough order: PR 2 → PR 3 → PR 4 → PR 5 → PR 6 → PR 7+.

## Relationship to existing work and ADRs

- **Reframes #1219 and #1217.** `PI` (#1219) becomes this design's first library
  (source, not a compiler flag). `LTRUNC`/`LMOD` (#1217) become library content
  once the follow-on primitives design lands. Both stop being per-function
  `--allow-*` flags.
- **Additive, not a replacement.** The Rust-baked standard library remains; this
  ships *additional* compatibility content as source.
- Complements **[ADR-0012](../adrs/0012-accept-vendor-dialect-files-as-is.md)**
  (accept vendor dialect files as-is) — the library provides the compatibility
  counterpart in IronPLC-authored source.
- Conformance mechanics:
  **[cross-crate-spec-conformance.md](cross-crate-spec-conformance.md)** and
  **[ADR-0037](../adrs/0037-mandatory-crate-slug-in-requirement-ids.md)**.

### Decisions to ratify as ADRs

1. IronPLC ships **source** compatibility libraries compiled by the normal
   pipeline, **additive to** (not replacing) the Rust-baked standard library.
2. IronPLC authors its own compatibility libraries; it does not redistribute
   third-party vendor libraries.
3. Libraries are opted into **explicitly** (compiler flag and/or project
   reference); dialects do not select libraries.
4. Library files are **installed alongside the binary and loaded from disk**.
5. A library declares the compiler flags it needs via a **pragma** (general,
   file-scoped).
