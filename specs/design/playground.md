# Playground Design

## Overview

IronPLC Playground is a browser-based compiler and runtime at **playground.ironplc.com**. Users write IEC 61131-3 source code, compile it to bytecode, and execute it — all client-side via WebAssembly with zero server infrastructure. This eliminates the download/install barrier for trying IronPLC.

The IronPLC compiler core (parser, codegen, container, vm) is pure Rust with no system dependencies, making it an ideal WebAssembly target.

## Design Goals

1. **Zero-install experience** — open a URL, write PLC code, click run, see results
2. **Fully client-side** — no backend server; all compilation and execution happens in the browser via WebAssembly
3. **Full pipeline** — compile IEC 61131-3 `.st` source AND execute `.iplc` bytecode files
4. **No framework complexity** — vanilla HTML/CSS/JS frontend, no build toolchain beyond wasm-pack

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│  Browser                                                 │
│                                                          │
│  ┌──────────┐   ┌───────────────────────────────────┐    │
│  │ index.html│   │  Web Worker (worker.js)           │    │
│  │ app.js    │──▶│                                   │    │
│  │ style.css │   │  ┌─────────────────────────────┐  │    │
│  │           │   │  │  ironplc-playground.wasm    │  │    │
│  │           │   │  │  compile()      ──▶ bytecode  │  │    │
│  │           │   │  │  run()          ──▶ variables │  │    │
│  │           │   │  │  run_source()   ──▶ both     │  │    │
│  │           │   │  │  load_program() ──▶ session  │  │    │
│  │           │   │  │  step()         ──▶ step     │  │    │
│  │           │   │  │  reset_session()──▶ clear    │  │    │
│  │           │   │  └─────────────────────────────┘  │    │
│  └──────────┘   └───────────────────────────────────┘    │
└──────────────────────────────────────────────────────────┘
```

All WASM compilation and execution runs in a Web Worker (`worker.js`) so the main thread stays responsive. The main thread (`app.js`) sends commands via `postMessage` and receives results asynchronously. This prevents the browser tab from freezing during long-running programs or high scan counts.

### Rust Crate: `compiler/playground/`

The `ironplc-playground` crate (`compiler/playground/`) is a `cdylib` compiled to WebAssembly via `wasm-pack`. It exposes `#[wasm_bindgen]` functions that return JSON strings:

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

A fourth export, **`init_panic_hook()`**, installs `console_error_panic_hook` so that any Rust panic inside WASM produces a readable stack trace in the browser console instead of an opaque `RuntimeError: unreachable`.

#### Stepping Mode

Three additional exports support step-through execution:

4. **`load_program(source: &str) -> String`** — Compile source and create a stepping session
   - Compiles via the same pipeline as `compile`, then stores the serialized bytecode and an initialized variable buffer in a `thread_local!` `VmSession`
   - Success: `{"ok": true, "total_scans": 0}`
   - Error: `{"ok": false, "diagnostics": [...]}`

5. **`step(scans: u32) -> String`** — Execute N scans within the current session
   - Deserializes the container from stored bytes, creates an ephemeral `Vm`, runs N rounds using the persisted variable buffer, then drops the VM. Variable values survive because they live in the owned `Vec<Slot>`, not in the VM.
   - Success: `{"ok": true, "variables": [...], "total_scans": N}`
   - Error (fault): `{"ok": false, "error": "VM trap: ...", "total_scans": N}`

6. **`reset_session() -> String`** — Clear the stepping session
   - Returns `{"ok": true}`

The key design constraint is that `VmRunning<'a>` borrows all buffers with lifetime `'a`, making it impossible to store across WASM calls. The solution is to store owned buffers in a `thread_local!` and re-create an ephemeral VM each step. Since `Vm::load()` wraps buffer references without resetting their contents, variable values persist naturally.

The UI auto-recompiles when the source changes: if the editor content has been modified since the last `load_program`, clicking Step calls `load_program` first, then `step`.

### Web Frontend: `playground/`

Vanilla HTML/CSS/JS — no framework.

- **`playground/index.html`** — Single page with code editor, compile/run button, output panel
- **`playground/app.js`** — UI logic: renders results, handles drag-and-drop, sends commands to the worker
- **`playground/worker.js`** — Web Worker that loads the WASM module and executes compile/run commands off the main thread
- **`playground/style.css`** — Layout and styling

The `playground/justfile` builds the site:
- `wasm-pack build` compiles the Rust crate to wasm
- Static files are assembled into `playground/_build/` for deployment

### Deployment Target: `ironplc/ironplc-playground`

A separate GitHub repository used purely as a deployment target for GitHub Pages. It is never manually edited — the CI workflow writes to it using `peaceiris/actions-gh-pages`.

Contents after deployment:
- `index.html`, `app.js`, `worker.js`, `style.css`
- `pkg/` — wasm-bindgen output (`.wasm` + JS glue)
- `CNAME` — contains `playground.ironplc.com`

**DNS and GitHub Pages setup**:
- CNAME record `playground.ironplc.com` → `ironplc.github.io` at the domain registrar
- Custom domain configured in the repository settings (Settings → Pages → Custom domain)
- Domain ownership verified at the organization level (github.com/organizations/ironplc/settings/pages)
- "Enforce HTTPS" enabled after certificate provisioning

## CI/CD Pipeline

The playground integrates into the existing deployment pipeline:

```
release
  → build-playground (wasm-pack build)
  → publish-prerelease
    → playground-e2e-test (Playwright against build artifact)
    → publish-playground (push to ironplc/ironplc-playground via GitHub Pages)
  → publish-release
```

### E2E Testing

Playwright tests run against a locally-served build artifact before publishing:

- Page loads and shows editor
- Compile and run steel thread program, verify variable values
- Compile error shows diagnostics
- Load `.iplc` file and execute
- Drag-and-drop `.iplc` file
- Step shows variables and scan count
- Step twice accumulates scan count
- Reset clears output and shows Ready
- Step after source change auto-recompiles

## Key Dependencies

| Dependency | Purpose |
|---|---|
| `wasm-bindgen` | Rust ↔ JavaScript FFI for WebAssembly |
| `wasm-pack` | Build tool for Rust → wasm compilation |
| `console_error_panic_hook` | Readable panic messages in browser console |
| `base64` | Encode/decode bytecode for transport as JSON strings |
| `serde` / `serde_json` | Serialize results to JSON |
| Playwright | E2E browser testing |
