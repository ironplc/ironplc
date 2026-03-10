# Implementation Plan: Playground

**Design:** [Playground Design](../design/playground.md)

## Status

### Completed

The following components are fully implemented:

1. **Rust crate `compiler/playground/`** — wasm-bindgen exports (`compile`, `run`, `run_source`, `init_panic_hook`) with full test coverage
2. **Web frontend `playground/`** — `index.html`, `app.js`, `worker.js`, `style.css` with code editor, compile/run UI, and output panel
3. **Web Worker execution** — all WASM compilation and execution runs off the main thread via `worker.js`, keeping the UI responsive during long-running programs
4. **WASM panic hook** — `console_error_panic_hook` produces readable stack traces instead of opaque `RuntimeError: unreachable`
5. **Build tooling `playground/justfile`** — `setup`, `compile`, `ci`, `serve`, `clean` recipes
6. **CI workflow `.github/workflows/partial_playground.yaml`** — builds wasm + playground, uploads artifact
7. **E2E workflow `.github/workflows/partial_playground_e2e.yaml`** — Playwright tests against build artifact
8. **E2E tests `playground/tests/e2e.spec.js`** — page load, compile/run, error display tests
9. **Deployment pipeline `.github/workflows/deployment.yaml`** — `build-playground`, `playground-e2e-test`, and `publish-playground` jobs integrated into the release pipeline
10. **Workspace integration** — `playground` is a member of `compiler/Cargo.toml` workspace
11. **Stepping session exports** — `load_program`, `step`, `reset_session` wasm-bindgen exports with `VmSession` thread-local storage (19 unit tests total)
12. **Stepping worker protocol** — `load_program`, `step`, `reset` commands in `worker.js`
13. **Step/Reset UI** — Step and Reset buttons in toolbar, source-change tracking with auto-recompile, `displayStepResult` display helper
14. **Stepping E2E tests** — step shows variables, scan count accumulates, reset clears output, source change auto-recompiles

### Remaining — Code Changes

The following E2E test should be updated:

1. **Fix file upload E2E test** — `playground/tests/e2e.spec.js` test `file_upload_when_iplc_file_then_executes_and_shows_results` currently just runs from the editor and does not exercise the file input path. It should compile a program via the WASM API to produce bytecode, write it to a temp file, and use Playwright's `setInputFiles` to upload through the actual file input element.

### Remaining — Manual Actions

The following items require manual action outside this repository:

1. **Create GitHub repository `ironplc/ironplc-playground`** — empty repo with GitHub Pages enabled on the `main` branch
2. **Configure DNS and GitHub Pages custom domain**
   - Add CNAME record `playground.ironplc.com` → `ironplc.github.io` at the domain registrar
   - In the `ironplc/ironplc-playground` repository Settings → Pages → Custom domain, enter `playground.ironplc.com`
   - Verify domain ownership at the organization level: github.com/organizations/ironplc/settings/pages
   - Enable "Enforce HTTPS" once the certificate is provisioned (automatic after DNS propagates)
3. **Verify `IRONPLC_WORKFLOW_PUBLISH_ACCESS_TOKEN`** — the existing personal access token (used for homebrew tap publishing) must have write access to the new `ironplc/ironplc-playground` repository

## File Inventory

### Files in `compiler/playground/`

| File | Description |
|---|---|
| `Cargo.toml` | Crate definition — `cdylib` + `rlib`, depends on parser, codegen, container, vm, wasm-bindgen, console_error_panic_hook, serde, base64 |
| `src/lib.rs` | Seven `#[wasm_bindgen]` exports (`compile`, `run`, `run_source`, `init_panic_hook`, `load_program`, `step`, `reset_session`) + `VmSession` thread-local + JSON result types + 19 unit tests |

### Files in `playground/`

| File | Description |
|---|---|
| `index.html` | Single-page app with code editor and output panel |
| `app.js` | UI logic: renders results, handles drag-and-drop, step/reset handlers, source-change tracking, sends commands to worker via postMessage |
| `worker.js` | Web Worker: loads WASM module, executes compile/run/step commands off the main thread |
| `style.css` | Layout and styling |
| `justfile` | Build recipes: `setup`, `compile`, `ci`, `serve`, `clean` |
| `package.json` | Node dependencies (Playwright) |
| `playwright.config.js` | Playwright configuration |
| `tests/e2e.spec.js` | E2E test suite |

### CI/CD Files

| File | Description |
|---|---|
| `.github/workflows/partial_playground.yaml` | Reusable workflow: build wasm + upload artifact |
| `.github/workflows/partial_playground_e2e.yaml` | Reusable workflow: Playwright E2E tests |
| `.github/workflows/deployment.yaml` | Main pipeline — includes `build-playground`, `playground-e2e-test`, `publish-playground` jobs; publishes to `ironplc/ironplc-playground` |

## Verification

1. **Rust tests**: `cd compiler && cargo test -p ironplc-playground`
2. **Local build**: `cd playground && just ci` (requires `wasm-pack` installed)
3. **Local serve**: `cd playground && just serve` → open http://localhost:8080
4. **E2E tests**: `cd playground && npx playwright test` (requires build artifact in `playground/_build/`)
5. **Full CI**: `cd compiler && just` — existing tests + coverage pass
