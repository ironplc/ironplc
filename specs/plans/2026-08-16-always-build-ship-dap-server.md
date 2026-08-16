# Plan: Always build and ship the DAP server

## Context

`ironplcdap` was declared with `required-features = ["dap"]`, and the `dap`
feature is off by default. `just compile` is a bare `cargo build`, so every
local and release build silently skipped the binary. A developer who pointed
the VS Code extension at `compiler/target/debug` (via `ironplc.path`, which the
extension also uses as the DAP-server directory — `extension.ts:299`) got
whatever `ironplcdap` happened to be left in that directory from the last time
someone explicitly passed `--features ironplc-vm-cli/dap`.

Observed: `target/debug/ironplcdap` two weeks older than `ironplcc`/`ironplcvm`
in the same directory, predating the continuous run loop (#1304), single
stepping (#1305), and the Layer-1 debug-info swap (#1364). The debugger
appeared broken; the code was fine.

The feature gate's stated rationale — "so the production `ironplcvm` binary
does not pull in the DAP layer" — does not hold:

- `mod dap;` is declared only in `dap_main.rs`. `main.rs` declares `cli`,
  `error`, `logger`. Separate `[[bin]]` targets are separate compilation units,
  so `ironplcvm` never compiles the DAP layer with or without the feature.
- `serde` with `derive` is already an unconditional dependency of `mcp`,
  `ironplc-cli`, and `playground` at the same feature set, so making it
  unconditional in `vm-cli` adds no compilation.

The gate's only observable effects were the stale binary and making the DAP
integration tests opt-in (`tests/dap.rs` is `#![cfg(feature = "dap")]`, so a
plain `cargo test` skipped them).

### The second half of the bug

Removing the gate fixes "not built". It does not fix "not shipped": no release
path ever produced `ironplcdap`, and the binary set is hardcoded in four places
that nothing cross-checks.

| Location | Lists |
|----------|-------|
| `justfile` `_package-macos` tar | `ironplcc ironplcvm ironplcmcp` |
| `justfile` `_package-linux` tar | `ironplcc ironplcvm ironplcmcp` |
| `setup.nsi` `!define`s + `File` lines | `APPFILE` / `VMFILE` / `MCPFILE` |
| `homebrew/Formula/ironplc.rb` | `libexec.install` + 3 symlinks |

Adding a binary means remembering all four, and `just` passes if you forget.
The extension's last-resort DAP discovery looks next to the installed
`ironplcc`, so every normally-installed user currently hits
`DebugServerNotFound`.

## Goals

1. `cargo build` produces `ironplcdap` on every platform, with no feature flag.
2. Every installer and package ships `ironplcdap` alongside the other binaries.
3. A binary that is built but not shipped fails CI, so this class of drift
   cannot recur silently.

## Non-goals

- Extracting the DAP server into its own crate. It would match the
  one-crate-per-shipped-binary shape of `ironplc-cli` and `mcp`, but it does not
  prevent this bug — the guard in goal 3 does — and it is churn on a working
  layout.
- Any change to DAP server behaviour, the VS Code extension, or the debugger
  itself.

## Design

### Part 1 — remove the feature gate

| File | Change |
|------|--------|
| `vm-cli/Cargo.toml` | Drop `required-features` from the `ironplcdap` bin, drop the `[features]` block, make `serde` non-optional. Replace the stale comment with why the binary is unconditional. |
| `justfile` | Drop `--features ironplc-vm-cli/dap` from `test`, `coverage`, `format`, `lint`. |
| `vm-cli/tests/dap.rs` | Drop `#![cfg(feature = "dap")]` so the DAP integration tests run under a plain `cargo test`. |
| `vm-cli/src/dap_main.rs` | Drop the "Feature-gated behind `dap`" sentence. |

### Part 2 — ship it

`justfile` gains a single `binaries` variable used by both tar recipes, so the
two Unix lists become one. `setup.nsi` gains a `DAPFILE` define and its `File`
line. The Homebrew formula gains the install entry and the symlink. Each of the
four now lists `ironplcdap`.

The NSIS uninstall section is `RMDir /r $INSTDIR`, so there is no per-file
delete list to update.

### Part 3 — guard against recurrence

New `compiler/test/tests/shipped_binaries_guard.rs`, following the existing
`spec_conformance_guard.rs` in the same directory: a workspace-level guard that
recovers both sides from files already in the tree, so there is no new manifest
to keep in sync, and with no new dependencies (hand-rolled parsers, each
fixture-tested).

Sets recovered:

- **Built**: `[[bin]]` names from each `members` entry of `compiler/Cargo.toml`.
- **Packaged (Unix)**: the `binaries := "…"` assignment in `justfile`.
- **Packaged (Windows)**: `!define X "<name>${EXTENSION}"` resolved through the
  `File "${ARTIFACTSDIR}\${X}"` lines in `setup.nsi`, so a define that is never
  installed does not count as shipped.
- **Packaged (Homebrew)**: `libexec.install` arguments and
  `bin.install_symlink libexec/"…"` targets, checked separately — a binary
  installed but not symlinked is not on the PATH.

The guard asserts all four sets are equal, and reports the specific missing
names per manifest.

## Testing

- `compiler/test`: the new guard, plus fixture tests for each parser.
- `vm-cli/tests/dap.rs` now runs unconditionally — it is the regression test for
  Part 1, since it can only build if `ironplcdap` is built by default.
- Manual: confirm `cargo build` alone produces `target/debug/ironplcdap`.
- `cd compiler && just` must pass (compile, coverage ≥ 85%, lint, dupes).

## Out of scope / follow-ups

- Smoke-testing the packaged `ironplcdap` in `verify-package` (it currently
  execs only `ironplcc` and `ironplcvm` via `tests/e2e/library/verify.sh`,
  whose contract is library verification and takes exactly two binaries).
  Presence is guarded here; executing the shipped DAP bytes is a follow-up.
