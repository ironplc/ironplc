# DAP launch errors carry V-codes

## Goal

Make every `ironplcdap` `launch` failure carry a stable IronPLC **V-code**, so a
DAP client sees the same `V#### - message` surface the `ironplcvm` CLI already
emits (and can link to the runtime problem-code docs). Chosen approach:
`LaunchError` owns its own `v_code()` and renders `"V#### - message"` via
`Display` — it does **not** reuse `VmError` (whose `exit_code` field has no
consumer on the DAP path, where a failed request returns a response rather than
exiting the process).

## Current state

`compiler/vm-cli/src/dap/launch.rs` defines `LaunchError` with a `message()`
that returns bare human text (`"NoDebugInfo: …"`, `"MultiInstanceUnsupported:
…"`). The `ironplcdap` binary (`dap_main.rs`) does not compile `error.rs`, so it
has no access to the generated V-code constants. V-codes come from
`vm-cli/resources/problem-codes.csv` → `build.rs` → `io_codes.rs`
(`pub const NAME: &str = "V6xxx"`). Runtime traps already expose `Trap::v_code()`.

## Mapping

| `LaunchError` variant | V-code |
|---|---|
| `ContainerOpen` | reuse `V6001` (`FILE_OPEN`) |
| `ContainerRead` | reuse `V6002` (`CONTAINER_READ`) |
| `VmStartFailed` | reuse the wrapped `Trap`'s own `v_code()` (a `V4xxx`/`V9xxx`) |
| `ProgramArgMissing` | new `V6008` |
| `NoDebugInfo` | new `V6009` |
| `MultiInstanceUnsupported` | new `V6010` |

## Changes

1. **CSV** — add three rows to `vm-cli/resources/problem-codes.csv`:
   `V6008 LaunchNoProgram`, `V6009 LaunchNoDebugInfo`, `V6010 LaunchMultiInstance`.
   `build.rs` generates `LAUNCH_NO_PROGRAM` / `LAUNCH_NO_DEBUG_INFO` /
   `LAUNCH_MULTI_INSTANCE` automatically.
2. **Codes in the DAP bin** — add `dap/problem_codes.rs`
   (`include!(".../io_codes.rs")`, `#![allow(dead_code)]` for the constants the
   DAP path doesn't use) and `pub mod problem_codes;` in `dap/mod.rs`. The
   `ironplcvm` binary keeps reaching the same generated constants through
   `error.rs`; both are one CSV source of truth.
3. **`LaunchError`** — `VmStartFailed` becomes
   `{ v_code: &'static str, detail: String }`; add `v_code()`; implement
   `Display` as `"{v_code} - {message}"` (keep `message()` as the plain text,
   dropping the redundant `NoDebugInfo:` name prefix but keeping the
   spec-mandated `MultiInstanceUnsupported:` wording). `start_vm` records
   `ctx.trap.v_code()`.
4. **Server** — `dap/server.rs` sends `err.to_string()` (V-coded) instead of
   `err.message()` for the launch failure responses.
5. **Docs** — add thin `docs/reference/runtime/problems/V6008.rst` …`V6010.rst`
   mirroring the existing `V6xxx` pages, and list them in
   `docs/extensions/thin_problem_pages_allowlist.txt` (the index regenerates
   itself). Required because the problem-code extension `exit(1)`s on any CSV
   code lacking a page.
6. **Tests** — assert `v_code()` per variant + the `Display` format in
   `launch.rs`; update the `server.rs` and `tests/dap.rs` assertions to check
   the `V6009` / `V6010` codes.

## Verification

`cd compiler && just` green (compile, coverage ≥85%, clippy, fmt, dupes). Docs
build is separate; the new pages + allowlist entries keep it green under `-W`.
