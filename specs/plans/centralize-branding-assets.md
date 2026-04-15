# Plan: Centralize Branding Assets

## Goal

Create a single `assets/` directory at the repository root containing all branding
images. Each consumer (website, Windows installer, VS Code extension) references
the centralized files directly so that updating branding requires changing files in
only one place.

## Current State

Branding images are duplicated across three locations:

| Location | Files | Consumer |
|---|---|---|
| `docs/_static/` | favicon.ico, favicon.svg, apple-touch-icon.png, ironplc-banner.png/svg, ironplc-inline.png/svg, ironplc-square.png/svg, ironplc-icon-32.svg, ironplc-icon-48.svg | Sphinx website |
| `docs/images/` | banner.svg | Sphinx website (unreferenced but branding) |
| `compiler/nsis/assets/` | logo.ico, banner.bmp/png/svg, finished-banner.bmp/png/svg | NSIS Windows installer |
| `integrations/vscode/images/` | logo.png, logo.svg | VS Code extension |

## Approach

1. Create `assets/` at the repo root
2. Move all branding files into `assets/`, using prefixed names to avoid collisions
3. Update all references to point directly to the centralized files
4. For VS Code (which requires assets inside the extension directory for packaging),
   add a `copy-assets` build step that copies from `assets/` before packaging

### Naming Convention in `assets/`

- Website files keep their existing names (already prefixed with `ironplc-`)
- NSIS files get `nsis-` prefix
- VS Code files get `vscode-` prefix

### Reference Updates

| Consumer | How it references `assets/` |
|---|---|
| Sphinx website | `index.rst` image directive uses `../assets/ironplc-banner.svg` directly |
| NSIS installer | `setup.nsi` paths changed to `..\assets\nsis-*` |
| VS Code extension | `copy-assets` justfile recipe copies to local `assets/` dir; `package.json` references `assets/vscode-logo.png` |

### File Mapping

| Source | Target in `assets/` |
|---|---|
| `docs/_static/favicon.ico` | `assets/favicon.ico` |
| `docs/_static/favicon.svg` | `assets/favicon.svg` |
| `docs/_static/apple-touch-icon.png` | `assets/apple-touch-icon.png` |
| `docs/_static/ironplc-banner.png` | `assets/ironplc-banner.png` |
| `docs/_static/ironplc-banner.svg` | `assets/ironplc-banner.svg` |
| `docs/_static/ironplc-inline.png` | `assets/ironplc-inline.png` |
| `docs/_static/ironplc-inline.svg` | `assets/ironplc-inline.svg` |
| `docs/_static/ironplc-square.png` | `assets/ironplc-square.png` |
| `docs/_static/ironplc-square.svg` | `assets/ironplc-square.svg` |
| `docs/_static/ironplc-icon-32.svg` | `assets/ironplc-icon-32.svg` |
| `docs/_static/ironplc-icon-48.svg` | `assets/ironplc-icon-48.svg` |
| `docs/images/banner.svg` | `assets/docs-banner.svg` |
| `compiler/nsis/assets/logo.ico` | `assets/nsis-logo.ico` |
| `compiler/nsis/assets/banner.bmp` | `assets/nsis-banner.bmp` |
| `compiler/nsis/assets/banner.png` | `assets/nsis-banner.png` |
| `compiler/nsis/assets/banner.svg` | `assets/nsis-banner.svg` |
| `compiler/nsis/assets/finished-banner.bmp` | `assets/nsis-finished-banner.bmp` |
| `compiler/nsis/assets/finished-banner.png` | `assets/nsis-finished-banner.png` |
| `compiler/nsis/assets/finished-banner.svg` | `assets/nsis-finished-banner.svg` |
| `integrations/vscode/images/logo.png` | `assets/vscode-logo.png` |
| `integrations/vscode/images/logo.svg` | `assets/vscode-logo.svg` |

## Steps

1. Create `assets/` directory
2. Move files to `assets/` with new names
3. Remove original directories (`docs/images/`, `compiler/nsis/assets/`, `integrations/vscode/images/`)
4. Update `docs/index.rst` image directive to reference `../assets/ironplc-banner.svg`
5. Update `compiler/setup.nsi` to reference `../assets/nsis-*`
6. Update `integrations/vscode/package.json` to reference `assets/vscode-logo.png`
7. Add `copy-assets` recipe to `integrations/vscode/justfile`
8. Add `assets/` to `integrations/vscode/.gitignore` (it's a build artifact there)
9. Run CI to confirm nothing breaks
