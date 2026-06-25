# Phase 4: DAP Server Scaffold

## Goal

Stand up `ironplcvm debug --dap <file.iplc>` — a single-threaded Debug
Adapter Protocol server that VS Code (or any DAP client) can connect to,
drive through the `initialize → launch → setBreakpoints →
configurationDone → continue/step → disconnect` lifecycle, and use to pause
on a breakpoint, walk the stack, and inspect variables. This is Phase 4 of
the debugger design (`specs/design/debugger-support.md` §"Phase 4: DAP
Server").

This phase is the **protocol plumbing** that turns the Phase 3 VM debug
engine into something an editor can talk to. It depends on Phase 3
(`DebuggerHook`, `run_round_debug`, `BreakpointTable`, `Phase`) for actual
execution control, and on Layer 1 debug info only for the *final*
source-line ↔ bytecode-offset translation and variable rendering — which is
isolated to one module (`dap/debug_info.rs`) so the rest of the server can
be built and tested against raw offsets first.

## Why now / sequencing

- **Phase 2 (iterative dispatch) is done**; **Phase 3 (debug engine) is
  planned** in `specs/plans/2026-06-25-vm-debug-engine.md`.
- The DAP server is the first thing a user can *see*: it produces a working
  VS Code debug session. Most of it — framing, the protocol type layer, the
  event loop, the state-machine legality table, the `initialize`/`launch`
  handshake — has **no dependency on debug info** and can be built with the
  Phase 3 engine alone, using offset-based breakpoints.
- The only debug-info-dependent piece is `setBreakpoints` (source line →
  bytecode offset) and `variables`/`scopes` rendering (slot → name/type).
  Those read the container's debug section via
  `container::debug_section` / `container::debug_format`, which the Layer 1
  work is finishing now. Isolating them in `dap/debug_info.rs` lets this
  phase land with a stubbed/offset-based breakpoint resolver and swap in the
  real line-map lookup when Layer 1 is ready.

## Current state (`compiler/vm-cli`)

`vm-cli/src/main.rs` defines `Args` with an `Action` subcommand enum (`Run`,
`Benchmark`, `Version`); each arm dispatches into `cli.rs`. `error.rs`
defines the CLI error type with `exit_code()`. `serde_json` is already a
dependency. No `dap` feature, no debug subcommand, no DAP modules.

## Scope

**In:**

- New `dap` cargo feature on `vm-cli` gating all DAP code and its extra
  dependencies.
- `Debug { file, --dap, --stop-on-entry, --scan-limit }` subcommand behind
  `#[cfg(feature = "dap")]`.
- `dap/framing.rs` — Content-Length header framing (read + write) over
  stdin/stdout.
- `dap/types.rs` — the DAP message types needed for v1 (`Request`,
  `Response`, `Event`, `Capabilities`, and the request/response bodies
  listed below). Prefer the `dap` / `dap-types` crate if it fits cleanly;
  otherwise hand-roll with `serde`.
- `dap/state.rs` — a `Phase` mirror and a per-(state, request) legality
  table that returns `requestNotApplicable` for illegal requests (including
  `pause` and `setVariable`, which are v1 cuts).
- `dap/launch.rs` — launch preconditions: reject multi-instance containers
  (`MultiInstanceUnsupported`) and containers with no debug section
  (`NoDebugInfo`).
- `dap/debug_info.rs` — the **only** debug-info-coupled module: source line
  ↔ `(FunctionId, offset)` translation (line map), and slot → name/type
  rendering (`container::debug_format`). Ships first with an offset-passthrough
  resolver so the server is testable before Layer 1 lands.
- `dap/server.rs` — the **single-threaded** event loop: drain queued DAP
  requests at natural stop points, then run the VM under
  `VmRunning::run_round_debug` to the next stop; translate `RoundOutcome` /
  `PauseReason` into `stopped` / `output` / `terminated` events.
- Re-export `ironplcvm-debug` binary (or `dap` feature default-on in the
  VS Code distribution) per the spec's packaging decision.

**Out (deferred):**

- VS Code extension wiring (`debugAdapter.ts`, launch config, toolbar) —
  Phase 5.
- `pause` (interactive interrupt), `setVariable` / forcing, multi-instance,
  conditional breakpoints, compound expression `evaluate` — all v1 cuts;
  the server returns `requestNotApplicable` / `evaluateUnsupported` for them.
- Two-thread server, `ArcSwap`, `AtomicBool` — explicitly out
  (`§Single-threaded DAP loop`).

## DAP surface for v1

Requests handled: `initialize`, `launch`, `setBreakpoints` (including
`logMessage` logpoints), `configurationDone`, `threads` (one synthetic
thread), `stackTrace`, `scopes`, `variables`, `continue`, `next`
(step-over), `stepIn`, `stepOut`, `evaluate` (bare-identifier subset only),
`disconnect`, plus the custom `ironplc/stepScan` and `ironplc/scanCount`.

Requests explicitly refused with `requestNotApplicable`: `pause`,
`setVariable`, `restart`, `setExpression`.

Capabilities advertised in `initialize`:
`supportsConfigurationDoneRequest: true`, `supportsLogPoints: true`,
`supportsStepInTargetsRequest: false`, `supportsSetVariable: false`,
`supportsConditionalBreakpoints: false`, `supportsEvaluateForHovers: true`
(identifier subset).

## Design

### Single-threaded event loop (`dap/server.rs`)

The loop alternates between **servicing the client** and **running the VM**;
there is no I/O thread and no shared mutable state across threads.

```
state = Initialized
loop {
    match state {
        // Stopped/Initialized: block on the next DAP request, handle it.
        Paused | Initialized | ConfigDone =>
            req = framing.read();
            handle(req)  // may mutate BreakpointTable, may set state = Running
        // Running: drive the VM to the next natural stop, then go back
        // to draining requests.
        Running =>
            match vm.run_round_debug(now, &mut debugger_hook)? {
                Completed       => emit(terminated); state = Initialized
                PausedAfterScan => emit(stopped{reason:"step"}); state = Paused
                Paused(reason)  => emit(stopped{map(reason)}); state = Paused
            }
    }
}
```

`setBreakpoints` received while `Running` is **queued** and applied at the
next natural stop — this is the documented single-threaded behaviour
(`§Single-threaded DAP loop`), not a bug. `continue` / `next` / `stepIn` /
`stepOut` set the `StepController` mode on the `DebuggerHook` and flip state
to `Running`. `scan-limit` (launch option) bounds runaway scans.

The `DebuggerHook` (Phase 3), its `BreakpointTable`, and the VM buffers are
all owned directly by the loop — no `Arc`, no atomics.

### State legality (`dap/state.rs`)

A `Phase` mirror (`Initialized`, `Configuring`, `Running`, `Paused`,
`Terminated`, `Faulted`) plus a function `legal(phase, command) -> bool`.
Illegal pairs short-circuit to a DAP error response with
`requestNotApplicable`. This table is the spec's per-request "Legal in"
column and is unit-tested exhaustively.

### Launch preconditions (`dap/launch.rs`)

On `launch`, load the container and check:
1. Debug section present → else fail `NoDebugInfo` with a message pointing
   at "compile with debug info enabled".
2. `program_instances.len() == 1` → else fail `MultiInstanceUnsupported`
   with the v1-limitation message from the spec.

Then size the VM buffers (operand stack, variable table, **frame stack from
`header.max_call_depth`**, data region) and construct `VmRunning`.

### Debug-info coupling, isolated (`dap/debug_info.rs`)

Two functions, the only Layer 1 consumers:
- `resolve_breakpoint(source_path, line) -> Vec<(FunctionId, offset)>` via
  the line map + SOURCE_FILE table (BLAKE3 drift check optional/warn).
- `render_variables(frame) -> Vec<DapVariable>` via VAR_NAME + `debug_format`.

Ship first with a passthrough resolver (treat the DAP `line` as a raw
offset, render slots without names) so `server.rs` is end-to-end testable
before Layer 1 finishes; swap in the real lookups behind the same signatures.

## Tests

- **Unit — framing**: Content-Length read/write roundtrip, including partial
  reads and multiple messages in one buffer.
- **Unit — state legality**: every (phase, command) pair returns the
  documented result; `pause` and `setVariable` → `requestNotApplicable`.
- **Unit — launch**: multi-instance container → `MultiInstanceUnsupported`;
  no-debug-section container → `NoDebugInfo`.
- **Integration — handshake**: spawn the binary, send `initialize` +
  `launch` + `setBreakpoints` + `configurationDone`, expect a `stopped`
  event at the breakpoint. (Uses an offset-based breakpoint until Layer 1
  line maps land.)
- **Integration — inspection**: from `stopped`, request `stackTrace`,
  `scopes`, `variables`; verify the frames and entries.
- **Integration — queued setBreakpoints**: send `setBreakpoints` while
  `Running`; verify it applies at the next stop, not mid-instruction.
- **Integration — pause refused**: `pause` while `Running` →
  `requestNotApplicable`.
- **Integration — logpoint**: `setBreakpoints` with `logMessage`; VM does
  not pause; the formatted text arrives as an `output` event.
- **Integration — trap**: trigger a trap; expect `stopped{reason:"exception"}`
  then a clean `disconnect`.

## Commit order

Each commit compiles and passes `cd compiler && just` (DAP code behind the
feature; CI builds with `--features dap`).

1. `dap` feature + `Debug` subcommand wiring (no-op handler) + `framing.rs`
   with its roundtrip unit test.
2. `types.rs` + `state.rs` legality table + tests. Still no VM.
3. `launch.rs` preconditions + buffer sizing; `initialize`/`launch`/
   `disconnect` handshake against the Phase 3 engine with an
   offset-passthrough `debug_info.rs`; handshake integration test.
4. `server.rs` run/stop loop: `continue`, `stepIn/next/stepOut`,
   `stackTrace`/`scopes`/`variables`, `stopped`/`output`/`terminated`
   events; inspection + step integration tests.
5. Logpoints + `evaluate` (identifier subset) + custom `ironplc/stepScan`,
   `ironplc/scanCount`.
6. Swap `debug_info.rs` passthrough for real line-map / debug_format lookups
   once Layer 1 is complete (or in parallel, behind the same signatures).

## Dependencies & packaging

- New optional deps under the `dap` feature: `serde` (derive) — `serde_json`
  is already present. Evaluate the `dap` crate for `types.rs`; hand-roll if
  it pulls async/`tokio`.
- Per the spec, ship `ironplcvm` (no DAP) and `ironplcvm-debug` (DAP on), or
  a single binary with `dap` default-on in the VS Code distribution. The
  VS Code extension (Phase 5) launches `ironplcvm-debug --dap <file.iplc>`.

## Risks

- **DAP type crate fit.** Many Rust DAP crates assume an async runtime; the
  v1 server is deliberately synchronous. Mitigation: prefer hand-rolled
  `serde` types if the crate forces `tokio`; the message set is small.
- **Blocking read starves nothing only because there's no background VM.**
  The single-threaded model is correct *because* the VM only runs when the
  loop is in `Running` and never concurrently with a blocking `read`. Keep
  that invariant explicit; do not add a background runner without moving to
  the Phase 6 two-thread design.
- **Layer 1 timing.** If line maps aren't ready when the rest of Phase 4
  lands, the passthrough resolver keeps the server shippable and testable;
  only source-line breakpoints and named variables wait on Layer 1.
