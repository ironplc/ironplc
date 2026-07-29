# Design: Library Support

## Status

Design document for the library-support subsystem. This PR contains the design
only — the requirement markers below are **not yet enforced** (the doc is not
listed in any crate's `build.rs`, so the spec-conformance guard does not scan
it). Each implementation slice wires the doc into its owning crate's `build.rs`
and adds the `#[spec_test]` tests for the requirements it implements. See
[Implementation Breakdown](#implementation-breakdown). This is expected to land
as multiple PRs.

## Overview

IronPLC has no library mechanism. Its "standard library" (TON, CTU, string
functions, …) is hand-written Rust baked into the compiler and registered
programmatically (`analyzer/src/stages.rs`, `analyzer/src/stdlib.rs`). When a
vendor function is missing — e.g. Beckhoff's `LTRUNC`/`LMOD`
([#1217](https://github.com/ironplc/ironplc/pull/1217), part of #1199) — the
current path is to register a single signature behind a per-function
`--allow-*` flag. That approach does not scale (one flag and one code change per
function), miscategorizes ordinary functions as compiler intrinsics, and ships
no implementation (the signature type-checks, then codegen has nothing to emit).

This design introduces a real **library** mechanism, following the approach used
by other compilers: a **bundled standard library shipped as source**, implicitly
available like a prelude; **additional libraries referenced explicitly** and
resolved from a search path; all library code compiled through the **same
pipeline** as user code; and a small set of **VM primitives** for the few
operations that cannot be expressed in the language itself.

### Goals

- Ship IronPLC's own standard library as IEC 61131-3 **source** the compiler
  reads, not Rust baked into the binary.
- Let user programs use library POUs/types by **referencing** a library, with a
  standard library available by default.
- Verify in **CI that every bundled library file compiles**, so the library
  cannot silently break.
- Ship the library files with every installer.
- Provide the **primitive opcodes** that let library functions like
  `LTRUNC`/`LMOD` be authored outside the compiler.

### Non-goals

- **Redistributing third-party vendor libraries** (Beckhoff `Tc2_*`, etc.).
  IronPLC ships only its own library. Consuming a user's *installed* vendor
  libraries is a downstream extension that reuses this design's reference and
  resolution mechanism; it is out of scope here.
- A full namespace / version / transitive-dependency resolver. First cut is
  name-based resolution of a flat set of bundles.

### Approach: what other compilers do

This mirrors the conventional model:

- A **bundled standard library** ships with the toolchain and is implicitly in
  scope (Rust's `std`/prelude, Go's `std`, the C runtime on the default include
  path). `--no-stdlib` opts out, as `#![no_std]` does.
- **Additional libraries are referenced explicitly** and resolved from a
  **search path** (Rust `extern crate` + `-L`, C `#include` + `-I`, CODESYS/
  TwinCAT library references + repository).
- Library code is **ordinary source** compiled by the same front end. The
  compiler already supports this: `analyze()` merges all units into one library
  and `xform_toposort_declarations` prunes everything unreachable from a PROGRAM
  root, so a large library can be supplied and only used POUs are emitted.
- The rare function that the language cannot express (e.g. a float→float
  truncation) binds to a **compiler/VM primitive**. Per ADR-0008 these are
  `func_id`s under the single `BUILTIN` opcode, so the compiler grows
  *primitives*, not *functions*.

## Architecture

### Library source location and authoring

**REQ-LIB-sources-100** IronPLC ships a standard library of IEC 61131-3 source
files under `compiler/library/`. Each immediate subdirectory of
`compiler/library/` is one named library bundle (the directory name is the
bundle name).

**REQ-LIB-sources-101** The bundle named `standard` is available to every
compilation by default, without an explicit reference, unless `--no-stdlib` is
set.

**REQ-LIB-sources-102** Library source files use the same Structured Text / IEC
syntax and file extensions (`.st`, `.iec`) as user source and are parsed by the
same parser with no library-only grammar.

Keeping the files as real source under `compiler/library/` (rather than Rust
string literals) makes them reviewable, diffable, editable by contributors, and
— critically — **compilable by the same CI that builds user programs** (see
[Build-time compile check](#build-time-compile-check)).

### Delivery to the compiler — Open decision D1

The library files must reach the *installed* compiler. Two mechanisms, both
keeping the source under `compiler/library/`:

- **Option A — Embed at build time (recommended).** A `build.rs` generator emits
  the bundle contents into the binary via `include_str!` (the pattern already
  used for embedded spec/schema text, e.g. `mcp/build.rs`). The installed
  `ironplcc` carries the library with it; no runtime filesystem lookup; behaves
  identically in tests, CI, and production; **no installer changes required**.
- **Option B — Install alongside the binary.** Ship the `compiler/library/` tree
  into the install layout (`$INSTDIR/lib`, `$HOME/.ironplc/lib`) and resolve it
  at runtime relative to the executable. Requires **new runtime path-resolution
  code** (none exists today) and edits to all four installers (see
  [Packaging](#packaging-and-installers)).

**Recommendation: Option A.** It matches how the compiler already ships and reads
bundled files, eliminates a class of "library not found next to the binary"
runtime failures, and needs no installer work. Option B's only advantages are
post-install visibility and user-editability of the files, which a standard
library does not need. This is the primary decision to confirm on this PR.

The requirements below are written to hold under either option; those specific
to Option A are marked.

**REQ-LIB-sources-110** Library bundle sources are available to the compiler
independent of the current working directory and of the user's project
directory.

**REQ-LIB-sources-111** *(Option A)* Library sources are embedded into the
`ironplcc` binary at build time; the installed compiler loads them with no
filesystem access.

**REQ-LIB-sources-120** Elements originating from a library bundle are tagged
with a builtin/library `FileId` (see `FileId::builtin()` in `dsl/src/core.rs`)
distinct from any user `FileId`, so diagnostics and debug info attribute them to
the library rather than to a user file.

### Reference and resolution

**REQ-LIB-sources-130** A library reference names a bundle. The compiler resolves
the bundle by name against a **library search path**: the built-in/bundled set
plus any directories added via `--library-path`.

**REQ-LIB-sources-131** When a project is discovered from a `.plcproj`, each
`<LibraryReference>` / placeholder entry in the project file is extracted as a
library reference (discovery currently reads only `<Compile Include>`).

**REQ-LIB-sources-140** Referencing a bundle that cannot be resolved on the
search path produces a diagnostic (proposed `P4039`, "referenced library not
found") naming the missing bundle, rather than a generic error.

**REQ-LIB-parser-200** The `--no-stdlib` compiler option disables the implicit
availability of the `standard` bundle.

**REQ-LIB-parser-210** The `--library <name>` compiler option references an
additional bundle by name; it is repeatable.

**REQ-LIB-parser-220** The `--library-path <dir>` compiler option adds a
directory to the library search path; it is repeatable.

These three become fields on `CompilerOptions` (owned by the `parser` crate),
wired through the CLI `FileArgs` alongside the existing `--allow-*` flags.

### Semantic integration

**REQ-LIB-analyzer-300** POUs, types, and global variables exported by an
in-scope library bundle resolve in user code as if declared in the compilation,
using the existing merge performed by `analyze()`.

**REQ-LIB-analyzer-310** A bundle's exports are in scope only when the bundle is
referenced. The `standard` bundle is implicitly referenced unless `--no-stdlib`;
all other bundles are in scope only when referenced via `--library`,
`--library-path`, or a project `<LibraryReference>`.

**REQ-LIB-analyzer-320** A call to a function that exists only in a bundle that
is **not** referenced produces a diagnostic (proposed `P4040`, "function
requires an unreferenced library") that names the bundle providing it, distinct
from the generic undeclared-function diagnostic (`P4017`).

**REQ-LIB-analyzer-330** A user declaration whose name matches a library export
shadows the library export within the user compilation (local-over-library
precedence); it is not a redefinition error.

**REQ-LIB-analyzer-340** A function declared with an external-binding attribute
(see [Primitive-backed functions](#primitive-backed-library-functions)) is
treated as declared — no undeclared-function diagnostic — and its signature is
taken from the declaration.

### Code generation and linking

**REQ-LIB-codegen-400** Library POUs reachable from a PROGRAM root are compiled
and linked into the output container; library POUs not reachable from any
PROGRAM root are omitted (the existing `xform_toposort_declarations` reachability
pass, applied to merged library + user code).

**REQ-LIB-codegen-410** A call to a source-implemented library function is
compiled as a `CALL` to the compiled library POU, using the same path as a call
to a user-defined function.

**REQ-LIB-codegen-420** A call to an external-binding library function emits the
bound `BUILTIN` opcode inline at the call site (no `CALL` frame), the way
`compile_trunc` already inlines its conversion.

### Primitive-backed library functions

Most of a library is ordinary ST. A few functions need an operation the language
cannot express; those bind to a VM primitive. `LTRUNC`/`LMOD` are the motivating
case — the VM today has no float→float truncation and no floating-point modulo.

**REQ-LIB-container-500** The `BUILTIN` `func_id` table defines float truncation
(`TRUNC_F32`, `TRUNC_F64`) and floating-point modulo (`MOD_F32`, `MOD_F64`)
primitives, with corresponding `arg_count` entries (ADR-0008; no new opcode
slots).

**REQ-LIB-vm-550** Executing a float-truncation primitive removes the operand's
fractional part toward zero and yields a float of the same width; the result is
**not** clamped to any integer type's range.

**REQ-LIB-vm-551** Executing a floating-point-modulo primitive yields the
floating remainder such that `MOD_F64(400.56, 360.0) = 40.56`; a zero divisor
yields NaN (consistent with float division).

The binding surface has two tiers, matching the two categories of library
function:

| Kind | Declaration | Codegen |
|------|-------------|---------|
| External-binding alias (`LTRUNC`, `LMOD`) | `{external := 'TRUNC_F64'}`, no body | inline `BUILTIN` (REQ-LIB-codegen-420) |
| Source POU (most functions) | ordinary ST body | compile + link + prune (REQ-LIB-codegen-400/410) |

With these primitives in place, `LTRUNC`/`LMOD` are declared in the bundled
library as external aliases — no per-function `--allow-*` flag, no bespoke
codegen arm — and adding the next float-trunc/mod-based function is pure library
content.

### Build-time compile check

**REQ-LIB-sources-190** Every source file under `compiler/library/` parses,
analyzes, and compiles to a container without error. CI fails if any bundled
library file does not compile.

This is implemented as an integration test that runs the real `check()` +
`compile()` entry points over the `compiler/library/` tree (directory input is
already supported; see the existing `check(&[resource_path("set")], …)` test in
`ironplc-cli/src/cli.rs`) and asserts success. It runs as part of `just`
(`compile`/`coverage`) so a change that breaks a library file fails CI the same
as any other test. It is the guardrail the current Rust-baked stdlib lacks.

## Packaging and installers

Under **Option A (embed)** no installer changes are needed — the library rides
inside the binary. Under **Option B (install alongside)** the following change,
staging `compiler/library/` into each install layout the way `bom.cdx.json` is
staged today:

- `compiler/justfile` `_package-macos` / `_package-linux`: `cp -r` the library
  tree into `target/<target>/release/` and add it to the `tar` argument list
  (alongside `ironplcc ironplcvm ironplcmcp bom.cdx.json`).
- `compiler/setup.nsi`: a new section `SetOutPath "$INSTDIR\lib"` +
  `File /r "..\library\*"`, mirroring the existing `$INSTDIR\examples` section.
- `compiler/install.sh`: after extraction, copy the `lib` directory into
  `${INSTALL_DIR}/lib` (the script currently extracts only named binaries).
- `compiler/homebrew/Formula/ironplc.rb`: `(share/"ironplc").install Dir["lib/*"]`.
- New runtime path-resolution to locate `lib` relative to the executable.

The existing release workflows (`partial_compiler.yaml`,
`partial_upload_release_artifacts.yaml`) manipulate whole artifacts and need no
change under either option.

## Problem codes

Two new compiler diagnostics are introduced (next free codes in the analyzer
`P40xx` range; final numbers allocated when implemented, documented under
`docs/reference/compiler/problems/`):

- `P4039` — referenced library not found on the search path (REQ-LIB-sources-140).
- `P4040` — function requires a library that is not referenced (REQ-LIB-analyzer-320).

## Requirement ownership summary

One area code, `LIB`, partitioned by owning crate in hundreds-blocks per the
`reference-to-twincat.md` convention:

| Block | Slug (crate) | Concern |
|-------|--------------|---------|
| `1xx` | `sources` | bundle location, delivery, `FileId` tagging, search-path resolution, `.plcproj` reference extraction, not-found diagnostic, compile check |
| `2xx` | `parser` | `CompilerOptions` (`--no-stdlib`, `--library`, `--library-path`) |
| `3xx` | `analyzer` | reference-gated scoping, shadowing, external-binding recognition, unreferenced-library diagnostic |
| `4xx` | `codegen` | reachability-pruned linking of library POUs, external-binding inlining |
| `5xx` | `container` / `vm` | float trunc/mod `BUILTIN` primitives |

Final slug placement for any requirement is confirmed when its slice wires the
doc into that crate's `build.rs`.

## Implementation Breakdown

The work is large; it splits into independently reviewable PRs. Because the
spec-conformance orphan guard treats a doc as "enforced" only once a `build.rs`
lists it — and then requires **every** slug used in the doc to be claimed by a
listing crate — enforcement is best turned on once, up front, rather than
incrementally.

- **PR 1 — this document.** Design only; not wired into any `build.rs`; CI-neutral.
- **PR 2 — enforcement bootstrap.** Wire `library-support.md` into the `build.rs`
  of every owning crate (`sources`, `parser`, `analyzer`, `codegen`, `container`,
  `vm`) and add `#[ignore]`d `#[spec_test]` stubs for every requirement, so the
  requirements become enforced-but-pending (mirrors how `reference-to-twincat.md`
  established enforcement). Each later PR removes `#[ignore]` as it implements.
- **PR 3 — library skeleton + compile check** (`sources`): create
  `compiler/library/standard/`, add one or two trivially-correct `.st` POUs, and
  the REQ-LIB-sources-190 compile-check test. Small, unblocks authoring.
- **PR 4 — delivery** (`sources`): implement Option A embed generator (or Option
  B install path + packaging), satisfying REQ-LIB-sources-110/111/120.
- **PR 5 — pipeline loading + options** (`sources` + `parser`): `--no-stdlib` /
  `--library` / `--library-path`; inject bundled sources into every compilation;
  REQ-LIB-parser-2xx, REQ-LIB-sources-130.
- **PR 6 — reference gating + diagnostics** (`analyzer` + `sources`):
  reference-gated scoping, shadowing, `P4039`/`P4040`; `.plcproj`
  `<LibraryReference>` extraction; REQ-LIB-analyzer-300/310/320/330,
  REQ-LIB-sources-131/140.
- **PR 7 — float primitives** (`container` + `vm`): `TRUNC_*`/`MOD_*`
  `BUILTIN`s; REQ-LIB-container-500, REQ-LIB-vm-550/551. Independent — can land
  in parallel with PRs 3–6.
- **PR 8 — external binding + codegen** (`analyzer` + `codegen`): external-binding
  attribute, inline-`BUILTIN` codegen; REQ-LIB-analyzer-340,
  REQ-LIB-codegen-400/410/420.
- **PR 9 — `LTRUNC`/`LMOD` as library content**: declare them in
  `compiler/library/standard/` as external aliases to the PR 7 primitives; remove
  the `--allow-extended-math-functions` flag and the analyzer/#1217 signature
  registration. This is the end-to-end proof: the functions compile *and run*.
- **PR 10+ — library content + docs**: port the rest of the Rust-baked stdlib to
  library source incrementally; user documentation.

Rough dependency order: PR 2 → (PR 3 → PR 4 → PR 5 → PR 6) and (PR 7) in
parallel → PR 8 → PR 9 → PR 10+.

## Relationship to existing work and ADRs

- **Reframes #1217.** `LTRUNC`/`LMOD` become two VM primitives plus a library
  alias (PRs 7 + 9), not a per-function flag. Recommendation: do not merge the
  flag; land them via the library.
- Builds on **[ADR-0008](../adrs/0008-unified-builtin-opcode.md)** (unified
  `BUILTIN` opcode — the primitive-extension surface),
  **[ADR-0003](../adrs/0003-plc-standard-function-blocks-as-intrinsics.md)**
  (intrinsics recognized at dispatch), and
  **[ADR-0012](../adrs/0012-accept-vendor-dialect-files-as-is.md)**.
- Related conformance mechanics:
  **[cross-crate-spec-conformance.md](cross-crate-spec-conformance.md)** and
  **[ADR-0037](../adrs/0037-mandatory-crate-slug-in-requirement-ids.md)**.

### Decisions to ratify as ADRs

1. IronPLC ships a **source** standard library compiled by the normal pipeline,
   replacing the Rust-baked stdlib.
2. Library availability is expressed by **references** (with an implicit standard
   library), not per-function `--allow-*` flags.
3. External library functions bind to `BUILTIN` **primitives** — the compiler
   grows opcodes, not functions.
4. Delivery mechanism — embed vs install-alongside (Open decision D1).
