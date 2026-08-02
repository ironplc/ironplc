# Parallelize the compiler quality and package builds in CI/release

## Goal

Cut the wall-clock time of the compiler build stage (used by both CI and the
release/deployment workflows) by running the coverage/quality build and the
release/installer build **in parallel** instead of serially.

## Problem

`.github/workflows/partial_compiler.yaml` has a single matrix job (`compiler`,
5 target legs) that runs two independent builds one after the other:

1. `just ci` — `compile` (debug build) + `coverage` (a second,
   instrumentation-flagged build that runs all tests, gated at 85% lines) +
   `lint` + `dupes`.
2. `just package` — a third build (`cargo build --release --target <triple>`)
   that produces the actual installer.

Per leg the wall-clock is `ci_time + package_time`. Two facts make splitting
these clean:

- **Nothing consumes the coverage output.** `lcov.info` is never uploaded; the
  `coverage` recipe is purely a pass/fail gate. There is no artifact to reuse
  between the two builds (a coverage-instrumented binary is unusable for
  release anyway).
- **`just ci` is run redundantly today.** `just ci` builds/tests on the *host*
  and never uses `matrix.rust_target` (only `just package` cross-compiles). The
  two Windows legs share one x86_64 Windows host and the two macOS legs share
  one arm64 host, so `just ci` runs 5× across only **3 unique hosts**.

## Approach

Split the single `compiler` job into two jobs that do not depend on each other,
so they run concurrently:

- **`compiler-quality`** — runs `just ci` on a 3-host matrix
  (`ubuntu-latest`, `windows-2025`, `macos-latest`): exactly the host arch/OS
  combinations `just ci` executes on today, deduplicated. Installs only what
  `just ci` needs (rustfmt, clippy, cargo-llvm-cov, cargo-dupes). No SBOM
  download, no cross-compile target, no NSIS.
- **`compiler-package`** — keeps the existing 5-target matrix, the
  `compiler-sbom` dependency, the SBOM download, and the NSIS install. Runs
  only `just package` and uploads the installer + sha256 artifacts. Drops the
  clippy/rustfmt/cargo-llvm-cov/cargo-dupes setup it no longer needs.

Per stage the wall-clock drops from `ci + package` to roughly
`max(ci, package)`.

### Correctness / gating

- The reusable workflow (partial) succeeds only if **both** jobs pass, so the
  existing gates are preserved: `upload-release-artifacts` and the release
  publish steps in `deployment.yaml` still block on coverage + lint passing.
- No caller (`deployment.yaml`, `integration.yaml`, `update.yaml`) references
  the partial's internal job names or any output; they gate on the caller-level
  `uses:` job. The partial exposes no `outputs`.
- Artifact names are unchanged: `ironplc-compiler-sbom`, and the per-target
  installer + `.sha256` names.

### Accepted trade-off

Because the two jobs are now independent, `just package` runs even when
`just ci` is failing (it will simply never publish, since the partial still
fails overall). This wasted-compute-on-failure is the inherent cost of the
parallelism.

## File map

- `.github/workflows/partial_compiler.yaml` — split `compiler` into
  `compiler-quality` and `compiler-package`.

## Tasks

- [ ] Add `compiler-quality` job (3-host matrix, `just ci`, no SBOM/target/NSIS).
- [ ] Rework `compiler` → `compiler-package` (5-target matrix, `just package`
      only, drop lint/coverage tool installs).
- [ ] Verify workflow YAML parses and callers are unaffected.
