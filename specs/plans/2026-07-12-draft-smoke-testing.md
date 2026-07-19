# Pre-publish (draft) smoke testing for the release pipeline

## Goal

Make the release smoke tests run *before* the GitHub Release is public, so a
bad build never becomes a published release. This restores a real pre-publish
gate and makes the pipeline compatible with GitHub's immutable releases (which
freeze a release's assets and metadata at publish time — only the title and
notes stay editable afterward).

## Background

The weekly deployment previously created the GitHub Release published + empty,
then attached assets and promoted it to latest. Immutable releases broke that:
attaching/promoting after publish is rejected (`target_commitish cannot be
changed when release is immutable`). Immutable releases was disabled to ship
v0.228.0; this change reworks the pipeline so the feature can be re-enabled
safely.

## Architecture

The release is now assembled as a **draft** and published exactly once, at the
end, after the smoke tests pass:

1. `partial_version.yaml` creates the release as a **draft** (mutable).
2. `partial_upload_release_artifacts.yaml` attaches all assets to the draft
   (`draft: true` keeps softprops from publishing it).
3. The smoke tests run against **this run's build artifacts** (byte-identical
   to the draft's assets) — not the public release — so they can run while the
   release is still a draft.
4. `deployment.yaml`'s `publish-release` publishes the draft
   (`draft: false, prerelease: false, makeLatest: true`) as the single publish
   step, after `publish-website` (i.e. after the smoke gate). Downstream jobs
   (Homebrew, playground) depend on `publish-release` and download from the
   now-public release, so ordering is preserved.

The enabling mechanism is a small, default-preserving override in the Unix
installer: `install.sh` builds asset URLs from `RELEASE_URL`, now overridable
via `IRONPLC_RELEASE_BASE_URL`. The smoke recipes point it at a local staging
directory served over `file://`, exercising the whole install path (arg/version
handling, checksum verify, extract, install, PATH, idempotency, real binaries
running). Only the github.com transport/redirect and "latest" API resolution
are not exercised — those inherently require a public release.

The existing job `needs` graph is left intact (smoke jobs still `need`
`upload-release-artifacts`), which guarantees the draft carries its assets
before `publish-release` runs. `stage-artifacts` is on for real runs and off
for the `dryrun-test-version-override` path (which still tests a public release).

## File Map

- `compiler/install.sh` — `RELEASE_URL` honors `IRONPLC_RELEASE_BASE_URL`
  (default unchanged; internal/testing hook, not in `--help`).
- `justfile` — `install-script-smoke` / `_install-script-smoke-run` (unix) and
  `endtoend-smoke` / `endtoend-smoke-download` (windows) take an optional
  `assets-dir`; when set they source assets locally instead of the public
  release.
- `.github/workflows/partial_install_script_test.yaml` — new `stage-artifacts`
  input; downloads `ironplcc-*` build artifacts into `smoke-assets/v<version>/`.
- `.github/workflows/partial_integration_test.yaml` — new `stage-artifacts`
  input; downloads the Windows installer + VSIX build artifacts into
  `smoke-assets/`.
- `.github/workflows/partial_version.yaml` — create the release as a draft.
- `.github/workflows/partial_upload_release_artifacts.yaml` — attach assets with
  `draft: true`.
- `.github/workflows/deployment.yaml` — pass `stage-artifacts` to the smoke
  jobs; publish the draft (`draft: false`) in `publish-release`.

## Verification

- Local: staged v0.228.0's linux asset into `smoke-assets/v0.228.0/` and ran
  `just install-script-smoke 0.228.0 smoke-assets` — install.sh fetched over
  `file://`, verified the checksum, installed, and `ironplcc version` reported
  `0.228.0` (idempotent second run passed too).
- Regression: `just install-script-smoke 0.228.0` (no override) still installs
  from the public GitHub release.
- All workflow YAML parses; `install.sh` passes `sh -n`; root `justfile` parses.
- Pipeline (next real run): release created as a draft → assets attached →
  smoke tests pass against build artifacts → draft published as latest. A smoke
  failure leaves only a deletable draft; nothing public.
- Optional: re-enable immutable releases and confirm the single-publish flow
  succeeds with no immutability error.
