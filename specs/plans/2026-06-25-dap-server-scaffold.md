# Phase 4: DAP Server Scaffold (minimal v1)

> **Renamed 2026-08-22.** This plan was written when the debug server was
> named `ironplcdap`. The name below has been updated to `ironplcvmd` so it
> matches the shipped binary; see
> [the rename plan](2026-08-22-rename-ironplcdap-to-ironplcvmd.md).

## Status (2026-08-02)

Commits 1–3 have landed (PR #1205 merged commit 3: the `initialize → launch →
disconnect` handshake, launch preconditions, and VM buffer sizing/startup).
This plan has since been **narrowed to a minimal Phase 4** whose only purpose
is to give Phase 5 (VS Code integration) something to launch: stop on a
source-line breakpoint, inspect the stack and variables, and resume.
**Single-stepping (`next`/`stepIn`/`stepOut`) and trap→`exception` handling
are deferred to a new Phase 4b** (see §"Phase 4b — deferred follow-up"),
because Phase 5 does not depend on them and the Phase 3 engine already
implements the stepping primitives, so wiring them later is cheap.

## Goal

Stand up `ironplcvmd <file.iplc>` — a single-threaded Debug Adapter Protocol
server that VS Code (or any DAP client) can connect to, drive through the
`initialize → launch → setBreakpoints → configurationDone → continue →
disconnect` lifecycle, pause on a **line breakpoint**, walk the stack, and
inspect variables. This is Phase 4 of the debugger design
(`specs/design/debugger-support.md` §"Phase 4: DAP Server"), **deliberately
cut down** from the surface in that spec to the smallest thing that is a real
debugger — and cut down again (no stepping) to the smallest thing Phase 5 can
launch.

## Scope cut from the design spec

The design spec's Phase 4 lists logpoints, `evaluate`, custom scan-cycle
requests, and a packaging split. This plan cuts the first DAP phase to the
minimum and defers the rest:

- **In (minimal Phase 4):** the handshake; **line breakpoints** (pause-only);
  one synthetic thread; `stackTrace` / `scopes` / `variables`; and the single
  execution-control command **`continue`** — everything needed to launch from
  VS Code, stop on a source-line breakpoint, inspect the stack and variables,
  and resume.
- **Deferred to Phase 4b (this plan, below):** single-stepping (`next`,
  `stepIn`, `stepOut`) and trap→`stopped{reason:"exception"}` handling. The
  Phase 3 engine already implements stepping
  (`DebuggerHook::step_over`/`step_in`/`step_out`) and the trap surfaces
  through the fault path, so both are small follow-ups that Phase 5 does not
  depend on.
- **Deferred to a later phase (unchanged):** logpoints, `evaluate` (any
  expression evaluation), the custom `ironplc/stepScan` + `ironplc/scanCount`
  requests, conditional breakpoints, `pause`, `setVariable`/forcing,
  multi-instance.

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

## Binary: `ironplcvmd` (not `ironplcvm-debug`)

Ship the DAP server as a dedicated, feature-gated binary named **`ironplcvmd`**.

Rationale: `ironplcvm-debug` reads as "a build of the VM for debugging the VM
itself," which is confusing. `ironplcvmd` names what it is — the DAP server.

The design spec's §"Why not a separate DAP binary?" argued for a subcommand
to avoid duplicating the VM-embedding code. We honour that concern *without*
the confusing name: `ironplcvmd` is a **second `[[bin]]` target in the same
`vm-cli` crate**, gated behind the `dap` feature, reusing the crate's VM
embedding (buffer sizing, container load) — a few lines of `main`, no
duplicated embedding logic. The VS Code extension (Phase 5) launches
`ironplcvmd <file.iplc>`, which speaks DAP on stdin/stdout.

```toml
# vm-cli/Cargo.toml
[[bin]]
name = "ironplcvmd"
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

Requests handled (minimal Phase 4): `initialize`, `launch`, `setBreakpoints`
(line breakpoints only — no `logMessage`), `configurationDone`, `threads`
(one synthetic thread), `stackTrace`, `scopes`, `variables`, `continue`,
`disconnect`.

Everything else returns DAP error `requestNotApplicable`. This explicitly
includes the stepping requests **`next`, `stepIn`, `stepOut`** (deferred to
Phase 4b — the `state::legal` table already models them, so promoting them is
a legality-plus-handler change), as well as `pause`, `setVariable`,
`evaluate`, `restart`, and the (not-yet-registered) custom `ironplc/*`
requests.

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
`continue` clears any step mode and flips state to `Running`; the stepping
commands (`next` / `stepIn` / `stepOut`), which set the `StepController` mode
on the `DebuggerHook`, are Phase 4b. The launch `scanLimit` bounds runaway
scans. The `BreakpointTable` and the VM buffers are owned directly by the
loop — no `Arc`, no atomics. Because the loop mutates the `BreakpointTable`
between rounds (a `setBreakpoints` at a pause) while the `DebuggerHook`
borrows it during a round, the hook is constructed per round; a fresh hook
after a breakpoint pause is told to suppress that one location once
(`DebuggerHook::suppress_next_breakpoint`) so `continue` makes forward
progress instead of re-triggering in place.

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
- **Integration — handshake**: spawn `ironplcvmd`, send `initialize` +
  `launch` + `setBreakpoints` + `configurationDone`, expect `stopped` at the
  breakpoint. (Offset-based breakpoint until Layer 1 line maps land.)
- **Integration — inspection**: from `stopped`, request `stackTrace`,
  `scopes`, `variables`; verify frames and entries.
- **Integration — queued setBreakpoints**: sent while `Running`, applied at
  the next stop, not mid-instruction.
- **Integration — pause refused**: `pause` while `Running` →
  `requestNotApplicable`.
- **Integration — stepping refused (minimal Phase 4)**: `next` / `stepIn` /
  `stepOut` while `Paused` → `requestNotApplicable` (promoted in Phase 4b).

Deferred to Phase 4b (see below):

- **Integration — stepping**: `next` over a CALL lands on the next line in
  the caller; `stepIn` enters the callee; `stepOut` returns to the caller.
- **Integration — trap**: trigger a trap; expect `stopped{reason:"exception"}`
  then a clean `disconnect`.

## Commit order

Each commit compiles and passes `cd compiler && just` (DAP code behind the
`dap` feature; CI builds the `ironplcvmd` bin with `--features dap`).

1. `dap` feature + `ironplcvmd` bin target (`dap_main.rs`, no-op handler) +
   `dap/framing.rs` with its roundtrip unit test.
2. Hand-rolled `dap/types.rs` (v1 messages only) + `dap/state.rs` legality
   table + tests. Still no VM.
3. `dap/launch.rs` preconditions + buffer sizing; `initialize`/`launch`/
   `disconnect` handshake against the Phase 3 engine with an
   offset-passthrough `dap/debug_info.rs`; handshake integration test.
4. `dap/server.rs` **minimal** run/stop loop: `configurationDone` starts the
   run; `setBreakpoints`; `continue`; `threads`; `stackTrace`/`scopes`/
   `variables`; `stopped`(breakpoint)/`terminated` events. Inspection + queued
   `setBreakpoints` + refusal integration tests. **No stepping, no trap-stop.**
   Adds `DebuggerHook::suppress_next_breakpoint` to the `vm` crate so a
   per-round hook resumes past a hit breakpoint.
5. Swap `debug_info.rs` passthrough for real line-map / `debug_format`
   lookups once Layer 1 is complete (or in parallel, behind the same
   signatures). This is the last piece of minimal Phase 4: it turns
   offset-keyed breakpoints and `var[i]` slots into source-line breakpoints
   and named/typed variables, which is what makes the Phase 5 VS Code session
   read as a real debugger.

## Phase 4b — deferred follow-up

Not required for Phase 5. Landed as its own change(s) after the minimal loop
is working in VS Code. The Phase 3 engine already provides the primitives, so
each is a thin server-side addition:

6. **Stepping.** Promote `next`/`stepIn`/`stepOut` from `requestNotApplicable`
   to handlers that call `DebuggerHook::step_over`/`step_in`/`step_out` and
   flip to `Running`; emit `stopped{reason:"step"}` on the `Step` pause.
   Because stepping consumes the hook's call-depth (`before_call`/
   `after_return`), the per-round-hook construction from commit 4 must
   preserve depth across a resume (not just the breakpoint-skip flag) — extend
   the `suppress_next_breakpoint` seam to carry the full resume state, or keep
   one hook alive for the duration of a `Running` span. Tests: `next` over a
   CALL, `stepIn` into a callee, `stepOut` back to the caller.
7. **Trap-stop.** Map an `Err(FaultContext)` from `run_round_debug` to
   `stopped{reason:"exception"}` with the trap's V-code in `description`;
   accept only inspection requests in the resulting `Faulted` phase (the
   `state::legal` table already encodes this); `disconnect` tears down
   cleanly. Test: trigger a divide-by-zero in the scan body and assert the
   exception stop + clean disconnect.

## Dependencies & packaging

- New optional dep under the `dap` feature: `serde` (derive). `serde_json` is
  already present. **No `dap` / `dap-types` crate dependency** (see above).
- One extra binary, `ironplcvmd`, feature-gated in the `vm-cli` crate; the
  production `ironplcvm` binary is unaffected. The VS Code extension (Phase
  5) launches `ironplcvmd <file.iplc>`.

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
