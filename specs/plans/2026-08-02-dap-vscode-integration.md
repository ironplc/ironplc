# Plan: DAP Phase 5 — VS Code Integration

## Context

Phases 1–4 of the debugger (`specs/design/debugger-support.md`) delivered debug
info in the container, the iterative VM, the VM debug engine, and a
single-threaded DAP server binary (`ironplcdap`, feature-gated on the `vm-cli`
crate). The server speaks DAP over stdin/stdout and drives
`initialize → launch → setBreakpoints → configurationDone → (run) → stopped →
inspect → continue → terminated → disconnect`.

Phase 5 (§"Phase 5: VS Code Integration") wires that server into the VS Code
extension so a user can press **F5** on a `.st` file, hit breakpoints, inspect
variables, and drive scan-cycle stepping.

### What already exists (Phase 4), and the one correction it forces

The design's Phase 5 table sketched the adapter as launching
`ironplcvm-debug --dap <file.iplc>`. The **actual** Phase 4 implementation is:

- binary name is **`ironplcdap`** (see `compiler/vm-cli/Cargo.toml`), not
  `ironplcvm-debug`;
- it takes **no CLI arguments** — it reads/writes DAP on stdin/stdout
  (`compiler/vm-cli/src/dap_main.rs`);
- the program under debug is supplied by the DAP **`launch`** request's
  `arguments.program` field, which must be a path to a compiled **`.iplc`**
  container that carries a debug section
  (`compiler/vm-cli/src/dap/launch.rs`).

Phase 5 matches the shipped server, not the older sketch.

## Goals

1. A `debuggers` contribution of type `ironplc` so VS Code offers the debugger
   for Structured Text files.
2. Pressing F5 on a `.st`/`.iec`/TwinCAT source: compile it (with debug info)
   to a temporary `.iplc` and launch `ironplcdap` against it. Pressing F5 on an
   already-compiled `.iplc`: launch directly.
3. A `DebugAdapterDescriptorFactory` that resolves the `ironplcdap` binary via
   **env var → setting → bundled** (alongside the discovered `ironplcc`).
4. Custom-request commands `ironplc.stepScan` and `ironplc.scanCount` on the
   debug toolbar, forwarding `ironplc/stepScan` / `ironplc/scanCount` to the
   active debug session (server-side handling of these lands in a later phase;
   without the commands the custom requests are unreachable from the UI).
5. `breakpoints` contribution so breakpoints can be set in ST sources.

## Non-goals (unchanged v1 cuts)

`pause`-while-running, multi-instance, and variable forcing remain out of scope
(§"v1 Scope Decisions"). Server-side `ironplc/stepScan` / `ironplc/scanCount`
handling is not part of this phase — Phase 5 only makes the requests reachable
from the UI.

## Design

Following the existing extension convention (`compilerDiscovery.ts`,
`taskProviderLogic.ts`, `iplcEditorLogic.ts`), all decision logic goes into
pure, dependency-injected modules that unit tests exercise directly, while the
thin `vscode`-importing files hold only registration and I/O. Unit-test
coverage is computed only over files the tests `require`, so the `vscode`-heavy
files do not dilute the 80% line threshold.

### New files

| File | Kind | Responsibility |
|------|------|----------------|
| `src/debugAdapterLogic.ts` | pure | `isSourceProgram`, `containerOutputPath`, `findDapServerPath`, `resolveProgramPath`, `buildDebugCompileArgs`, `scanCountMessage` |
| `src/debugAdapter.ts` | vscode | `IronplcDebugConfigurationProvider` (fill defaults, compile source→`.iplc`), `IronplcDebugAdapterDescriptorFactory` (spawn `ironplcdap`) |
| `src/customRequests.ts` | vscode | register `ironplc.stepScan` / `ironplc.scanCount` commands forwarding to `vscode.debug.activeDebugSession.customRequest` |
| `src/test/unit/debugAdapterLogic.test.ts` | test | BDD-style tests for every pure function |

### `debugAdapterLogic.ts` API

- `SOURCE_EXTENSIONS: string[]` and `isSourceProgram(program): boolean` — a
  program that needs compiling (`.st`, `.iec`, `.TcPOU`, …) vs. a ready `.iplc`.
- `containerOutputPath(program, tmpDir): string` — deterministic temp `.iplc`
  path derived from the source basename.
- `findDapServerPath(env, compilerDir?): DapDiscoveryResult | undefined` —
  resolves `ironplcdap[.exe]` in order **env `IRONPLCDAP` → setting
  `ironplc.dapServerPath` → bundled (compilerDir)**, mirroring
  `CompilerEnvironment` injection so it is testable without a filesystem.
- `resolveProgramPath(configProgram, activeEditorPath): string | undefined` —
  default the program to the active editor when the launch config omits it.
- `buildDebugCompileArgs(program, output): string[]` — `['compile', program,
  '-o', output]` for `ironplcc`.
- `scanCountMessage(response): string` — format the `ironplc/scanCount` reply.

### `package.json` contributions

- `debuggers`: `{ type: 'ironplc', label: 'IronPLC', languages: [...],
  program attribute, configuration snippet, initialConfigurations }`.
- `breakpoints`: `[{ language: '61131-3-st' }, { language: 'twincat-pou' }]`.
- `commands`: `ironplc.stepScan`, `ironplc.scanCount`.
- `menus.debug/toolBar`: both commands, gated `when: debugType == 'ironplc'`.
- `configuration`: `ironplc.dapServerPath` (override for the DAP binary).
- `activationEvents`: `onDebugResolve:ironplc`.

### `extension.ts`

Register the config provider, the adapter factory (built from the discovered
compiler dir), and the custom-request commands. The compiler discovery result
already gives the bin directory; the adapter factory reuses it for bundled DAP
resolution.

## Testing

- Unit (`debugAdapterLogic.test.ts`): `isSourceProgram` for source vs `.iplc`;
  `containerOutputPath` basename mapping; `findDapServerPath` env/setting/bundled
  ordering and the not-found case; `resolveProgramPath` fallback to active
  editor; `buildDebugCompileArgs` shape; `scanCountMessage` formatting.
- `npm run compile` + `npm run lint` clean.
- `npm run test:unit` keeps aggregate line coverage ≥ 80%.
- Manual (documented, not automated here): F5 on a `.st` with a breakpoint
  stops, variables populate, Step Scan toolbar button issues the custom request.

## Out of scope / follow-ups

- Server-side `ironplc/stepScan` / `ironplc/scanCount` handling (currently
  answered `requestNotApplicable`).
- Single-stepping (`next`/`stepIn`/`stepOut`) server support.
- Functional (electron) debug tests — the unit layer covers the decision logic;
  the electron harness is not run in this phase.
