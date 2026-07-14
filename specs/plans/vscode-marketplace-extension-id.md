# VS Code Marketplace Extension ID Plan

This document describes how IronPLC publishes its VS Code extension to two
independent registries — the Visual Studio Marketplace and Open VSX — using a
different extension ID on each.

## Problem

Publishing access to the Visual Studio Marketplace has been restored, but the
original extension ID `ironplc.ironplc` can no longer be reused there (the
publisher was blocked and the extension removed; the ID is retired). Open VSX,
however, still hosts a live `ironplc.ironplc` with an existing install base.

A VS Code extension ID is `<publisher>.<name>`, and that value is baked into the
VSIX manifest at package time, so a single VSIX carries a single ID. The two
registries are independent namespaces, so the ID does **not** need to match
across them.

## Decision

- **Open VSX** keeps `ironplc.ironplc` (unchanged) — protects existing users.
- **Marketplace** publishes as `ironplc.ironplc-vscode` — same publisher and
  same `displayName` ("IronPLC"), only the machine `name` differs. The differing
  internal ID is invisible in the store UI.

Only the `name` field changes for the Marketplace build. Command IDs, language
IDs, and activation events are literal strings unaffected by `name`.

## Rollout

Automated Marketplace publishing is **deferred** until the new listing has been
manually tested. This plan is delivered in two stages.

### Stage 1 — enable local build/test (this change)

1. **Decouple runtime code from the literal ID.**
   `src/extension.ts` looked itself up via
   `vscode.extensions.getExtension('ironplc.ironplc')` to read its version. That
   returns `undefined` under the Marketplace ID. Capture the version from the
   `ExtensionContext` at activation instead, so it is ID-agnostic. This is a
   behavior-preserving change for the existing `ironplc.ironplc` build and is a
   prerequisite for the new ID to report its version correctly.

2. **Add a `package-marketplace` justfile recipe** that overrides `name` to
   `ironplc-vscode` and packages a VSIX with extension ID `ironplc.ironplc-vscode`.
   Run `just package-marketplace <file>.vsix` locally to build a VSIX for manual
   install (`code --install-extension <file>.vsix`) or a manual `vsce publish`
   test. The Open VSX / GitHub-release VSIX is unchanged (`name: ironplc`).

### Stage 1.5 — package the Marketplace VSIX in CI, build-only (this change)

As a stepping stone toward Stage 2, `partial_vscode_extension.yaml` now runs
`just package-marketplace` in the credential-free build job to prove that the
Marketplace build (extension ID `ironplc.ironplc-vscode`) packages cleanly in
CI. The resulting VSIX is intentionally **not** uploaded as a build artifact and
**not** published — this step only validates that packaging works. It runs after
the Open VSX VSIX and the SBOM so its `package.json` `name` override does not
affect those artifacts.

### Stage 2 — automate Marketplace publishing (deferred)

Once the new listing has been validated by hand:

3. **Upload the Marketplace VSIX from CI.** Building already happens in
   Stage 1.5. Add an optional `marketplace-artifact-name` input to
   `partial_vscode_extension.yaml` that uploads the packaged Marketplace VSIX as
   a separate build artifact, in the credential-free build job (preserving ADR
   0032's artifact producer/consumer split).

4. **Publish from `deployment.yaml`.** Request the Marketplace artifact from the
   build job, download it in `publish-release`, and publish with `vsce` using
   `VS_MARKETPLACE_TOKEN`. Open VSX publishing is unchanged.
   - The smoke-test job's `ironplc-vscode-extension-name` input must be set to
     the ID the release actually installs (`ironplc.ironplc-vscode`).

5. **Docs.** Update `docs/quickstart/installation.rst` and
   `integrations/vscode/README.md` to reflect Marketplace availability under the
   new ID, leaving the Open VSX `ironplc.ironplc` link intact.
