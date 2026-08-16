# Plan: Implement the `ironplc/scanCount` custom DAP request

## Context

Phase 5 wired two custom-request commands into the VS Code debug toolbar,
`ironplc.stepScan` and `ironplc.scanCount`, but deliberately left the server
side unimplemented (`specs/plans/2026-08-02-dap-vscode-integration.md`
§"Out of scope"). Both therefore reach the server as unmodelled commands and
fall through to the `requestNotApplicable` branch, verified on the wire:

```
RESP  ironplc/stepScan   success=False  msg='requestNotApplicable'
RESP  ironplc/scanCount  success=False  msg='requestNotApplicable'
```

Two toolbar buttons ship that cannot work.

`scanCount` is the cheap half. `VmRunning::scan_count()` already exists
(`compiler/vm/src/vm.rs:618`) and the DAP server already reads it to enforce the
`scanLimit` bound (`dap/server.rs:275`), so the value is in hand at every stop
point. `stepScan` is not in this plan: `RoundOutcome::PausedAfterScan` is
declared but never constructed anywhere in the VM, and `StepMode` has no
scan-level variant, so it needs real debug-engine work and gets its own change.

### The client half is broken independently

`session.customRequest(...)` **rejects** when the response carries
`success: false`. `customRequests.ts` awaits both calls with no `catch`, so:

- the deliberate `'IronPLC: scan count is not available.'` fallback in
  `scanCountMessage` (`debugAdapterLogic.ts:189`) is unreachable — the rejection
  happens before it is called;
- both buttons surface an unhandled rejection rather than a useful message.

Implementing the server fixes `scanCount`'s happy path but leaves the failure
path broken, and leaves `stepScan` failing badly until its own change lands. So
the error handling is part of this change.

### Icon

`scanCount` currently uses `$(watch)` — a wristwatch, which reads as elapsed
time rather than a count of cycles. It becomes `$(symbol-numeric)`, a numeric
glyph that says "shows a number" and collides with no built-in debug-toolbar
icon.

(`stepScan` uses `$(debug-step-over)`, the exact glyph VS Code already renders
for its built-in Step Over button, so the toolbar shows it twice meaning two
things. That is left for the `stepScan` change.)

## Goals

1. `ironplc/scanCount` returns `{ "scanCount": <completed cycles> }` at any stop
   point where inspection is legal.
2. A refused or failed custom request produces a clear message, not an unhandled
   rejection — for both commands.
3. The `scanCount` toolbar icon communicates what the button does.

## Non-goals

- `ironplc/stepScan` server support (needs a scan-level pause in the VM debug
  engine; `RoundOutcome::PausedAfterScan` has no producer today).
- Any change to scan-cycle execution, `scanLimit`, or `stopOnEntry`.

## Design

### Legality

`ScanCount` joins the **inspection** group in the legality table: legal in
`Paused` and `Faulted`, illegal everywhere else. The rationale matches
`threads`/`stackTrace`/`scopes`/`variables` — it reads VM state without
touching execution, and knowing which scan a trap landed on is useful at a
terminal pause.

It is *not* legal in `Initialized`/`Configuring` (no `VmRunning` exists before
`launch`), nor in `Running` (the single-threaded loop services no requests while
scanning). `Terminated` is excluded to stay consistent with the rest of the
inspection group; VS Code tears the toolbar down on `terminated`, so it is
unreachable from the UI anyway.

Adding the variant to `Command` automatically extends the exhaustive
phase × command legality test in `state.rs`, which checks `legal()` against an
independent `expected_legal_phases()` table.

### Server

| File | Change |
|------|--------|
| `dap/types.rs` | `ScanCountResponseBody { scan_count: u64 }`, `camelCase` so the wire field is `scanCount`. |
| `dap/state.rs` | `Command::ScanCount`; map `"ironplc/scanCount"`; legality as above; extend both test tables. |
| `dap/server.rs` | Dispatch arm answering with `running.scan_count()`. |

### Extension

| File | Change |
|------|--------|
| `src/debugAdapterLogic.ts` | `customRequestFailedMessage(title)` — the message shown when a custom request is refused. Pure, so it is unit-testable alongside `scanCountMessage`. |
| `src/customRequests.ts` | Wrap both `customRequest` calls in `try`/`catch`. `scanCount` falls back to `scanCountMessage(undefined)` on failure, which is exactly the "not available" path that was already written and unreachable. `stepScan` reports that the server does not support it yet. |
| `package.json` | `scanCount` icon `$(watch)` → `$(symbol-numeric)`. |

## Testing

- `state.rs`: the exhaustive legality test covers the new command once it is
  added to `ALL_COMMANDS` and `expected_legal_phases`; plus a `from_request`
  mapping assertion.
- `server.rs`: `scanCount` at a `stopOnEntry` pause reports 0 completed scans;
  after continuing through breakpoint stops in later scans it reports the
  increasing count; `scanCount` before `launch` is refused.
- `debugAdapterLogic.test.ts`: `customRequestFailedMessage` formatting.
- `cd compiler && just` and the extension's `npm run compile && npm run lint &&
  npm run test:unit` must pass.

## Out of scope / follow-ups

- `ironplc/stepScan` server support and its icon.
- Surfacing the scan count continuously (e.g. a status-bar item) rather than
  on demand behind a button.
