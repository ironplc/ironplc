# Parallelize installer builds and cross-platform tests in CI/release

## Goal

Cut the wall-clock time of the installer build stage
(`.github/workflows/partial_compiler.yaml`, used by CI, update, and the
release/deployment workflows) by running the cross-platform test suite **in
parallel** with the release/installer builds instead of serially inside them —
without changing which tests run or on which platforms they run.

## Problem

The 2026-08-02 plan split the quality gate (`just ci`) out of the package
matrix so coverage/lint run concurrently with packaging. Two serial chains
remain in the installer path:

1. **Tests serialize with packaging inside every matrix leg.** Each of the 5
   `compiler-package` legs runs `just test` (a debug build plus the full test
   suite) and only then `just package` (the release build + installer). Per
   leg the wall-clock is `test_time + package_time`.

   The test runs are also redundant: `cargo test` always builds and runs for
   the *host*, never for `matrix.rust_target`. The two Windows legs share the
   same `windows-2025` host image and the two macOS legs share the same
   `macos-latest` image, so the identical host suite executes 5× across only
   **3 unique hosts** (Windows x86_64, Linux x86_64, macOS arm64).

2. **The SBOM job is an uncached serial prefix.** All 5 package legs `needs:`
   `compiler-sbom`, which runs `cargo install --locked cargo-cyclonedx` from
   source on every run with no cache. Every minute spent there delays every
   installer leg.

## Approach

### Split tests into their own parallel job

Add a **`compiler-test`** job with a 3-leg matrix over the unique host images
(`windows-2025`, `ubuntu-latest`, `macos-latest`) that runs `just test`. It
has no `needs`, so it starts immediately and runs concurrently with
`compiler-sbom`, `compiler-quality`, and `compiler-package`.

Test granularity is unchanged: the same `just test` command runs on the same
set of unique (host OS, toolchain) combinations that execute today — the split
removes only the two byte-for-byte duplicate runs (second Windows leg, second
macOS leg). The job needs no cross-compile `target:`, no SBOM, and no NSIS.

**`compiler-package`** drops its `Run tests` step and becomes build-only:
download SBOM, `just package`, upload artifacts. Per leg the wall-clock drops
from `test + package` to `package`, and the stage's wall-clock from roughly
`sbom + test + package` to `max(test, sbom + package)`.

### Verify the shipped bytes in each package leg

Moving `just test` out of the package legs surfaced that it never tested the
shipped code anyway: `cargo test` is a debug-profile build for the host, while
the installer packs a release-profile build for `matrix.rust_target`. To test
what we actually ship, add a `verify-package` recipe that runs the **packaged
release binaries** from `target/<target>/release`: `ironplcc version`, then
compile and execute the bundled-library fixture (`uses_pi.st`) on the shipped
compiler + VM, asserting the `2 * PI * 10.0` result (mirroring
`library-e2e`). This exercises the shipped compiler, VM, and staged
compatibility libraries together — coverage the old in-job test run never
provided.

The recipe runs on the 4 legs whose target the build host can execute
(`verify: true` in the matrix): x86_64 Windows, x86_64 Linux, and both macOS
targets (the x86_64 macOS build runs on the arm64 runner via Rosetta 2, which
the recipe installs if absent). The aarch64 Windows cross-build cannot run on
its x86_64 host and remains covered by the post-release smoke tests. On
Windows the release directory lacks `resources/libs` (the NSIS installer
copies `sources\resources\libs` at install time), so the recipe performs the
same copy `setup.nsi` does before running the check.

### Cache the SBOM tooling

Add `Swatinem/rust-cache` to `compiler-sbom` (it caches `~/.cargo/bin`, so
`cargo install --locked cargo-cyclonedx` becomes a near no-op on a warm
cache), shrinking the serial prefix ahead of all 5 installer legs.

### Correctness / gating

- The reusable workflow succeeds only if **all** jobs pass, so existing gates
  are preserved: in `deployment.yaml`, `upload-release-artifacts` and the
  publish steps still block on the tests passing (they `needs:` the caller
  job wrapping this partial).
- No caller (`deployment.yaml`, `integration.yaml`, `update.yaml`) references
  the partial's internal job names or any output; the partial exposes no
  `outputs`. Verified by inspection.
- Artifact names are unchanged: `ironplc-compiler-sbom` and the per-target
  installer + `.sha256` names.

### Accepted trade-off

As with the quality split, `compiler-package` now builds (and uploads build
artifacts) even when tests are failing. Nothing publishes: release attachment
happens in `upload-release-artifacts`, which still gates on the whole partial
passing. Wasted compute on failure is the inherent cost of the parallelism.

### Rejected alternative

Generating the SBOM inside each package leg would remove the `needs:` edge
entirely, but duplicates the SBOM work 5× and requires the CycloneDX toolchain
on all three OSes. Caching the tool install keeps one SBOM of record and
shrinks the same serial edge.

## File map

- `.github/workflows/partial_compiler.yaml` — add `compiler-test` (3-host
  matrix, `just test`); remove the test step from `compiler-package`; add a
  `Verify packaged release binaries` step gated by `matrix.verify`; add
  `rust-cache` to `compiler-sbom`.
- `compiler/justfile` — add `verify-package` and per-OS implementations.

## Tasks

- [x] Add `compiler-test` job (3 unique hosts, `just test`, no `needs`).
- [x] Remove `Run tests` from `compiler-package`; update comments that said
      cross-platform tests run there.
- [x] Add `verify-package` recipe and run it on the 4 host-runnable legs
      after `just package`.
- [x] Add `Swatinem/rust-cache` to `compiler-sbom`.
- [x] Verify workflow YAML parses and callers are unaffected.
