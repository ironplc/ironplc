# Plan: Rename `ironplcdap` to `ironplcvmd`

Issue: [#1399](https://github.com/ironplc/ironplc/issues/1399)
Design doc: [Debugger Support](../design/debugger-support.md)

## Goal

Rename the debug server binary from `ironplcdap` to `ironplcvmd`, so the name
describes what the program *is* — the virtual machine's debug daemon — rather
than the protocol it happens to speak. Rename the override setting and
environment variable with it, and remove the duplication that made the rename a
30-file edit in the first place.

## Context

`ironplcdap` names the Debug Adapter Protocol. The protocol is an
implementation detail of how the extension talks to the server; the program is
the VM under a debugger. `ironplcvmd` sits beside `ironplcvm` the way a daemon
sits beside its command, and it survives a protocol change.

Per the issue, there is **no compatibility fallback**: the old setting and
environment variable stop being read. The setting shipped in v0.235.0-v0.239.0,
but nobody has set it, so this is a clean rename with no upgraders to carry: the
docs describe only the new names.

### The duplication problem

The name appears 154 times across 32 files. Most of that is prose and is
harmless. What matters is the set of places where a *load-bearing* copy of the
string lives, because each one is a place a future rename can silently miss:

| Layer | Load-bearing copies | Cross-checked today? |
|-------|--------------------|----------------------|
| `compiler/vm-cli/Cargo.toml` `[[bin]]` | 1 (canonical) | — |
| `compiler/justfile` `binaries` | 1 | Yes — `shipped_binaries_guard.rs` |
| `compiler/setup.nsi` | 1 | Yes — same guard |
| `compiler/homebrew/Formula/ironplc.rb` | 2 | Yes — same guard |
| `compiler/vm-cli/tests/dap.rs` `cargo_bin!` | 1 | No, but fails loudly at test time |
| `docs/reference/runtime/<name>.rst` | 1 (the slug) | **No** |
| VS Code extension source | 3 (binary, env var, setting key) | **No** |
| VS Code `package.json` setting id | 1 | **No** |
| VS Code `problem-codes.csv` E0007 text | 1 | **No** |
| VS Code unit tests | ~8 hand-typed literals | **No** |

The packaging row is already solved: `shipped_binaries_guard.rs` recovers the
binary set from each manifest and asserts all four agree, so a rename that
misses an installer fails CI. That is the pattern to extend, not replace.

The genuinely delicate coupling is the one nothing checks: **the extension
hand-types the compiler's binary name.** Rename the `[[bin]]` target and the
extension still compiles, still passes its tests, and fails only at runtime as
E0007 on a user's machine.

## Architecture

Two moves, in addition to the rename itself.

**1. One constant per name in the extension.** `debugAdapterLogic.ts` already
holds `CONTAINER_EXTENSION` as an exported single source of truth for a string
the extension shares with the server. The three debug-server names join it:

```ts
export const DEBUG_SERVER_BINARY = 'ironplcvmd';
export const DEBUG_SERVER_ENV_VAR = 'IRONPLCVMD';
export const DEBUG_SERVER_PATH_SETTING = 'debugServerPath';
export const DEBUG_SERVER_PATH_SETTING_ID = `${CONFIG_SECTION}.${DEBUG_SERVER_PATH_SETTING}`;
```

`debugAdapter.ts`, `findDapServerPath`, the E0007 hint text, and every unit
test read the constants instead of retyping the string. The hint text itself
becomes `debugServerNotFoundHint()` — pure logic in the logic module, so it is
testable and has exactly one copy.

**2. Guards for the copies that cannot import a constant.** A manifest, a CSV,
and a Cargo target are not TypeScript, so they get tests that compare them to
the constant rather than derive from it:

- `package.json` declares exactly `DEBUG_SERVER_PATH_SETTING_ID`, and its
  description names `DEBUG_SERVER_BINARY`.
- The E0007 message names `DEBUG_SERVER_BINARY`.
- **`compiler/vm-cli/Cargo.toml` declares a `[[bin]]` named
  `DEBUG_SERVER_BINARY`.** This is the weld that does not exist today. It turns
  a silent runtime break into a failing unit test in the same repo.
- `shipped_binaries_guard.rs` gains: every built binary has a reference page at
  `docs/reference/**/<name>.rst`. All four binaries already do; the guard keeps
  the docs slug from drifting from the binary it documents.

Prose copies (`.rst` body text, doc comments, spec history) are left as plain
text. Substituting them would cost readability for no safety — a stale word in
a sentence is a documentation bug, not a broken install, and the doc-page guard
catches the one prose copy that is actually addressable (the slug).

## File map

**Compiler**
- `compiler/vm-cli/Cargo.toml` — `[[bin]]` name and its comment
- `compiler/vm-cli/src/dap_main.rs`, `src/dap/mod.rs`, `src/dap/problem_codes.rs`, `src/error.rs`
- `compiler/vm-cli/tests/dap.rs` — `cargo_bin!` target and test names
- `compiler/justfile` — `binaries`
- `compiler/setup.nsi` — `DAPFILE` → `VMDFILE`
- `compiler/homebrew/Formula/ironplc.rb`
- `compiler/test/tests/shipped_binaries_guard.rs` — fixtures, plus the new
  docs-page guard

**Extension**
- `integrations/vscode/src/debugAdapterLogic.ts` — new constants, `debugServerNotFoundHint`
- `integrations/vscode/src/debugAdapter.ts`, `src/extension.ts`
- `integrations/vscode/package.json` — setting id and description
- `integrations/vscode/resources/problem-codes.csv` — E0007 text
- `integrations/vscode/src/test/unit/debugAdapterLogic.test.ts` — use constants; new name guards
- `integrations/vscode/src/test/unit/problems.test.ts`

**Docs**
- `docs/reference/runtime/ironplcdap.rst` → `docs/reference/runtime/ironplcvmd.rst`
- `docs/reference/runtime/index.rst`, `docs/reference/editor/debugging.rst`,
  `docs/reference/editor/settings.rst`, `docs/reference/editor/problems/E0007.rst`,
  `docs/how-to-guides/getting-started/debug-a-program.rst`,
  `docs/how-to-guides/troubleshoot-editor.rst`,
  `docs/explanation/ironplc-ecosystem.rst`

**Specs**
- `specs/design/debugger-support.md` and the DAP plans under `specs/plans/`

## Tasks

- [x] Commit this plan
- [x] Rename the binary across the compiler and packaging manifests
- [x] Rename the docs page and update the referring pages
- [x] Introduce the extension name constants and route all uses through them
- [x] Add the manifest / CSV / Cargo-target guards
- [x] Add the docs-page guard to `shipped_binaries_guard.rs`
- [x] Update the design doc and DAP plans
- [x] `cd compiler && just`; extension `npm run pretest && npm run test:unit`
