# Design: Compatibility Library Testing and Distribution

> **Status: Proposal.** This document proposes an architecture change and has
> not been accepted or implemented. `REQ-*` requirement markers will be
> assigned per [cross-crate-spec-conformance](cross-crate-spec-conformance.md)
> when the design is accepted; adding them now would imply a stability this
> proposal does not yet have.

## Overview

This document proposes how compatibility libraries are **tested** and
**distributed**, replacing two aspects of the current architecture:

1. **Testing.** Library *behavior* tests move out of Rust and into IEC 61131-3
   Structured Text test programs that ship inside the library package and are
   executed by the real compiler + VM. Rust tests keep testing the
   *mechanism* (loading, activation, shadowing, round-trip fidelity); ST tests
   test the *content* (does `LTRUNC(2.8)` return `2.0`).
2. **Distribution.** The library package — the existing on-disk format from
   [Compatibility Library Format](compatibility-library-format.md), extended to
   carry its tests — becomes an independently versioned, independently licensed
   distribution artifact whose source of truth moves out of the compiler
   repository. The compiler gains the ability to load libraries from installed
   locations beyond the bundled `resources/libs`.

The two are deliberately one design: **the package is the unit of both testing
and distribution.** A library carries its own executable conformance suite, so
the same tests validate it in development, in the packaged artifact, in an
external repository against any released compiler, and — because the tests are
plain vendor-compatible ST — in the *vendor's own environment* against the
genuine implementation.

This builds on:

- [Compatibility Libraries](compatibility-libraries.md) — the behavioral design
  (activation, flat names, portability promise)
- [Compatibility Library Format](compatibility-library-format.md) — the on-disk
  package format this document extends
- [Compatibility Library Authoring policy](../steering/compatibility-library-authoring.md)
  — risk tiers and clean-room provenance
- [VM CLI](vm-cli.md) — the `ironplcvm` execution and variable-dump interface

## Problems

### Problem 1: licensing risk concentrates in the compiler

IronPLC is MIT-licensed. Compatibility libraries are authored clean-room from
public vendor documentation per the
[authoring policy](../steering/compatibility-library-authoring.md), but a
residual risk remains that cannot be engineered to zero: a claim arising from
the documentation's license terms, from interface reproduction, or from the
AI-authorship provenance question. Today that residual risk sits **inside the
compiler repository and inside every compiler release artifact**. If any single
library were ever challenged:

- every installer (tarball, NSIS, Homebrew) contains the contested content and
  would need re-release;
- the compiler repository's own history carries it;
- the doubt attaches to IronPLC's MIT license as a whole, not to one
  separable component.

And Tier C libraries (vendored third-party source such as OSCAT) are *defined*
as needing a separate distribution mechanism with its own licensing
([Compatibility Libraries §Non-Goals](compatibility-libraries.md)) — a
mechanism that does not exist yet.

### Problem 2: library behavior tests are Rust, and that does not scale

The behavioral coverage for bundled libraries lives in
`compiler/codegen/tests/it/end_to_end_tc2_math.rs` and
`end_to_end_tc2_utilities.rs`. Each test hand-rolls the pipeline in Rust:
format an ST source string, load the registry, filter shadows, analyze,
compile, run one VM round, then assert against **positional raw VM buffer
slots** (`bufs.vars[2].as_f64()`), including a hand-written STRING header
decoder for string results. Consequences:

- **Coupling.** The tests know variable slot layout, `VmBuffers` internals,
  and container string encoding. Every library author must be a compiler
  developer.
- **Scaling.** Four functions already need ~20 bespoke Rust tests. A library
  with OSCAT's surface (hundreds of POUs) is not writable this way.
- **Not shippable.** The tests compile into the compiler's test binaries. An
  externalized library cannot carry them; a user or third-party library author
  cannot run them against an installed compiler.
- **Uneven coverage.** `Tc2_System` and `Tc2_BuiltIns` have *no* end-to-end
  execution tests at all — only CLI check tests and a manually-dispatched
  installer e2e — because adding a Rust harness per library is expensive
  enough that it gets skipped.
- **Duplication.** The activate → shadow-filter → analyze → compile → run
  pipeline assembly is repeated in four places (`project`, `ironplc-cli`,
  `playground`, and each e2e test helper).

### Prior art

Testing 61131 code *in* 61131 is established practice:

- **TcUnit** (TwinCAT) — an xUnit-style framework written in ST itself: test
  suites are POUs, assertions are ST calls, the PLC runtime executes the tests.
- **CODESYS Test Manager** — scripted test cases executed against the CODESYS
  runtime.
- **Golden-output harnesses** in open-source IEC toolchains (e.g. matiec's test
  suite): a corpus of ST programs compiled and run, with results compared to
  expectations.

The proposal below is closest to TcUnit in spirit (tests are ST, executed by
the runtime) but starts with a deliberately smaller convention that requires
**zero new runtime features**, so it works on the VM that exists today.

## Design: Testing

### Tests live inside the package

The package layout from
[Compatibility Library Format](compatibility-library-format.md) gains a
`tests/` subdirectory per version:

```
Tc2_Math/
├── library.toml
└── 1.0.0/
    ├── Tc2_Math.st            # declarations (unchanged)
    └── tests/
        ├── tests.toml         # optional per-test settings
        ├── test_ltrunc.st
        ├── test_lmod.st
        ├── test_modabs.st
        └── test_frac.st
```

The declaration loader (`load_version_library`) reads `.st` files
**non-recursively** from the version directory, so `tests/` is invisible to
compilation today with no loader change — the layout is backward- and
forward-compatible.

### Test convention

Each `.st` file under `tests/` is one test case containing one `PROGRAM`:

```iecst
(* LTRUNC: integer part toward zero, staying LREAL.
   Vectors from specs/design/library-interfaces/tc2-math.md §LTRUNC. *)
PROGRAM test_ltrunc
VAR
    zero : LREAL := 0.0;
    nan_result : LREAL;
    CHECK_FRACTIONAL_POS : BOOL;
    CHECK_FRACTIONAL_NEG : BOOL;
    CHECK_INTEGRAL_UNCHANGED : BOOL;
    CHECK_BEYOND_INT_RANGE_EXACT : BOOL;
    CHECK_NAN_PROPAGATES : BOOL;
END_VAR
CHECK_FRACTIONAL_POS := LTRUNC(2.8) = 2.0;
CHECK_FRACTIONAL_NEG := LTRUNC(-2.8) = -2.0;
CHECK_INTEGRAL_UNCHANGED := LTRUNC(5.0) = 5.0;
CHECK_BEYOND_INT_RANGE_EXACT := LTRUNC(1.5E300) = 1.5E300;
nan_result := LTRUNC(zero / zero);
CHECK_NAN_PROPAGATES := nan_result <> nan_result;
END_PROGRAM
```

Rules:

1. **Assertion variables.** Every `BOOL` variable whose name begins with
   `CHECK_` is an assertion. The test **passes** when, after the final scan,
   every `CHECK_` variable is `TRUE`.
2. **Vacuous tests fail.** A test program with zero `CHECK_` variables fails.
   A test that silently asserts nothing must not read as green.
3. **Traps fail.** Any VM trap during execution fails the test (a library
   whose spec says "NaN, never a trap" is thereby pinned).
4. **Diagnostics fail.** The test source must compile cleanly with only the
   containing library activated.
5. **One scan by default.** A test needing multiple scans (timers, stateful
   FBs) declares it in `tests.toml`:

```toml
# tests/tests.toml — optional; absent file means defaults for every test
[test."test_ton_delay.st"]
scans = 10
```

The convention is intentionally *plain ST plus a naming rule*. There is no
IronPLC-specific pragma, call, or type in a test file. Approximate comparisons
are written explicitly (`ABS(actual - expected) < 1.0E-9`), which keeps the
tolerance visible in the test instead of hidden in a harness.

Failure reporting needs no new runtime machinery: the container's debug
section already carries variable names and IEC type tags (used by
`ironplcvm --dump-vars`, REQ-VC-vm-cli-008/009), so the runner reports
*which named `CHECK_` variables* were `FALSE`.

### The runner: `ironplcc test`

A new subcommand executes a package's tests:

```
ironplcc test <package-dir> [--lib-version <v>] [--filter <substring>]
```

For each test file: parse the library through the real registry loader,
activate it, merge ahead of the test program (the same order the project
pipeline uses), analyze, compile, run the declared number of scans on the VM
in-process, then evaluate the convention. Output is one line per test plus the
names of failed `CHECK_` variables; exit code `0` all passed / `1` failures /
`2` package or compile error — mirroring `ironplcvm`'s exit-code scheme.

Notes:

- The runner lives in `ironplcc` (adding an `ironplc-vm` dependency) so that
  one installed command serves compiler CI, external library repositories, and
  third-party library authors alike.
- Building it is the occasion to extract the activate → shadow-filter →
  analyze → compile pipeline assembly into one shared function, collapsing the
  four existing duplicates.
- The existing manually-dispatched installer e2e (`tests/e2e/library/`)
  becomes `ironplcc test` runs over the *installed* packages — the packaging
  check and the behavior check become the same command.

### What stays in Rust

The split is mechanism vs. content:

| Stays Rust (mechanism) | Moves to ST packages (content) |
|---|---|
| Registry/loader unit tests (manifest validation, case sensitivity, P6010/P6011) | Function/constant behavior vectors (the `end_to_end_tc2_math.rs` / `end_to_end_tc2_utilities.rs` tables) |
| `REQ-CL-*` / `REQ-LF-*` spec-conformance tests | New-library coverage (`Tc2_System`, `Tc2_BuiltIns` — currently untested end-to-end) |
| Activation/dormancy/shadowing semantic tests | Edge-case vectors as libraries grow |
| `plc2plc` round-trip fidelity tests | |
| CLI `.plcproj` discovery tests with negative controls | |
| One end-to-end smoke test that the runner's own pipeline works | |

A thin Rust integration test iterates the bundled libraries and invokes the
runner's API, so `cd compiler && just` still exercises every bundled library's
ST suite and CI stays a single command.

The clean-room specs in `specs/design/library-interfaces/` remain the source
of truth for vectors; their acceptance-criteria sections change from "pinned
by an end-to-end Rust test" to "pinned by the package's ST test suite".
Authoring a test is authoring library content: the
[authoring policy](../steering/compatibility-library-authoring.md) applies to
test files exactly as to declaration files.

### Tests double as vendor cross-validation

Because a test file is plain ST with no IronPLC constructs, the same file can
be imported into the vendor's own environment (e.g. TwinCAT) and run against
the **genuine** library. Every `CHECK_` variable `TRUE` under the vendor's
implementation confirms our expected vectors match observed vendor behavior —
validation the Rust tests could never provide, achieved without ever looking
at vendor source. This directly strengthens both the portability promise
("same behavior") and the clean-room record (expectations validated against
behavior, not implementation).

### Future: assertion library and richer reporting

Deliberately *not* in the first increment:

- **`IronPLC_Test` assertion library** (vendor `IronPLC`) — ST sugar such as
  an assertion-latching function block, if `CHECK_` expressions grow unwieldy.
  It would itself be a compatibility-format library, dogfooding the mechanism.
- **Machine-readable results** (xUnit XML from `ironplcc test`) for CI
  annotation in external repositories.
- **Expected-diagnostic tests** ("this call must fail to compile"), needed
  when the bindings mechanism introduces declare-only POUs whose *calls* are
  compile errors.
- An `__ASSERT` VM intrinsic with line-precise failure locations via the debug
  line map, if the `CHECK_` granularity proves insufficient.

## Design: Distribution

### The package is the artifact

A distributable library is the existing directory format — now including its
tests — archived as `<Name>-<version>.tar.gz` (and `.zip` for Windows), with a
`LICENSE` file at the package root:

```
Tc2_Math-1.0.0.tar.gz
└── Tc2_Math/
    ├── library.toml
    ├── LICENSE
    └── 1.0.0/
        ├── Tc2_Math.st
        └── tests/…
```

Installing is extracting the directory into a library root. The manifest gains
two fields:

| Field | Required | Meaning |
|---|---|---|
| `license` | for externally distributed packages | SPDX expression for *this package* (bundled packages default to the repository's MIT). |
| `min_compiler_version` | no | Oldest IronPLC release the package is validated against; the loader warns when older. |

Shipping the tests inside the artifact is a feature, not overhead: any user
can run `ironplcc test` on a downloaded package to verify it against *their*
installed compiler version before trusting it.

### Multi-root loading

`LibraryRegistry` grows from one root to an ordered list, first match by
(case-sensitive) name wins:

1. `--library-path <dir>` — repeatable CLI option, highest precedence
2. The user library directory — a platform-appropriate per-user data dir
   (e.g. `~/.local/share/ironplc/libs`, `%APPDATA%\ironplc\libs`)
3. The bundled root — `<bindir>/resources/libs`, unchanged

`LibraryRegistry::with_root` already exists as the seam; the change is
threading a root list through `SourceProject` instead of constructing
`LibraryRegistry::bundled()` at each call site. Activation semantics
([Compatibility Libraries §Activation channels](compatibility-libraries.md))
are untouched: roots determine where a *named* library is found, never
*whether* it is activated. The P6011 "library not found" diagnostic gains a
hint listing the searched roots and how to install a package.

The playground already loads libraries as served plain-text files and is
unaffected.

### Repository split

- **`ironplc/libraries` (new repository)** becomes the source of truth for
  Tier A/B packages. MIT-licensed, own release cadence, own git history
  carrying the clean-room provenance record. Its CI is the **compatibility
  matrix**: for each package × each supported IronPLC release, download the
  released compiler and run `ironplcc test` — possible only because tests are
  data inside the package, not Rust inside the compiler.
- **Tier C** (vendored third-party source, e.g. OSCAT) gets its own
  repository per upstream license — plausibly outside the `ironplc` org —
  using the *same* package format and the *same* runner. This is the "separate
  distribution mechanism" the compatibility-libraries design promised. The
  compiler repository and artifacts never contain Tier C content; a Tier C
  package reaches a machine only by explicit user download into roots 1–2.
- **The compiler repository keeps** the mechanism (loader, registry, runner,
  activation) and its mechanism tests — none of which reproduce any vendor
  interface.

### What the compiler release bundles

Two forces oppose: licensing isolation says *bundle nothing*; the paved-path
promise ("a real TwinCAT project compiles out of the box") says the libraries
its project file references must be present — and implicit libraries like
`Tc2_BuiltIns` *must* be, since they activate with no reference at all.

Proposed resolution, in stages:

- **Near term — bundle released packages.** The compiler release pipeline
  stops copying `sources/resources/libs` from its own tree and instead pulls
  **released package artifacts** from `ironplc/libraries` into
  `resources/libs`. Out-of-box behavior is unchanged, but every library in an
  installer is now a separable component with its own `LICENSE`, its own
  provenance history, and its own line in the SBOM. Pulling a contested
  library means dropping one artifact from the next release — no compiler code
  or history change.
- **Longer term — slim the bundle.** As the library count grows, the bundle
  shrinks toward the implicit set (`Tc2_BuiltIns`) plus whatever the paved
  path demands; reference-activated libraries move to install-on-demand, with
  P6011 telling the user exactly what to fetch. A minimal
  `ironplcc library install <name>` fetch command can arrive here — the "no
  package manager" non-goal in the current design is read as *no dependency
  resolution*, not *no download convenience* — but manual
  download-and-extract is fully supported first, and dependency resolution
  between libraries remains out of scope.

### Why this addresses the licensing concern

> **Not legal advice** — the same engineering-risk framing as the
> [authoring policy](../steering/compatibility-library-authoring.md).

- **Blast-radius isolation.** A challenge to one library is a matter for one
  package in one repository. The compiler's code, history, releases, and MIT
  license are not parties to it; remediation is "unpublish one artifact", not
  "rewrite compiler history and re-release every installer".
- **Aggregation, not derivation.** Libraries are already runtime-loaded data
  files — never `include_str!`-embedded, compiled, or linked into the binary.
  Separate repositories, separate artifacts, and per-package `LICENSE` files
  make the aggregate relationship explicit and inspectable instead of
  implicit.
- **Honest SBOM.** Each bundled package appears in the release SBOM as a
  distinct component with its own license and provenance, rather than being
  indistinguishable from compiler source.
- **A real home for Tier C.** OSCAT-class libraries become *possible* under
  their own terms without ever touching IronPLC's, closing the gap the current
  design explicitly left open.

## Increments

Each increment is independently shippable and useful without the later ones.

1. **ST test convention + runner.** Specify `tests/` + `tests.toml` in the
   format doc; implement `ironplcc test` (extracting the shared pipeline
   assembly); migrate the `Tc2_Math` / `Tc2_Utilities` Rust vector tests to
   package tests; add the missing `Tc2_System` / `Tc2_BuiltIns` suites; wire
   the bundled-library runner into `cargo test`; convert the installer e2e to
   the runner. No distribution change yet — this pays off even if
   externalization never happens.
2. **Multi-root loading.** `--library-path`, the user library directory, the
   multi-root registry, the improved P6011 hint — and reference documentation
   for `--library`, which is currently undocumented.
3. **Repository split + packaged artifacts.** Create `ironplc/libraries`,
   move Tier A/B source of truth (preserving the non-squashed clean-room spec
   history), add `license` / `min_compiler_version` to manifests, produce
   per-package release artifacts, switch the compiler release pipeline to
   pulling them, stand up the compatibility-matrix CI.
4. **Later.** `IronPLC_Test` assertion sugar, xUnit XML output,
   expected-diagnostic tests alongside the bindings mechanism, a Tier C pilot
   package, optional `library install` convenience.

## Alternatives Considered

- **Data-driven vectors (TOML/CSV tables) with a Rust harness.** Solves
  scaling but not coupling or shipping: the harness stays in the compiler,
  externalized libraries still cannot carry their tests, and vendors cannot
  execute a TOML table. It also invents a second expectation language when ST
  already is one.
- **Adopting TcUnit wholesale.** Its FB-and-reporting architecture leans on
  runtime features (rich FB support, ADS-style logging) beyond the current VM,
  and it is GPL-adjacent vendor-ecosystem code — exactly the provenance
  question this design is trying to shrink. The `CHECK_` convention is
  TcUnit-compatible in spirit and can grow toward it via `IronPLC_Test`.
- **Golden `--dump-vars` files per test.** Works today with zero new code, but
  expectations are positional, intent-free, and brittle against declaration
  reordering; a failed diff names a slot, not a claim. Named `CHECK_`
  variables keep the assertion and its meaning in one place. The dump remains
  useful as runner debug output.
- **Keeping libraries in-tree but under a different top-level directory and
  license.** Improves labeling, but the content still rides in the compiler's
  repository history and every release artifact — the blast radius does not
  shrink.
- **A full package manager with remote fetch and dependency resolution.**
  Explicitly rejected by the existing design's non-goals; nothing here needs
  it. Distribution is "download a directory"; the only future convenience
  contemplated is a single-package fetch command.

## Open Questions

- Exact user library directory per platform (follow existing IronPLC config
  conventions vs. `directories`-crate defaults).
- Package integrity: checksums are cheap (the release pipeline already emits
  SHA256s for Homebrew); whether to add signatures, and when.
- Version-compatibility policy: is `min_compiler_version` a warning or an
  error, and does the compatibility matrix publish which pairs are validated?
- Whether the implicit-library set can ever be externalized, or is permanently
  part of the compiler's paved path.
- Whether `ironplcc test` should also run a package's declarations through
  `plc2plc` round-trip as a structural lint on the package itself.

## References

- [Compatibility Libraries](compatibility-libraries.md)
- [Compatibility Library Format](compatibility-library-format.md)
- [Compatibility Library Authoring policy](../steering/compatibility-library-authoring.md)
- [VM CLI and Variable Dump](vm-cli.md)
- [Cross-crate spec conformance](cross-crate-spec-conformance.md)
- [TcUnit — unit testing framework for TwinCAT, written in ST](https://tcunit.org/)
- [CODESYS Test Manager](https://store.codesys.com/en/codesys-test-manager.html)
- Clean-room behavior specs: [tc2-math](library-interfaces/tc2-math.md),
  [tc2-utilities](library-interfaces/tc2-utilities.md),
  [tc2-builtins](library-interfaces/tc2-builtins.md)
