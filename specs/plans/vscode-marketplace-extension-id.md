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

- **Open VSX** keeps `ironplc.ironplc` and `displayName` "IronPLC" (unchanged) —
  protects existing users.
- **Marketplace** publishes as `ironplc.ironplc-vscode` with `displayName`
  "IronPLC IDE" — same publisher (`ironplc`); the machine `name` differs because
  `ironplc.ironplc` is retired, and the `displayName` differs because the
  Marketplace still reserves the bare "IronPLC" display name from the removed
  listing and rejects a new upload with it as already in use. Adding a neutral
  qualifier ("IDE" — deliberately not tied to Structured Text or IEC 61131-3, so
  the name still fits if other languages/standards are added later) follows the
  common `<Brand> <Descriptor>` convention used by peer PLC extensions on the
  Marketplace. The differing internal ID is invisible in the store UI.

Only the `name` and `displayName` fields change for the Marketplace build.
Command IDs, language IDs, and activation events are literal strings unaffected
by `name`.

## Rollout

This plan was delivered in stages so the new listing could be validated by hand
before automated publishing was switched on. All stages are now complete.

### Stage 1 — enable local build/test (done)

1. **Decouple runtime code from the literal ID.**
   `src/extension.ts` looked itself up via
   `vscode.extensions.getExtension('ironplc.ironplc')` to read its version. That
   returns `undefined` under the Marketplace ID. Capture the version from the
   `ExtensionContext` at activation instead, so it is ID-agnostic. This is a
   behavior-preserving change for the existing `ironplc.ironplc` build and is a
   prerequisite for the new ID to report its version correctly.

2. **Add a `package-marketplace` justfile recipe** that overrides `name` to
   `ironplc-vscode` and `displayName` to "IronPLC IDE", then packages a VSIX with
   extension ID `ironplc.ironplc-vscode`. Run `just package-marketplace
   <file>.vsix` locally to build a VSIX for manual install (`code
   --install-extension <file>.vsix`) or a manual `vsce publish` test. The Open
   VSX / GitHub-release VSIX is unchanged (`name: ironplc`, `displayName:
   IronPLC`).

### Stage 1.5 — package and upload the Marketplace VSIX in CI, no publish (done)

As a stepping stone toward Stage 2, `partial_vscode_extension.yaml` now runs
`just package-marketplace` in the credential-free build job to prove that the
Marketplace build (extension ID `ironplc.ironplc-vscode`) packages cleanly in
CI. Packaging runs unconditionally; the resulting VSIX is uploaded as a separate
build artifact when the optional `marketplace-artifact-name` input is set (the
`deployment.yaml` and `integration.yaml` callers set it), so the VSIX can be
downloaded and installed for **manual** validation. The consolidated
`upload-release-artifacts` job attaches it to the GitHub Release so users can
install the Marketplace build (extension ID `ironplc.ironplc-vscode`) manually.
At this stage it was **not** automatically published to the Marketplace; Stage 2
added that. The packaging step runs after the Open VSX VSIX and the SBOM so its
`package.json` `name` override does not affect those artifacts.

### Stage 2 — automate Marketplace publishing (this change)

The new listing has been validated by hand, so publishing is now automated.

3. **Publish from `deployment.yaml`.** Building and uploading the Marketplace
   VSIX already happens in Stage 1.5. `publish-release` now downloads that
   artifact (`ironplc-vscode-extension-marketplace.vsix`) alongside the Open VSX
   one and publishes it with `vsce` using `VS_MARKETPLACE_TOKEN`. Each registry
   is published from its own VSIX because the extension ID is baked into the
   manifest at package time. Open VSX publishing is unchanged.
   - The smoke test now installs the Marketplace VSIX and sets
     `ironplc-vscode-extension-name` to its real extension ID,
     `ironplc.ironplc-vscode`. It previously said `garretfick.ironplc`, a
     leftover from the pre-`ironplc` publisher that matched neither VSIX. The
     Marketplace build is the one exercised end to end because it is the channel
     most users install from and the one whose ID is new; the two VSIXs are
     otherwise byte-identical apart from `name` and `displayName`.
   - The smoke test sideloads the VSIX into
     `.vscode/extensions/<extension-id>-<version>` and writes a matching
     `extensions.json`, so this input must track whichever VSIX the job
     downloads.

4. **Docs.** Delivered separately in #1390, which landed first.
   `docs/quickstart/installation.rst`, `integrations/vscode/README.md`, and
   `docs/how-to-guides/troubleshoot-editor.rst` describe both listings —
   "IronPLC IDE" (`ironplc.ironplc-vscode`) on the Marketplace and "IronPLC"
   (`ironplc.ironplc`) on Open VSX — and make installing from the Extensions
   view the primary path, keeping the VSIX download as the fallback for
   environments that cannot reach either registry.

   Known gap: the docs describe a single release VSIX, but two are attached
   (`ironplc-vscode-extension-marketplace.vsix` matches the Marketplace listing,
   `ironplc-vscode-extension.vsix` matches Open VSX). A VS Code user who
   sideloads the Open VSX VSIX will not receive Marketplace updates. Worth a
   follow-up if manual installation on VS Code turns out to be common.
