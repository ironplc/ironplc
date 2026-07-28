# Phase 4: DAP Server Scaffold (minimal v1)

## Goal

Stand up `ironplcdap <file.iplc>` — a single-threaded Debug Adapter Protocol
server that VS Code (or any DAP client) can connect to, drive through the
`initialize → launch → setBreakpoints → configurationDone →
continue/step → disconnect` lifecycle, pause on a **line breakpoint**, walk
the stack, and inspect variables. This is Phase 4 of the debugger design
(`specs/design/debugger-support.md` §"Phase 4: DAP Server"), **deliberately
cut down** from the surface in that spec to the smallest thing that is a real
debugger.

## Scope cut from the design spec

The design spec's Phase 4 lists logpoints, `evaluate`, custom scan-cycle
requests, and a packaging split. This plan cuts the first DAP phase to the
minimum and defers the rest:

- **In:** the handshake; **line breakpoints** (pause-only); one synthetic
  thread; `stackTrace` / `scopes` / `variables`; and the four execution-
  control commands **`continue`, `next`, `stepIn`, `stepOut`**.
- **Deferred to a later phase:** logpoints, `evaluate` (any expression
  evaluation), the custom `ironplc/stepScan` + `ironplc/scanCount` requests,
  conditional breakpoints, `pause`, `setVariable`/forcing, multi-instance.

Logpoints are deferred out of this first DAP phase. The engine hooks for them
are cheap once breakpoints work, so they are a natural early follow-up, but
they are not in the first cut.

This cut also pulls **Phase 3** down: no `LogpointTable` / `LogSink`, no
expression-subset evaluator. See `2026-06-25-vm-debug-engine.md`.

## Why now / sequencing

- **Phase 2 (iterative dispatch) is done**; **Phase 3 (debug engine) is
  planned** in `2026-06-25-vm-debug-engine.md`.
- The DAP server is the first thing a user can *see*: a working VS Code
  debug session. Framing, the type layer, the event loop, the state-machine
  legality table, and the `initialize`/`launch` handshake have **no
  dependency on debug info** and can be built with the Phase 3 engine alone,
  using offset-based breakpoints.
- The only debug-info-dependent piece is `setBreakpoints` (source line →
  bytecode offset) and `variables`/`scopes` rendering (slot → name/type).
  Those read the container debug section via `container::debug_section` /
  `container::debug_format`, which Layer 1 is finishing now. Isolating them
  in `dap/debug_info.rs` lets this phase land with an offset-passthrough
  resolver and swap in real line-map lookups when Layer 1 is ready.

## Current state (`compiler/vm-cli`)

`vm-cli/src/main.rs` defines `Args` with an `Action` subcommand enum (`Run`,
`Benchmark`, `Version`); each arm dispatches into `cli.rs`. `error.rs`
defines the CLI error type with `exit_code()`. `serde_json` is already a
dependency. No `dap` feature, no DAP binary, no DAP modules.

## Binary: `ironplcdap` (not `ironplcvm-debug`)

Ship the DAP server as a dedicated, feature-gated binary named **`ironplcdap`**.

Rationale: `ironplcvm-debug` reads as "a build of the VM for debugging the VM
itself," which is confusing. `ironplcdap` names what it is — the DAP server.

The design spec's §"Why not a separate DAP binary?" argued for a subcommand
to avoid duplicating the VM-embedding code. We honour that concern *without*
the confusing name: `ironplcdap` is a **second `[[bin]]` target in the same
`vm-cli` crate**, gated behind the `dap` feature, reusing the crate's VM
embedding (buffer sizing, container load) — a few lines of `main`, no
duplicated embedding logic. The VS Code extension (Phase 5) launches
`ironplcdap <file.iplc>`, which speaks DAP on stdin/stdout.

```toml
# vm-cli/Cargo.toml
[[bin]]
name = "ironplcdap"
path = "src/dap_main.rs"
required-features = ["dap"]

[features]
dap = ["dep:serde"]   # serde_json already present
```

## DAP types: hand-rolled (`dap/types.rs`)

**Decision: hand-roll a minimal `serde` types module; do not take the `dap`
crate as a dependency.**

This is the documented "discuss why we cannot use the dap crate" the review
asked for. Evidence:

- The `dap` crate (`sztomi/dap-rs`) is **alpha** (`0.4.1-alpha1`), last
  committed **Feb 2024**, last published **Sep 2023**, and self-warns that
  "breakages will be frequent; any pre-1.0 version may be breaking."
- It has **8 reverse dependencies** on all of crates.io, none mainstream.
- **No major Rust DAP implementation uses it.** Helix hand-rolls
  `helix-dap-types`; Lapce hand-rolls `lapce/dap-types`; probe-rs's
  `probe-rs-debug` (the embedded VS Code debugger) defines its own types with
  no DAP crate dependency. Hand-rolling a small serde types module is the
  ecosystem norm, not a workaround.
- Our cut-down v1 surface is **~12–15 small request/response/event structs**
  — trivial to own, and owning them avoids an alpha dependency on our public
  build.

**Fallback if we'd rather not own even the types:** vendor or depend on
`lapce/dap-types` (types only, no transport/runtime). Re-evaluate only if the
type surface grows past the v1 cut.

The hand-rolled `types.rs` uses `serde` derive + the already-present
`serde_json`. It models only the v1 messages below.

## DAP surface for the first phase

Requests handled: `initialize`, `launch`, `setBreakpoints` (line breakpoints
only — no `logMessage`), `configurationDone`, `threads` (one synthetic
thread), `stackTrace`, `scopes`, `variables`, `continue`, `next`
(step-over), `stepIn`, `stepOut`, `disconnect`.

Everything else returns DAP error `requestNotApplicable`, explicitly
including `pause`, `setVariable`, `evaluate`, `restart`, and the (not-yet-
registered) custom `ironplc/*` requests.

Capabilities advertised in `initialize`:
`supportsConfigurationDoneRequest: true`. Everything optional is **false /
omitted**: `supportsLogPoints`, `supportsConditionalBreakpoints`,
`supportsEvaluateForHovers`, `supportsSetVariable`,
`supportsStepInTargetsRequest` — all off for the first phase.

## Design

### Single-threaded event loop (`dap/server.rs`)

The loop alternates between **servicing the client** and **running the VM**;
no I/O thread, no shared mutable state across threads.

```
state = Initialized
loop {
    match state {
        Paused | Initialized | ConfigDone =>
            req = framing.read();
            handle(req)  // may mutate BreakpointTable; may set state = Running
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
next natural stop (documented single-threaded behaviour, not a bug).
`continue` / `next` / `stepIn` / `stepOut` set the `StepController` mode on
the `DebuggerHook` and flip state to `Running`. The launch `scanLimit`
bounds runaway scans. The `DebuggerHook`, its `BreakpointTable`, and the VM
buffers are owned directly by the loop — no `Arc`, no atomics.

### State legality (`dap/state.rs`)

A `Phase` mirror (`Initialized`, `Configuring`, `Running`, `Paused`,
`Terminated`, `Faulted`) plus `legal(phase, command) -> bool`. Illegal pairs
short-circuit to a DAP error with `requestNotApplicable`. Unit-tested
exhaustively.

### Launch preconditions (`dap/launch.rs`)

On `launch`, load the container and check:
1. Debug section present → else fail `NoDebugInfo` (message: "compile with
   debug info enabled").
2. `program_instances.len() == 1` → else fail `MultiInstanceUnsupported`
   (v1-limitation message from the spec).

Then size the VM buffers (operand stack, variable table, **frame stack from
`header.max_call_depth`**, data region) and construct `VmRunning`.

### Debug-info coupling, isolated (`dap/debug_info.rs`)

The only Layer 1 consumers, two functions:
- `resolve_breakpoint(source_path, line) -> Vec<(FunctionId, offset)>` via
  the line map + SOURCE_FILE table.
- `render_variables(frame) -> Vec<DapVariable>` via VAR_NAME + `debug_format`.

Ships first with a passthrough resolver (treat the DAP `line` as a raw
offset; render slots without names) so `server.rs` is end-to-end testable
before Layer 1 finishes; swap in real lookups behind the same signatures.

## Tests

- **Unit — framing**: Content-Length read/write roundtrip; partial reads;
  multiple messages in one buffer.
- **Unit — state legality**: every (phase, command) pair returns the
  documented result; `pause`, `setVariable`, `evaluate` →
  `requestNotApplicable`.
- **Unit — launch**: multi-instance → `MultiInstanceUnsupported`;
  no-debug-section → `NoDebugInfo`.
- **Integration — handshake**: spawn `ironplcdap`, send `initialize` +
  `launch` + `setBreakpoints` + `configurationDone`, expect `stopped` at the
  breakpoint. (Offset-based breakpoint until Layer 1 line maps land.)
- **Integration — inspection**: from `stopped`, request `stackTrace`,
  `scopes`, `variables`; verify frames and entries.
- **Integration — stepping**: `next` over a CALL lands on the next line in
  the caller; `stepIn` enters the callee; `stepOut` returns to the caller.
- **Integration — queued setBreakpoints**: sent while `Running`, applied at
  the next stop, not mid-instruction.
- **Integration — pause refused**: `pause` while `Running` →
  `requestNotApplicable`.
- **Integration — trap**: trigger a trap; expect `stopped{reason:"exception"}`
  then a clean `disconnect`.

## Commit order

Each commit compiles and passes `cd compiler && just` (DAP code behind the
`dap` feature; CI builds the `ironplcdap` bin with `--features dap`).

1. `dap` feature + `ironplcdap` bin target (`dap_main.rs`, no-op handler) +
   `dap/framing.rs` with its roundtrip unit test.
2. Hand-rolled `dap/types.rs` (v1 messages only) + `dap/state.rs` legality
   table + tests. Still no VM.
3. `dap/launch.rs` preconditions + buffer sizing; `initialize`/`launch`/
   `disconnect` handshake against the Phase 3 engine with an
   offset-passthrough `dap/debug_info.rs`; handshake integration test.
4. `dap/server.rs` run/stop loop: `continue`, `next`/`stepIn`/`stepOut`,
   `stackTrace`/`scopes`/`variables`, `stopped`/`terminated` events;
   inspection + stepping integration tests.
5. Swap `debug_info.rs` passthrough for real line-map / `debug_format`
   lookups once Layer 1 is complete (or in parallel, behind the same
   signatures).

## Dependencies & packaging

- New optional dep under the `dap` feature: `serde` (derive). `serde_json` is
  already present. **No `dap` / `dap-types` crate dependency** (see above).
- One extra binary, `ironplcdap`, feature-gated in the `vm-cli` crate; the
  production `ironplcvm` binary is unaffected. The VS Code extension (Phase
  5) launches `ironplcdap <file.iplc>`.

## Risks

- **Owning the DAP types.** Hand-rolling means we track protocol additions
  ourselves. Mitigation: the v1 surface is tiny and stable (the handshake +
  breakpoints + stepping messages have been stable in DAP for years);
  `lapce/dap-types` is the drop-in fallback if the surface grows.
- **Single-threaded model invariant.** The VM runs only when the loop is in
  `Running`, never concurrently with a blocking `read`. Keep that explicit;
  do not add a background runner without moving to the Phase 6 two-thread
  design.
- **Layer 1 timing.** If line maps aren't ready, the passthrough resolver
  keeps the server shippable and testable; only source-line breakpoints and
  named variables wait on Layer 1.
