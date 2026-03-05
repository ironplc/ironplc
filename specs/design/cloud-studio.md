# Cloud Studio Design

## Overview

IronPLC Cloud Studio is a browser-based compiler and runtime at **cloudstudio.ironplc.com**. Users write IEC 61131-3 source code, compile it to bytecode, and execute it — all client-side via WebAssembly with zero server infrastructure. This eliminates the download/install barrier for trying IronPLC.

The IronPLC compiler core (parser, codegen, container, vm) is pure Rust with no system dependencies, making it an ideal WebAssembly target.

## Design Goals

1. **Zero-install experience** — open a URL, write PLC code, click run, see results
2. **Fully client-side** — no backend server; all compilation and execution happens in the browser via WebAssembly
3. **Full pipeline** — compile IEC 61131-3 `.st` source AND execute `.iplc` bytecode files
4. **No framework complexity** — vanilla HTML/CSS/JS frontend, no build toolchain beyond wasm-pack

## Architecture

```
┌─────────────────────────────────────────────────┐
│  Browser                                        │
│                                                 │
│  ┌──────────┐   ┌──────────────────────────┐    │
│  │ index.html│   │  ironplc-web-app.wasm    │    │
│  │ app.js    │──▶│                          │    │
│  │ style.css │   │  compile() ──▶ bytecode  │    │
│  │           │   │  run()     ──▶ variables │    │
│  │           │   │  run_source() ──▶ both   │    │
│  └──────────┘   └──────────────────────────┘    │
└─────────────────────────────────────────────────┘
```

### Rust Crate: `compiler/web-app/`

The `ironplc-web-app` crate (`compiler/web-app/`) is a `cdylib` compiled to WebAssembly via `wasm-pack`. It exposes three `#[wasm_bindgen]` functions that return JSON strings:

1. **`compile(source: &str) -> String`** — Parse IEC 61131-3 source and produce base64-encoded bytecode
   - Uses `ironplc_parser::parse_program()` → `ironplc_codegen::compile()` → `Container::write_to()`
   - Success: `{"ok": true, "bytecode": "<base64>"}`
   - Error: `{"ok": false, "diagnostics": [{"code": "...", "message": "...", "start": N, "end": N}]}`

2. **`run(bytecode_base64: &str, scans: u32) -> String`** — Execute pre-compiled `.iplc` bytecode
   - Decodes base64 → `Container::read_from()` → `Vm::new().load().start().run_round()`
   - Success: `{"ok": true, "variables": [{"index": 0, "value": 42}], "scans_completed": N}`
   - Error: `{"ok": false, "error": "VM trap: ...", "variables": [...], "scans_completed": N}`

3. **`run_source(source: &str, scans: u32) -> String`** — Compile and execute in one step
   - Chains `compile` → `run`
   - Returns combined result with diagnostics and execution output

All functions return JSON strings for simple JS interop without complex wasm-bindgen type marshalling.

### Web Frontend: `web/`

Vanilla HTML/CSS/JS — no framework.

- **`web/index.html`** — Single page with code editor, compile/run button, output panel
- **`web/app.js`** — Loads wasm module, wires UI to wasm exports, renders JSON results
- **`web/style.css`** — Layout and styling

The `web/justfile` builds the site:
- `wasm-pack build` compiles the Rust crate to wasm
- Static files are assembled into `web/_build/` for deployment

### Deployment Target: `ironplc/ironplc-cloudstudio`

A separate GitHub repository used purely as a deployment target for GitHub Pages. It is never manually edited — the CI workflow writes to it using `peaceiris/actions-gh-pages`.

Contents after deployment:
- `index.html`, `app.js`, `style.css`
- `pkg/` — wasm-bindgen output (`.wasm` + JS glue)
- `CNAME` — contains `cloudstudio.ironplc.com`

**DNS**: CNAME record `cloudstudio.ironplc.com` → `ironplc.github.io`

## CI/CD Pipeline

The web app integrates into the existing deployment pipeline:

```
release
  → build-web-app (wasm-pack build)
  → publish-prerelease
    → web-app-e2e-test (Playwright against build artifact)
    → publish-web-app (push to ironplc/ironplc-cloudstudio via GitHub Pages)
  → publish-release
```

### E2E Testing

Playwright tests run against a locally-served build artifact before publishing:

- Page loads and shows editor
- Compile and run steel thread program, verify variable values
- Compile error shows diagnostics
- Load `.iplc` file and execute
- Drag-and-drop `.iplc` file

## Key Dependencies

| Dependency | Purpose |
|---|---|
| `wasm-bindgen` | Rust ↔ JavaScript FFI for WebAssembly |
| `wasm-pack` | Build tool for Rust → wasm compilation |
| `base64` | Encode/decode bytecode for transport as JSON strings |
| `serde` / `serde_json` | Serialize results to JSON |
| Playwright | E2E browser testing |
