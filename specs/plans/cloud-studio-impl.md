# Implementation Plan: Cloud Studio

**Design:** [Cloud Studio Design](../design/cloud-studio.md)

## Status

### Completed

The following components are fully implemented:

1. **Rust crate `compiler/web-app/`** — wasm-bindgen exports (`compile`, `run`, `run_source`) with full test coverage (9 tests)
2. **Web frontend `web/`** — `index.html`, `app.js`, `style.css` with code editor, compile/run UI, and output panel
3. **Build tooling `web/justfile`** — `setup`, `compile`, `ci`, `serve`, `clean` recipes
4. **CI workflow `.github/workflows/partial_web_app.yaml`** — builds wasm + web app, uploads artifact
5. **E2E workflow `.github/workflows/partial_web_app_e2e.yaml`** — Playwright tests against build artifact
6. **E2E tests `web/tests/e2e.spec.js`** — page load, compile/run, error display tests
7. **Deployment pipeline `.github/workflows/deployment.yaml`** — `build-web-app`, `web-app-e2e-test`, and `publish-web-app` jobs integrated into the release pipeline
8. **Workspace integration** — `web-app` is a member of `compiler/Cargo.toml` workspace

### Remaining

The following items require manual action outside this repository:

1. **Create GitHub repository `ironplc/ironplc-cloudstudio`** — empty repo with GitHub Pages enabled on the `main` branch
2. **Configure DNS** — add CNAME record `cloudstudio.ironplc.com` → `ironplc.github.io` at the domain registrar
3. **Verify `IRONPLC_WORKFLOW_PUBLISH_ACCESS_TOKEN`** — the existing personal access token (used for homebrew tap publishing) must have write access to the new `ironplc/ironplc-cloudstudio` repository

## File Inventory

### Files in `compiler/web-app/`

| File | Description |
|---|---|
| `Cargo.toml` | Crate definition — `cdylib` + `rlib`, depends on parser, codegen, container, vm, wasm-bindgen, serde, base64 |
| `src/lib.rs` | Three `#[wasm_bindgen]` exports + JSON result types + 9 unit tests |

### Files in `web/`

| File | Description |
|---|---|
| `index.html` | Single-page app with code editor and output panel |
| `app.js` | Loads wasm module, wires UI to wasm exports, renders results |
| `style.css` | Layout and styling |
| `justfile` | Build recipes: `setup`, `compile`, `ci`, `serve`, `clean` |
| `package.json` | Node dependencies (Playwright) |
| `playwright.config.js` | Playwright configuration |
| `tests/e2e.spec.js` | E2E test suite |

### CI/CD Files

| File | Description |
|---|---|
| `.github/workflows/partial_web_app.yaml` | Reusable workflow: build wasm + upload artifact |
| `.github/workflows/partial_web_app_e2e.yaml` | Reusable workflow: Playwright E2E tests |
| `.github/workflows/deployment.yaml` | Main pipeline — includes `build-web-app`, `web-app-e2e-test`, `publish-web-app` jobs; publishes to `ironplc/ironplc-cloudstudio` |

## Verification

1. **Rust tests**: `cd compiler && cargo test -p ironplc-web-app`
2. **Local build**: `cd web && just ci` (requires `wasm-pack` installed)
3. **Local serve**: `cd web && just serve` → open http://localhost:8080
4. **E2E tests**: `cd web && npx playwright test` (requires build artifact in `web/_build/`)
5. **Full CI**: `cd compiler && just` — existing tests + coverage pass
