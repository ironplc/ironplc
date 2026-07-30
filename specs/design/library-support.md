# Design: Library Support

## Overview

IronPLC has no library mechanism. Its "standard library" (TON, CTU, string
functions, …) is hand-written Rust baked into the compiler and registered
programmatically (`analyzer/src/stages.rs`, `analyzer/src/stdlib.rs`). Every
function IronPLC lacks — e.g. Beckhoff's `LTRUNC`/`LMOD`
([#1217](https://github.com/ironplc/ironplc/pull/1217), part of #1199) — today
requires new Rust and, often, a per-function `--allow-*` flag. That does not
scale and pushes ordinary library functions into the compiler.

This design introduces a **library**: IronPLC's own IEC 61131-3 **source** that
the compiler reads and compiles through the same pipeline as user code, shipped
with the compiler and installed alongside it. It follows the conventional model
— a library that ships with the toolchain and is available to every program,
like a language's standard library.

IronPLC provides its **own** libraries. It does **not** redistribute third-party
vendor libraries (Beckhoff `Tc2_*`, etc.) — that is a licensing non-starter.
Instead, to be compatible with vendor and edition dialects, IronPLC provides
**compatibility libraries** it authors itself.

### Scope

**In scope:** shipping a single IronPLC-authored library as `.st` source,
selected by the active dialect/edition, loaded from disk, compiled by the normal
pipeline, verified by CI, and installed with the compiler.

**Out of scope (future designs):**

- *How to offer multiple or vendor-specific compatibility libraries* and
  reference them individually. For now IronPLC offers a single dialect-matched
  library.
- Functions that require an operation the language cannot express (a float→float
  truncation, etc.) and the compiler/VM support they need. `LTRUNC`/`LMOD` fall
  here and are deferred to a follow-on design; this design builds the library
  foundation they will use.

### Goals

- Ship IronPLC's library as IEC 61131-3 `.st` source the compiler reads, not
  Rust baked into the binary.
- Make the dialect-appropriate library available to every compilation.
- Install the library files alongside the compiler so users can see and read
  them.
- Verify in CI that every shipped library file compiles.

### Approach: what other compilers do

- A library ships with the toolchain and is available by default (Rust's
  `std`/prelude, Go's `std`, the C runtime).
- Library code is **ordinary source** compiled by the same front end. The
  compiler already supports this: `analyze()` merges all units into one library
  and `xform_toposort_declarations` prunes everything unreachable from a PROGRAM
  root, so the library can be supplied and only used POUs are emitted.

## Architecture

### Library source

**REQ-LIB-sources-100** IronPLC ships library source under `compiler/library/`.
Each immediate subdirectory of `compiler/library/` is one library, named by the
directory.

**REQ-LIB-sources-101** A library is named to match a dialect/edition (the same
names the compiler uses for its dialects), so the standard library can differ by
IEC 61131-3 edition. The library matching the active dialect is available to
every compilation by default. IronPLC ships a single such library initially.

**REQ-LIB-sources-102** Library source files are `.st` files, parsed by the same
parser as user source with no library-only grammar.

Keeping the files as real `.st` source (rather than Rust string literals) makes
them reviewable, diffable, editable, **visible to users after install**, and —
critically — compilable by the same CI that builds user programs (see
[Build-time compile check](#build-time-compile-check)).

### Delivery: installed alongside the binary

Library files are shipped as files and installed next to the compiler, then
loaded from disk at runtime. This keeps the library **visible and readable to
users** and makes the set **extensible** — adding or adjusting a library is a
file change, not a compiler rebuild.

**REQ-LIB-sources-110** The compiler locates its library directory relative to
the installed executable, independent of the current working directory and of
the user's project directory.

**REQ-LIB-sources-140** If the library for the active dialect cannot be located
or read, the compiler emits a clear diagnostic (proposed `P4039`) identifying
the missing library, rather than silently omitting standard functions.

Because the library is read from disk as ordinary files, its elements carry
normal file-based `FileId`s — no special builtin tagging is needed, and
diagnostics and debug info point at the real library file on disk.

### Semantic integration

**REQ-LIB-analyzer-300** POUs, types, and global variables exported by the
active library resolve in user code as if declared in the compilation, using the
existing merge performed by `analyze()`.

**REQ-LIB-analyzer-330** A user declaration whose name collides with a library
export is a duplicate-definition error, reported with the existing
duplicate-definition diagnostic — the same as colliding with any other
declaration.

### Code generation and linking

**REQ-LIB-codegen-400** Library POUs reachable from a PROGRAM root are compiled
and linked into the output container; library POUs not reachable from any PROGRAM
root are omitted (the existing `xform_toposort_declarations` reachability pass,
applied to merged library + user code).

**REQ-LIB-codegen-410** A call to a library function is compiled as a `CALL` to
the compiled library POU, using the same path as a call to a user-defined
function.

### Build-time compile check

**REQ-LIB-sources-190** Every source file under `compiler/library/` parses,
analyzes, and compiles to a container without error. CI fails if any shipped
library file does not compile.

Implemented as an integration test that runs the real `check()` + `compile()`
entry points over the `compiler/library/` tree (directory input is already
supported; see the existing `check(&[resource_path("set")], …)` test in
`ironplc-cli/src/cli.rs`) and asserts success. It runs as part of `just`
(`compile`/`coverage`) so a change that breaks a library file fails CI the same
as any other test. It is the guardrail the current Rust-baked stdlib lacks.

## Compiler-options boundary

Selecting the library follows the active dialect/edition, which the compiler
already knows (`--dialect`). The **parser crate must not learn about libraries**
— library location, loading, and selection live in the CLI/`sources` layer. No
library option is added to the parser by this design. If library-related options
are needed later, `CompilerOptions` should be restructured so the parser keeps
only the minimal syntax-affecting subset and the CLI owns the rest (either by
moving `CompilerOptions` into its own crate, or by the CLI defining the full
option set with a mapping down to the parser's subset).

## Packaging and installers

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
- Runtime path-resolution locates the library directory relative to the
  executable (REQ-LIB-sources-110).

The existing release workflows (`partial_compiler.yaml`,
`partial_upload_release_artifacts.yaml`) manipulate whole artifacts and need no
change.

## Problem codes

- `P4039` — the library for the active dialect could not be located or read
  (REQ-LIB-sources-140). Next free code in the analyzer `P40xx` range; final
  number allocated when implemented, documented under
  `docs/reference/compiler/problems/`.

## Requirement ownership summary

One area code, `LIB`, partitioned by owning crate:

| Block | Slug (crate) | Concern |
|-------|--------------|---------|
| `1xx` | `sources` | library location, dialect selection, disk delivery, runtime path resolution, load diagnostic, compile check |
| `3xx` | `analyzer` | library exports in scope, duplicate-definition on collision |
| `4xx` | `codegen` | reachability-pruned linking of library POUs |

Final slug placement for any requirement is confirmed when its slice wires the
doc into that crate's `build.rs`.

## Implementation Breakdown

The work splits into independently reviewable PRs. Because the spec-conformance
orphan guard treats a doc as "enforced" only once a `build.rs` lists it — and
then requires **every** slug used in the doc to be claimed by a listing crate —
enforcement is turned on once, up front.

- **PR 1 — this document.** Design only; not wired into any `build.rs`.
- **PR 2 — enforcement bootstrap.** Wire `library-support.md` into the `build.rs`
  of every owning crate (`sources`, `analyzer`, `codegen`) and add `#[ignore]`d
  `#[spec_test]` stubs for every requirement, so the requirements become
  enforced-but-pending (mirrors how `reference-to-twincat.md` established
  enforcement). Each later PR removes `#[ignore]` as it implements.
- **PR 3 — library skeleton + compile check** (`sources`): create
  `compiler/library/<dialect>/`, add one or two trivially-correct `.st` POUs, and
  the REQ-LIB-sources-190 compile-check test. Small; locks in the guardrail first
  and unblocks authoring.
- **PR 4 — delivery + packaging** (`sources`): install the library alongside the
  binary and resolve it at runtime relative to the executable; update all
  installers; REQ-LIB-sources-110/140.
- **PR 5 — pipeline integration** (`sources` + `analyzer` + `codegen`): load the
  dialect's library into every compilation; exports resolve; duplicate-definition
  on collision; reachability-pruned linking; REQ-LIB-analyzer-300/330,
  REQ-LIB-codegen-400/410.
- **PR 6+ — library content**: port stdlib functions from Rust to library source
  incrementally; user documentation.

Rough order: PR 2 → PR 3 → PR 4 → PR 5 → PR 6+.

## Relationship to existing work and ADRs

- **Foundational for #1199 and reframes #1217.** Functions IronPLC lacks become
  library content rather than per-function `--allow-*` flags. Functions that need
  new compiler/VM primitives (`LTRUNC`/`LMOD`) are a follow-on design that builds
  on this library foundation.
- Complements **[ADR-0012](../adrs/0012-accept-vendor-dialect-files-as-is.md)**
  (accept vendor dialect files as-is) — the library provides the compatibility
  counterpart in IronPLC-authored source.
- Conformance mechanics:
  **[cross-crate-spec-conformance.md](cross-crate-spec-conformance.md)** and
  **[ADR-0037](../adrs/0037-mandatory-crate-slug-in-requirement-ids.md)**.

### Decisions to ratify as ADRs

1. IronPLC ships a **source** library compiled by the normal pipeline, replacing
   the Rust-baked stdlib.
2. IronPLC authors its own **compatibility libraries**; it does not redistribute
   third-party vendor libraries.
3. Library files are **installed alongside the binary and loaded from disk**
   (visible and extensible), not embedded.
