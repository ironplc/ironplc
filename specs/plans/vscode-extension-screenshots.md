# Design: VS Code Extension Screenshots for IronPLC Documentation

## Context

IronPLC's documentation describes the VS Code extension (syntax highlighting, diagnostics, settings, bytecode viewer) entirely in prose. Pages like `docs/reference/editor/overview.rst` and `docs/reference/editor/settings.rst` would be significantly clearer with screenshots, and the quickstart installation flow would benefit from visual confirmation of each step.

Manually captured screenshots drift: the extension UI changes, VS Code itself evolves, themes shift, and stale images hurt docs more than none at all. The user wants automated capture so images stay current with minimal maintenance.

**Simplification:** The capture script is designed to run on a developer Mac (where `ironplcc` is already installed) rather than in CI. This eliminates Xvfb handling, cross-platform concerns, and the need to coordinate the compiler binary with CI. The script produces PNGs that get committed to the repo; re-running the recipe on demand keeps them fresh.

## Goals

1. Automated capture of VS Code extension screenshots via a single `just` recipe runnable on macOS.
2. Cover the feature documentation that actually changes: syntax highlighting, diagnostics, settings panel, bytecode viewer.
3. Produce images consistent enough to drop into Sphinx docs without post-processing.
4. Keep the conventions open to Linux/Windows capture later if needed (but don't build for that now).

## Non-Goals

- CI integration. Screenshots are regenerated on demand and committed.
- Cross-platform support for the capture script. macOS only, initially.
- Installation/marketplace screenshots. These require the real VS Code marketplace UI and are captured manually (rare-change, one-shot).
- Visual regression testing (pixel-diffing to catch UI changes).

## Approach: Playwright + Electron on macOS

Playwright's `_electron.launch()` drives VS Code (an Electron app) directly and exposes `page.screenshot()`. Combined with `@vscode/test-electron`'s existing `downloadAndUnzipVSCode()` (already a project dependency), this gives a self-contained capture pipeline.

Why Playwright over alternatives:
- `@vscode/test-electron` alone has no screenshot API — it encapsulates the Electron process.
- Playwright has stable selectors for Monaco editor classes and is the de facto standard for Electron automation.
- Running natively on macOS avoids Xvfb rendering fidelity concerns.

Because `ironplcc` is installed on the dev machine, the extension's LSP spins up normally, and diagnostics (red/yellow squiggles with problem codes) can be captured alongside static UI.

## Script Layout

```
integrations/vscode/src/screenshots/
    captureScreenshots.ts   # Per-scenario capture functions
    run.ts                  # CLI entry point
    settings.json           # VS Code settings for deterministic rendering
    fixtures/
        valid.st            # Reuses src/test/functional/resources/valid.st
        invalid.st          # Intentional errors to trigger diagnostics
        sample.iplc         # Pre-compiled bytecode for viewer screenshot
```

`run.ts`:
1. Resolve output directory from CLI arg (default: `../../docs/reference/editor/images/`).
2. Call `downloadAndUnzipVSCode()` to get VS Code binary path (pinned version).
3. Create temp user-data-dir and seed it with `settings.json`.
4. Verify `ironplcc` is on PATH (or at the configured `ironplc.path`); warn if missing so diagnostics scenarios are skipped cleanly.
5. Run each capture scenario in `captureScreenshots.ts`.
6. Clean up temp dirs and exit.

`captureScreenshots.ts` (one function per scenario):
1. Launch VS Code via `electron.launch()` with `--extensionDevelopmentPath`, `--disable-extensions`, `--user-data-dir`, `--locale=en`, and the target file.
2. Wait for `.monaco-editor` selector to confirm editor readiness. For diagnostics, additionally wait for a diagnostic marker selector to ensure the LSP has reported.
3. Navigate UI as needed (e.g., `Meta+,` on macOS to open settings, type "ironplc" to filter).
4. Call `page.locator(...).screenshot()` for element-scoped screenshots where possible, falling back to `page.screenshot({ clip })` for full-window shots.
5. Close the Electron app.

`settings.json`:
```json
{
  "workbench.colorTheme": "Default Light Modern",
  "workbench.startupEditor": "none",
  "telemetry.telemetryLevel": "off",
  "update.mode": "none",
  "extensions.autoUpdate": false,
  "window.restoreWindows": "none"
}
```

## Scenarios Captured

| Scenario | Output | Notes |
|---|---|---|
| Syntax highlighting | `syntax-highlighting.png` | Opens `valid.st`, screenshots editor |
| Diagnostics squiggles | `diagnostics-squiggles.png` | Opens `invalid.st`, waits for LSP, screenshots editor with squiggles and hover |
| Settings panel | `settings-panel.png` | `Meta+,` + filter "ironplc" |
| Bytecode viewer | `bytecode-viewer.png` | Opens `sample.iplc` via the custom editor |

Manual-only (not in script):
- `installation-marketplace.png`
- `installation-installed.png`
- `first-file-new-st.png`

## Capture Conventions

These apply to both automated and the few remaining manual captures so the visual language is consistent:

- **Theme:** VS Code "Default Light Modern" (matches Furo docs theme).
- **Window size:** 1200×800 logical pixels.
- **Locale:** `en`.
- **Chrome:** Activity bar visible, side bar hidden unless the screenshot is about the side bar, status bar visible, panel hidden.
- **Format:** PNG, no alpha.
- **Sample code:** `valid.st` reused from `integrations/vscode/src/test/functional/resources/` so screenshots, functional tests, and grammar snapshots all show the same code.

## Image Storage

Screenshots live next to the docs that reference them, matching the pattern in `docs/quickstart/sense-control-actuate.rst`:

```
docs/reference/editor/images/
    syntax-highlighting.png
    diagnostics-squiggles.png
    settings-panel.png
    bytecode-viewer.png
docs/quickstart/images/
    installation-marketplace.png
    installation-installed.png
    first-file-new-st.png
```

## RST Integration

Use Sphinx's `figure` directive with alt text, captions, and fixed widths. Example addition to `docs/reference/editor/overview.rst`:

```rst
.. figure:: images/syntax-highlighting.png
   :alt: VS Code showing an IEC 61131-3 Structured Text file with syntax highlighting
   :width: 600px

   Structured Text with syntax highlighting in VS Code.
```

Similar additions for `diagnostics-squiggles.png`, `bytecode-viewer.png` in the same file; `settings-panel.png` in `docs/reference/editor/settings.rst`; installation screenshots in `docs/quickstart/installation.rst`.

## Build Integration

- Add `"playwright": "^1.44.0"` to `integrations/vscode/package.json` devDependencies. No browser download (Electron mode only).
- Add a `screenshots` recipe to `integrations/vscode/justfile`. macOS-only initially:
  ```
  screenshots:
    #!/usr/bin/env bash
    if [[ "$(uname)" != "Darwin" ]]; then
      echo "screenshots recipe currently supports macOS only" >&2
      exit 1
    fi
    node ./out/screenshots/run.js
  ```
- The recipe depends on `compile` having been run (existing TypeScript compile step emits to `out/`).
- Output PNGs are committed to the repo. The recipe is run on demand when the UI changes — same pattern as `update-grammar-snapshots` in the existing justfile.

## Risks

- **VS Code version drift:** Pin the version passed to `downloadAndUnzipVSCode()` so UI changes don't show up unexpectedly in captures.
- **Monaco selector stability:** `.monaco-editor` and related classes are internal but have been stable for years. If they change, the script fails loudly rather than producing silently-broken images.
- **LSP timing for diagnostics:** The script must wait for the LSP to publish diagnostics before screenshotting. Use a selector-based wait for the diagnostic marker/squiggle element with a reasonable timeout.
- **macOS-only:** If another contributor on Linux/Windows wants to regenerate screenshots, they can't. This is a deliberate tradeoff for simplicity; cross-platform support can be added later if it becomes a real constraint.

## Key Files

- `integrations/vscode/package.json` — add playwright dev dep
- `integrations/vscode/src/screenshots/captureScreenshots.ts` — new
- `integrations/vscode/src/screenshots/run.ts` — new
- `integrations/vscode/src/screenshots/settings.json` — new
- `integrations/vscode/src/screenshots/fixtures/invalid.st` — new (bad ST to trigger diagnostics)
- `integrations/vscode/src/screenshots/fixtures/sample.iplc` — new (pre-compiled bytecode)
- `integrations/vscode/justfile` — add `screenshots` recipe
- `integrations/vscode/src/test/functional/resources/valid.st` — reused as canonical sample, no change
- `integrations/vscode/src/test/functional/runTest.ts` — reference for `runTests()` / `downloadAndUnzipVSCode()` usage
- `docs/reference/editor/overview.rst` — add figure directives
- `docs/reference/editor/settings.rst` — add figure directive
- `docs/quickstart/installation.rst` — add manual installation figure directives
- `docs/reference/editor/images/` — new directory (committed PNGs)
- `docs/quickstart/images/` — new directory (committed PNGs)

## Verification

1. `cd integrations/vscode && npm install` — Playwright installs.
2. `cd integrations/vscode && just compile && just screenshots` — script runs, PNGs appear in `docs/reference/editor/images/`.
3. Inspect each PNG: correct theme, size, content; diagnostics PNG shows squiggles.
4. `cd docs && just compile` — Sphinx builds without errors; open `_build/reference/editor/overview.html` and confirm images render at correct size with captions and alt text.
5. Commit the generated PNGs; future re-runs produce minimal diffs (pixel-level only) unless the UI genuinely changed.
