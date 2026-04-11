# Surface Pause/Stop as Inline Code Lenses

## Goal

Make pause and stop discoverable in the VS Code extension by replacing the
"Run Program" code lens with inline "Pause" + "Stop" (or "Resume" + "Stop")
actions while a program is running. Today those actions exist only in the
status bar, where users miss them.

## Background

The plan at `specs/plans/2026-04-05-program-run-button.md` was fully
implemented: `RunSession.pause()`, `resume()`, and `stop()` all work
(`integrations/vscode/src/runSession.ts:80-106`), the commands
`ironplc.pauseProgram` and `ironplc.stopProgram` are registered
(`integrations/vscode/src/extension.ts:207-226`), and status bar items appear
while running (`extension.ts:92-117`). The user complaint — "once started,
there is no way to stop or pause" — is a **discoverability** problem, not a
missing feature. The code lens above the `PROGRAM` declaration is the most
visible affordance, and today it never changes.

The LSP server already supports `ironplc/run`, `ironplc/step`, and
`ironplc/stop` (`compiler/ironplc-cli/src/lsp.rs:274-306`). Pause is client
side: halt the step interval, let the `VmRunner` session persist until the
user clicks stop. No server-side changes are required.

## Architecture

Make the code lens provider reactive to `RunSession` state. Every state
transition fires both the existing status bar update and a new
`onDidChangeCodeLenses` event, so VS Code re-renders the lenses.

Lens mapping:

| State            | Lenses                                              |
|------------------|-----------------------------------------------------|
| `idle` / `error` | `$(play) Run Program` → `ironplc.runProgram`        |
| `idle` + no compiler | `$(warning) Run Program (no compiler)` → `ironplc.runProgram` |
| `running`        | `$(debug-pause) Pause`, `$(debug-stop) Stop`        |
| `paused`         | `$(debug-continue) Resume`, `$(debug-stop) Stop`    |

Multiple `vscode.CodeLens` objects at the same range render as " | "-separated
links, so pause and stop appear side-by-side above the PROGRAM line.

## File Map

### Modified files
- `integrations/vscode/src/runCodeLensProvider.ts` — add `state` +
  `hasCompiler` parameters to `findProgramLenses`; add a new
  `RunProgramCodeLensProvider` class with `onDidChangeCodeLenses` and
  `setState()`.
- `integrations/vscode/src/extension.ts` — replace the anonymous inline
  `CodeLensProvider` with an instance of `RunProgramCodeLensProvider`; call
  `setState()` from `updateStatusBar` so lens and status bar refresh together.
- `integrations/vscode/src/test/unit/runCodeLensProvider.test.ts` — add tests
  for each state variant (running, paused, error, no-compiler, multi-program).

### Unchanged
- `runSession.ts` — pause/resume/stop logic is already correct.
- LSP server — the existing run/step/stop primitives are enough.
- `package.json` — commands already contributed; no new keybindings.

## Tasks

- [ ] Add `state` + `hasCompiler` parameters to `findProgramLenses`
- [ ] Add `RunProgramCodeLensProvider` class in `runCodeLensProvider.ts`
- [ ] Wire `RunProgramCodeLensProvider` into `registerRunSupport` and have
  `updateStatusBar` call `setState()`
- [ ] Add BDD unit tests for running / paused / error / no-compiler states
- [ ] Verify `npm run compile` and `npm test` pass in
  `integrations/vscode/`
- [ ] Manual smoke test: run → pause → resume → stop cycle through inline
  lenses
