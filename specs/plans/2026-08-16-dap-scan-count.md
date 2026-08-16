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

1. `ironplc/scanCount` returns `{ "scanCount": <completed cycles> }` at any stop
   point where inspection is legal, as the programmatic API for non-VS-Code
   clients and tests.
2. The count is visible continuously in the debug panel via a `Runtime` scope,
   and the `ironplc.scanCount` command and toolbar button are retired.
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

`ScanCount` joins the **inspection** group in the legality table: legal in
`Paused` and `Faulted`, illegal everywhere else — matching
`threads`/`stackTrace`/`scopes`/`variables`, since it reads VM state without
touching execution and which scan a trap landed on is useful at a fault pause.
It is illegal before `launch` (no `VmRunning` exists) and while `Running` (the
single-threaded loop services no requests mid-scan).

Adding the variant automatically extends the exhaustive phase × command test in
`state.rs`, which checks `legal()` against an independent expected-phases table.

### Server

| File | Change |
|------|--------|
| `dap/types.rs` | `ScanCountResponseBody { scan_count: u64 }`, `camelCase` so the wire field is `scanCount`. |
| `dap/state.rs` | `Command::ScanCount`; map `"ironplc/scanCount"`; legality as above; extend both test tables. |
| `dap/server.rs` | `ironplc/scanCount` dispatch arm; the first scope renamed `Program` (`PROGRAM_REF`, `program_variables_body`); a second `Runtime` scope at reference 2 (`runtime_variables_body`); and `variables` dispatching on the requested reference. |

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

## Testing

- `state.rs`: the exhaustive legality test covers the new command once it is in
  `ALL_COMMANDS` and `expected_legal_phases`.
- `server.rs`: `scanCount` reports 0 at a `stopOnEntry` pause, advances by one
  between consecutive breakpoint stops, and is refused before `launch`;
  `scopes` offers both scopes with distinct references; the `Runtime` scope
  reports `scanCount` as `ULINT` and advances across cycles; an unknown
  reference returns no variables.
- `debugAdapterLogic.test.ts`: `customRequestFailedMessage` formatting.
- `cd compiler && just`, plus the extension's `npm run compile && npm run lint
  && npm run test:unit`.

## Out of scope / follow-ups

- `ironplc/stepScan` server support and its icon (it still uses
  `$(debug-step-over)`, the same glyph as VS Code's built-in Step Over button).
- More `Runtime` entries — cycle time, next-due — which now have a home that
  needs no new scope.
