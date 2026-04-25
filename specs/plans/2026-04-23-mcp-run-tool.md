# Plan: Add MCP `run` Tool (Container Execution)

## Context

The IronPLC MCP server is designed in `specs/design/mcp-server.md` as a two-milestone surface: a **validation milestone** (parse/check/symbols/etc.) and an **execution milestone** (compile → run). The validation milestone is fully implemented. The execution milestone is **partially** implemented — `compile` produces an `.iplc` bytecode container and stashes it in an LRU cache returning a `container_id`, and `container_drop` evicts entries — but the `run` tool that loads a cached container into the IronPLC VM and executes it is **not yet implemented**.

A clarification on terminology: "container" here means an IronPLC `.iplc` bytecode container, not a Docker/OCI container. The design (REQ-ARC-001..035) explicitly uses the bytecode VM as the sandbox; there is no OS-level isolation involved. All the heavy infrastructure already exists — `ironplc-vm` provides the execution engine (`Vm::new().load(c, &mut bufs).start()`, `run_round`, `read_variable_raw`), `ironplc-container` round-trips the bytecode, the cache holds the bytes, and `vm-cli` is a working reference implementation. What is missing is the MCP-side handler that wires those parts together, resolves fully-qualified variable names (REQ-ARC-020/021), enforces resource limits (REQ-ARC-030..035), and emits a JSON trace per REQ-TOL-040..048.

This plan adds that missing capability so an AI agent can complete the design's intended `compile → run` loop and verify program *behavior*, not just syntax.

## Recommended Approach

Ship the MVP (Phase 10 of the master plan) first: `every_cycle` trace mode, `container_id` lookup, name resolution, resource-limit enforcement, no stimuli. Defer Phase 11 (stimuli, all trace modes, full IEC value codec, task filter, `container_base64` ingestion) to a follow-up — the JSON↔PLC value conversion alone (REQ-TOL-043 covers every IEC type recursively, including NaN/Infinity, enums, arrays, structs) is orthogonal to "VM lifecycle + name resolution + limits."

### File changes

**New files**
- `compiler/mcp/src/tools/run.rs` — handler, input/response types, name resolution, response builder. Mirrors the structure of `tools/compile.rs`.
- `compiler/mcp/src/runner.rs` — VM lifecycle (`Container::read_from` → `VmBuffers::from_container` → `Vm::new().load().start()` → cycle loop → `stop()`), `build_symbol_map`, `value_from_raw`. Pulled out so unit tests don't need the MCP transport.

**Modified files**
- `compiler/mcp/Cargo.toml` — add `ironplc-vm = { path = "../vm", features = ["profiling"] }`. The `profiling` feature is required because fuel approximation (see below) reads `Vm::profile().total()`, which is gated behind it.
- `compiler/mcp/src/cache.rs` — extend `CachedContainer` with a `VariableSymbolMap` field (qualified-name → `VarIndex` + `iec_type_tag` + var-section + address). REQ-ARC-070 already requires this; today the field is missing. Add `ResolvedVar` struct and a bare-name reverse index for REQ-ARC-020 fallback resolution.
- `compiler/mcp/src/tools/compile.rs` — after codegen at `compile.rs:140`, call `runner::build_symbol_map(library, context, &container)` and pass into `CachedContainer::new`. Library and context are already in scope (`compile.rs:133-134`).
- `compiler/mcp/src/server.rs` — register `run` with `#[tool]` mirroring the `compile` block at lines 144–161; `async`, `Parameters<RunInput>`, dispatches to `tools::run::build_response`.
- `compiler/mcp/tests/cli.rs` — add a `mcp_compile_then_run` helper (the existing single-call helper at line 122 doesn't fit two-step flows) and integration cases.

### Key design decisions

1. **Symbol map** (REQ-ARC-070, REQ-ARC-020/021). The cache currently stores no name → `VarIndex` resolution. Build the map at compile time by walking `SemanticContext::symbols().get_programs()` and `get_variables_in_scope(Global)` (same iteration as `tools/project_io.rs::collect_io` lines 102–123) and pairing each variable's bare name with the matching entry in `container.debug_section.var_names` (which carries `var_index` and `iec_type_tag`). Two-tier resolver: prefer exact qualified-name hit; fall back to bare-name lookup with global-vs-program disambiguation per REQ-ARC-020. Ambiguous → `P8001` diagnostic, run does not start (REQ-ARC-021).

2. **Fuel limit** (REQ-ARC-030/033) — **flagged spec deviation.** `Vm::run_round` takes only `current_time_us`; there is no per-instruction fuel budget API. MVP uses an **between-rounds approximation** via `Vm::profile().total()` (analogous to how REQ-ARC-035 already permits between-cycle wall-clock checks). Document the gap in `runner.rs`. Spec-conformant fix is a separate VM PR adding `set_fuel_budget` / `Trap::OutOfFuel`.

3. **Trace value formatting**. Reuse the type-tag dispatch from `vm-cli/src/cli.rs::format_variable_value` lines 250–277, but emit typed JSON per REQ-TOL-043 (BOOL→bool, 32-bit ints→number, LINT/ULINT→string for 64-bit precision, REAL/LREAL→number with `"NaN"`/`"Infinity"` strings for non-finite, TIME→`"T#Nms"`). For MVP, return `null` for `STRING`/`WSTRING`/`DATE`/structs/arrays — Phase 11 fills these in.

4. **`container_base64` input.** Defer to Phase 11. The MVP can't easily synthesize a `VariableSymbolMap` without re-running the analyzer on sources we don't have. Document in the tool description.

5. **Error surface**. Every failure (unknown `container_id`, unresolved variable, exceeded duration, VM trap) returns `ok: false` with diagnostics — never an MCP-layer error (REQ-TOL-024). `ok: true` only when `terminated_reason == "completed"` (REQ-TOL-047).

### Inner execution loop sketch

```text
loop {
    if simulated_us / 1000 >= max_duration_ms      → "duration",   break
    if wall_start.elapsed() >= max_wall_clock_ms   → "wall_clock", break
    if profile.total() >= max_fuel                 → "fuel",       break  (approximation)
    if samples.len() >= max_samples                → "sample_cap", truncated, break

    let current_us = max(simulated_us, running.next_due_us().unwrap_or(simulated_us + 1000));
    match running.run_round(current_us) {
        Ok(()) => { /* compare bufs.tasks[i].scan_count delta to detect which tasks ran;
                       emit one trace entry per ran-task with read_variable_raw values */ }
        Err(ctx) => { let faulted = running.fault(ctx);
                      diagnostic from faulted.trap();
                      "error"; drain final_values from faulted; break }
    }
    simulated_us = current_us + 1;
}
let stopped = running.stop();
final_values = read_variable_raw for every var in effective trace set
completed_cycles = bufs.tasks zip cached.tasks → (name, scan_count)
```

The pattern mirrors `vm-cli/src/cli.rs:58-93`, replacing the wall-clock `thread::sleep` with simulated-time advancement driven by `next_due_us()`. Task-name lookup uses the index alignment between `bufs.tasks[i]` and `cached.tasks[i]`.

## Verification

**Unit tests** in `tools/run.rs::tests` (mirror the `compile.rs` test layout — `make_cache()` helper, compile first to populate cache, then exercise `run`):
- happy path: simple cyclic counter program, `duration_ms: 500`, 100ms task → ~5 trace entries, increasing values, `terminated_reason == "completed"`
- unknown `container_id`, missing/double `container_id`+`container_base64`, unresolved variable, ambiguous bare name, wildcard rejected, too many traced variables, `limits` override that loosens, stimuli supplied (Phase 11 guard), non-`every_cycle` mode (Phase 11 guard)
- limit enforcement: each of `"duration"`, `"wall_clock"`, `"fuel"`, `"sample_cap"`, `"error"` (divide by zero) → correct `terminated_reason`, `ok: false`
- value codec: `value_from_raw` for BOOL true/false, REAL NaN → `"NaN"` string, LINT max → string, basic ints → number
- name resolver: qualified hit, bare-global single-resource hit, ambiguous → diagnostic
- annotate with `#[spec_test(REQ_TOL_040)]` etc. so the existing `spec_conformance.rs` harness picks them up

**CLI integration tests** in `tests/cli.rs`: add a `mcp_compile_then_run` helper (extracts `c_<n>` from compile response and substitutes into the run call), then cover unknown id, simple counter happy path, unresolved variable, duration-zero edge case.

**End-to-end smoke**:
```bash
cd compiler && cargo build -p ironplc-mcp
# Hand-craft a JSON-RPC sequence (initialize, initialized, compile, run) piped to the binary
echo '...' | cargo run -p ironplc-mcp --bin ironplcmcp
```
Cross-check `final_values` against `vm-cli run --dump-vars -` on the same compiled `.iplc`; the two should agree.

**Full CI gate** (CLAUDE.md requires this before any PR):
```bash
cd compiler && just
```
This runs `cargo fmt --check`, `cargo clippy -D warnings`, all crate tests, and the spec-conformance harness.

## Critical Files

- `compiler/mcp/src/tools/run.rs` — **new**, handler + input/response + resolver
- `compiler/mcp/src/runner.rs` — **new**, VM lifecycle + symbol map builder + value codec
- `compiler/mcp/src/cache.rs` — extend `CachedContainer` with `VariableSymbolMap`
- `compiler/mcp/src/tools/compile.rs` — populate symbol map after codegen (line 140)
- `compiler/mcp/src/server.rs` — register `#[tool] async fn run`
- `compiler/mcp/Cargo.toml` — add `ironplc-vm` with `features = ["profiling"]`
- `compiler/mcp/tests/cli.rs` — two-step compile-then-run helper + cases

## References

- Design: `specs/design/mcp-server.md` §`run` (lines 457–540), §Variable Naming (583–597), §VM Sandboxing (599–625), §Container Cache (571–581)
- Master plan: `specs/plans/2026-04-14-mcp-server-plan.md` Phases 10 & 11 (lines 399–469)
- Reference impl: `compiler/vm-cli/src/cli.rs` (`run` lines 26–102, `format_variable_value` lines 250–277)
- VM API: `compiler/vm/src/vm.rs` (`Vm`, `VmRunning::run_round`, `read_variable_raw`, `next_due_us`, `stop`, `fault`)
- Closest analogue: `compiler/mcp/src/tools/compile.rs` (response shape, cache use, diagnostic surface)
- I/O classification to reuse for Phase 11 stimulus validation: `compiler/mcp/src/tools/project_io.rs::classify` (lines 125–162)
