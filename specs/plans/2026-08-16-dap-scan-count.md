# Plan: Expose the scan-cycle count in the debugger

## Context

Phase 5 wired two custom-request commands into the VS Code debug toolbar,
`ironplc.stepScan` and `ironplc.scanCount`, but deliberately left the server
side unimplemented (`specs/plans/2026-08-02-dap-vscode-integration.md`
§"Out of scope"). Both reached the server as unmodelled commands and fell
through to the `requestNotApplicable` branch, verified on the wire:

```
RESP  ironplc/stepScan   success=False  msg='requestNotApplicable'
RESP  ironplc/scanCount  success=False  msg='requestNotApplicable'
```

Two toolbar buttons shipped that could not work.

`scanCount` is the cheap half. `VmRunning::scan_count()` already exists
(`compiler/vm/src/vm.rs:618`) and the DAP server already reads it to enforce the
`scanLimit` bound (`dap/server.rs`), so the value is in hand at every stop
point. `stepScan` is not in this plan: `RoundOutcome::PausedAfterScan` is
declared but never constructed anywhere in the VM, and `StepMode` has no
scan-level variant, so it needs real debug-engine work and gets its own change.

### The button is the wrong shape

The first cut of this work implemented `ironplc/scanCount` behind the existing
toolbar button, which pops the count in a notification. That is the wrong
interaction for this value: the scan count changes every cycle, so it is
something you *watch* while stepping, not something you *ask for*. A
click-per-look button also can't show it changing.

The count therefore moves into a second DAP **scope**, `Runtime`, alongside the
existing program-variable scope. Clients re-request `scopes` and `variables` at
every stop, so the value tracks execution with no polling, no extension-side state,
and no button. This is protocol-standard rather than VS Code-specific, which
matters for design goal #6 ("any DAP-compatible editor"), and it is the
direction the design already points: the `scopes` row of §"How DAP Uses the
Debug Info" plans to group variables into Locals / Inputs / Outputs / In-Out /
Globals, so multiple scopes are the intended end state and today's single
flat scope is the placeholder. That scope is named `Program`, not `Variables`:
clients render scopes inside a pane already titled Variables, so `Variables`
would read as `Variables > Variables`. `Locals` would be conventional but is
inaccurate while the scope is unfiltered.

A separate scope, rather than a synthetic entry inside `Program`, keeps
runtime state from colliding with an ST variable that happens to be named
`scanCount`.

### The client half is broken independently

`session.customRequest(...)` **rejects** when the response carries
`success: false`, and `customRequests.ts` awaited with no `catch`, so a refused
request surfaced as an unhandled rejection rather than a message. `stepScan`
remains refused until its own change lands, so it needs the guard regardless.

## Goals

1. The number of completed scan cycles is visible continuously in the debug
   panel via a `Runtime` scope.
2. The `ironplc.scanCount` command, its toolbar button, and the
   `ironplc/scanCount` custom request are all retired. An intermediate cut
   implemented the custom request and kept it as a "programmatic API"; that
   does not hold up. Every DAP client can read a scope over standard
   `scopes`/`variables`, while only an IronPLC-aware client knows a custom
   request — so the request was the *less* portable path, with no caller, and
   left two ways to read one counter.
3. A refused custom request produces a clear message, not an unhandled
   rejection.

## Non-goals

- `ironplc/stepScan` server support (needs a scan-level pause in the VM debug
  engine; `RoundOutcome::PausedAfterScan` has no producer today).
- Splitting `Program` into per-IEC-section scopes. That is the design's
  eventual shape but is independent of this change.
- Any change to scan-cycle execution, `scanLimit`, or `stopOnEntry`.

## Design

### Legality

No new command, so the legality table is unchanged: the count is read through
`scopes`/`variables`, which are already legal in `Paused` and `Faulted`. That
also means it is unavailable before `launch` (no `VmRunning` exists) and while
`Running` (the single-threaded loop services no requests mid-scan), which is the
correct behaviour and needed no new rule.

### Server

| File | Change |
|------|--------|
| `dap/server.rs` | The first scope renamed `Program` (`PROGRAM_REF`, `program_variables_body`); a second `Runtime` scope at reference 2 (`runtime_variables_body`) carrying `scanCount`; and `variables` dispatching on the requested reference. |

**Reference dispatch is also a latent-bug fix.** `program_variables_body` ignored
`arguments.variablesReference` entirely and returned the program variables for
*any* handle — `VariablesArguments` was defined but never read. With two scopes
the argument must be honoured, and a handle the server never issued now yields
an empty list instead of a plausible-looking wrong answer. Dispatch is needed
anyway before structured expansion (FB instance fields) lands.

### Extension

| File | Change |
|------|--------|
| `package.json` | Remove the `ironplc.scanCount` command, its `debug/toolBar` entry, and its `commandPalette` entry. The `Runtime` scope replaces it. |
| `src/customRequests.ts` | Drop the `scanCount` handler; wrap the remaining `stepScan` call in `try`/`catch`. |
| `src/debugAdapterLogic.ts` | Drop the now-unused `ScanCountResponse` / `scanCountMessage`; add `customRequestFailedMessage(title)`, pure and unit-testable. |
| `src/test/functional/suite/extension.test.ts` | Drop the assertion that `ironplc.scanCount` is registered. |

## Testing

- `server.rs`: `scopes` offers `Program` and `Runtime` with distinct references;
  the `Runtime` scope reports 0 completed scans at a `stopOnEntry` pause, and
  reports `scanCount` as `ULINT` advancing by one cycle between consecutive
  breakpoint stops; an unknown reference returns no variables.
- `debugAdapterLogic.test.ts`: `customRequestFailedMessage` formatting.
- The functional suite asserts which commands are registered, so removing a
  contributed command means removing its assertion there too. Run the
  extension's **`just ci`** (which includes `test:functional`), not only
  `npm run test:unit` — the unit suite does not see command registration.
- `cd compiler && just`.

## Out of scope / follow-ups

- `ironplc/stepScan` server support and its icon (it still uses
  `$(debug-step-over)`, the same glyph as VS Code's built-in Step Over button).
- More `Runtime` entries — cycle time, next-due — which now have a home that
  needs no new scope.
