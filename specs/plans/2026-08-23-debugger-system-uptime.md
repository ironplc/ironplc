# Plan: Show the VM's uptime in the debugger's `Runtime` scope

## Context

[#1397](https://github.com/ironplc/ironplc/issues/1397) reports that the
debugger advances program time a flat 1 ms per scan instead of following the
task configuration, so `INTERVAL` has no effect while debugging and timers
elapse after a scan count unrelated to the program's configuration.

That bug is hard to see, let alone confirm a fix for, because **the debugger
shows no time at all**. The `Runtime` scope carries exactly one entry,
`scanCount` (`specs/plans/2026-08-16-dap-scan-count.md`), and the clock the
program actually runs against is invisible: a session can only be reasoned
about by counting scans and assuming what each one is worth.

The clock is already in hand. `VmRunning::inject_system_uptime`
(`compiler/vm/src/vm.rs`) receives `current_time_us` at the start of every scan
and writes it to `__SYSTEM_UP_TIME` / `__SYSTEM_UP_LTIME` — but **only** when
the container was compiled with `--allow-system-uptime-global`
(`FLAG_HAS_SYSTEM_UPTIME`). Without that flag the value passes through the VM
and is dropped, so a program that never opted into the globals gives an
observer nothing to look at, even though the VM knew the number.

This plan makes that number observable in the debugger regardless of the flag.
It does not change how time advances; that is #1397's own fix, which this
change exists to make verifiable.

## Goals

1. The `Runtime` scope carries a `systemUptime` entry, in milliseconds, next to
   `scanCount`.
2. The value is available whether or not the program declares the uptime
   globals — it is VM state, not program state.
3. The value is the one the *current* scan runs against, so it matches what
   `__SYSTEM_UP_LTIME` reads inside that scan when the globals are present.

## Non-goals

- Fixing #1397 (how the debug driver advances its clock). This change only
  makes the clock visible.
- Surfacing uptime outside the debugger (`ironplcvm` output, the LSP, the
  playground).
- Writable runtime values, or rendering the value as a `TIME` literal. The
  entry is a plain millisecond count, like `scanCount` is a plain cycle count.

## Design

### VM

`VmRunning` gains a `system_time_us` field: the clock value the most recently
*started* scan cycle runs against. `inject_system_uptime` is renamed
`set_system_time` and records the value **before** the `FLAG_HAS_SYSTEM_UPTIME`
check, so tracking is independent of whether the globals are written. Both
drivers already funnel through it — `run_round` once per round with ready
tasks, `run_round_debug` once per *fresh* scan — so the recorded value follows
scan starts, not calls: resuming a paused scan does not move it, which is what
makes the displayed value the one the paused code is reading.

A new accessor `VmRunning::uptime_ms() -> i64` returns
`(system_time_us / 1000) as i64`, the same arithmetic and the same signed
64-bit type the `__SYSTEM_UP_LTIME` injection uses, so the debugger and the
program cannot disagree about the number.

### DAP server

`runtime_variables_body` (`compiler/vm-cli/src/dap/server.rs`) appends a second
`Variable`:

| Name | Type | Value |
|------|------|-------|
| `systemUptime` | `LINT` | `running.uptime_ms()`, milliseconds |

`LINT` is the IEC 61131-3 spelling of the i64 the VM keeps, and matches the
type of `__SYSTEM_UP_LTIME`. `scanCount` stays first, so existing clients and
tests that read the scope's first entry are unaffected.

No protocol change: the entry rides the `variables` request the client already
re-issues at every stop, so it tracks execution with no polling.

## Testing

- `vm.rs`: `uptime_ms` is 0 before the first round; it reports the clock a
  round ran with when the container has **no** uptime globals; and it stays put
  across a resumed (paused) scan rather than following the resume's clock.
- `server.rs`: the `Runtime` scope reports `systemUptime` as `LINT`, and the
  value advances between consecutive stops.
- `docs/reference/editor/debugging.rst`: `Runtime` scope table gains the row.
- `specs/design/debugger-support.md`: §Scopes and the `scopes`/`variables`
  legality rows name both entries.
- `cd compiler && just`.
