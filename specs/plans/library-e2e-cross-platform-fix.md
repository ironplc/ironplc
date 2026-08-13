# Fix the Cross-Platform Library Installer E2E Test

## Context

[Deployment run 31690344796](https://github.com/ironplc/ironplc/actions/runs/31690344796)
was the first deployment to run the library installer end-to-end test, wired in
by #1359. Two of its four legs failed, so `publish-website` and everything
downstream of it was skipped and version 0.237.0 never shipped.

The legs are:

| Leg | Installer | Result |
|-----|-----------|--------|
| `library-e2e` (ubuntu-latest) | tarball + `install.sh` | pass |
| `library-e2e` (macos-latest) | tarball + `install.sh` | pass |
| `library-e2e` (windows-2025) | NSIS | **fail** |
| `library-e2e-brew` (macos-latest) | Homebrew formula | **fail** |

Neither leg had ever been green: #1359 wired recipes into the deployment that
had only ever been run by hand on Linux/macOS tarballs.

A third job in the same run, `OpenCode Integration E2E`, also failed. That
failure is unrelated to this change (see "Out of scope" below).

### Failure 1 — Windows: PowerShell array truthiness

The Windows leg *did* produce the correct result and then failed the assertion:

```
PI: 3.141592653589793 circumference: 62.83185307179586
... FAIL: VM did not compute 2 * PI * 10.0 from the library PI
```

`ironplcvm --dump-vars` prints one line per variable, and the program pulls in
the library's `PI` global, so there are two lines. In PowerShell, `&cmd` captures
multi-line output as an **array of strings**, and `-notmatch` against an array is
a *filter*, not a boolean: it returns every element that does not match. The
`PI:` line does not contain `62.8318`, so the filter returns a one-element array,
which is truthy, so the `if` body ran and failed the test.

The single-line case would have worked, which is why the recipe looked correct
when it was written by hand.

The `sh` implementation of the same assertion (`grep -q`) has no such failure
mode — it is line-oriented by construction. The bug exists only because the
assertions were written **twice**, once per shell.

### Failure 2 — macOS Homebrew: formulae must live in a tap

```
Homebrew requires formulae to be in a tap, rejecting:
  /tmp/ironplc-e2e.rb
```

`brew install --formula <path>` for a file outside a tap is no longer supported.
The recipe filled the formula template into `/tmp` and installed it from there.

## Goal

1. Make all four legs pass.
2. Reduce the number of places a platform-specific bug can hide, so that a green
   Linux run is meaningful evidence about Windows and macOS.

## The simplification

The honest split of this test is:

* **Acquire + install** — genuinely different per OS (`install.sh` unpacks a
  tarball into `~/.ironplc`, NSIS installs into `%LOCALAPPDATA%`, Homebrew
  installs into `libexec` and symlinks into a keg `bin`). This difference *is*
  the thing under test and cannot be collapsed.
* **Compile, run, assert** — identical everywhere. Same fixtures, same flags,
  same expected output. Only the two binary paths differ.

Today the second half is implemented twice: once as `sh`
(`_library-e2e-run-installed`) and once as PowerShell (`library-e2e-test`).
That duplication is the entire reason Linux passing said nothing about Windows.

**The fix is to have exactly one implementation of the verification, in POSIX
`sh`, and run it on all three platforms.** GitHub's Windows runners ship Git for
Windows, so `bash` is on `PATH` — it is what `shell: bash` already uses in
Actions, and what every developer with Git for Windows has.

Moving the verification out of the justfile and into a checked-in script
(`tests/e2e/library/verify.sh`) rather than a `sh`-shebang recipe is deliberate:
a `just` recipe body is still interpreted by `just`'s per-OS shell selection
(`set windows-shell`, `[unix]`/`[windows]` attributes), whereas a plain script
invoked as `bash tests/e2e/library/verify.sh <ironplcc> <ironplcvm>` is the same
bytes run by the same interpreter everywhere. The per-OS recipes shrink to a
single line each whose only job is to name the two binary paths.

Two path rules keep that script genuinely portable:

* **Everything the script hands to a native binary is a relative path.** The
  script `cd`s to the repository root (derived from its own location) and uses
  `tests/e2e/library/uses_pi.st` and `target/library-e2e/pi.iplc`. Absolute
  POSIX paths such as `mktemp -d`'s `/tmp/tmp.XXXX` are meaningless to a native
  Windows `.exe` and only survive today by Git Bash's implicit path
  translation — a mechanism worth not depending on.
* **Only the two binary paths are absolute**, and the Windows recipe writes them
  with forward slashes so no backslash escaping is involved.

The one line of PowerShell that must remain is exit-code propagation
(`if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }`), so a failing script cannot
be reported as a passing job. That is process plumbing, not test logic.

### What this buys

After the change the only per-OS logic left in this test is the installer
invocation itself: three commands, no assertions, no output parsing, no control
flow. A bug in the *verification* now fails on Linux too. A bug in the
*installation* is by definition platform-specific and cannot be collapsed — but
it is now the only place to look.

## Plan

### 1. `tests/e2e/library/verify.sh` (new)

POSIX `sh`, `set -eu`, takes `<ironplcc> <ironplcvm>`. Derives the repository
root from `$0` and `cd`s there so all other paths are relative. For each of the
two fixtures: compile with `--dialect twincat --library <lib>`, run one scan with
`--scans 1 --dump-vars -`, echo the dump, and `grep -q` the expected results
(`62.8318`; `okTrue: TRUE` and `okFalse: TRUE`). Distinct failure messages for
compile failure, run failure, and wrong result, so a red run says which stage
broke. Outputs go to `target/library-e2e/` (already git-ignored).

### 2. `justfile`

* Delete `_library-e2e-run-installed`; both `sh` call sites move to the script.
* `[unix] library-e2e-test` → one line invoking the script with
  `$HOME/.ironplc/bin/{ironplcc,ironplcvm}`.
* `[macos] library-e2e-brew-test` → one line invoking the script with
  `$(brew --prefix ironplc)/bin/{ironplcc,ironplcvm}`.
* `[windows] library-e2e-test` → one line invoking the script through `bash`
  with `%LOCALAPPDATA%/Programs/IronPLC Compiler/bin/{ironplcc,ironplcvm}.exe`
  (forward slashes via `replace(env_var('LOCALAPPDATA'), '\', '/')`), plus
  `$LASTEXITCODE` propagation. All PowerShell assertions are deleted.
* `[macos] library-e2e-brew-download` → create a throwaway local tap with
  `brew tap-new --no-git ironplc/e2e`, write the filled-in formula to that tap's
  `Formula/ironplc.rb`, and `brew install --formula ironplc/e2e/ironplc`.
  Untap/uninstall first so the recipe is re-runnable locally. The tap name
  (`ironplc/e2e`) is deliberately not the published tap (`ironplc/brew`).

### 3. `tests/e2e/library/README.md`

Document the shared script, why it is a script rather than a recipe, and drop
the stale "intentionally not wired into the release pipeline yet" note.

## Validation

The recipes cannot be exercised on Linux alone, and `just --dry-run` does not
evaluate `[windows]` recipe bodies on Linux. `partial_library_e2e.yaml` already
has a `workflow_dispatch` trigger for exactly this reason, so validation is:
push the branch, dispatch the workflow against it with a release version that
ships both libraries (0.237.0), and require all four legs green before merge.

Locally on Linux: `sh tests/e2e/library/verify.sh` against an installed
toolchain, plus `just --evaluate` / `just --summary` to confirm the justfile
still parses.

## Out of scope

`OpenCode Integration E2E` failed in the same run because Ollama's `llama-server`
took a **segmentation fault while warming up** `qwen2.5:1.5b`, before any IronPLC
code ran:

```
Load failed ... error="llama-server process has terminated: signal: segmentation fault (core dumped)"
[GIN] ... | 500 | 20.25s | POST "/v1/chat/completions"
```

That leg passed in the previous deployment and involves no code touched here;
`ai-action/setup-ollama` installs whatever Ollama release is current, so the
likely cause is an upstream Ollama regression on the runner's CPU (the log shows
the AMX backend in use). It is tracked separately — mixing it into this change
would make it impossible to tell which fix made the deployment green.

A `workflow_dispatch` re-run of that leg on this branch
([31700029937](https://github.com/ironplc/ironplc/actions/runs/31700029937))
confirms the instability is in the Ollama/model layer and shows a *second*
failure mode. This time `llama-server` did not crash — the pre-flight probe
passed — and the run instead **hung** in layer 3:

```
12:27:27 Attempt 1/3: asking the agent to call ironplc_check...
(no further output; llama-server still alive; cancelled manually)
```

So the real-agent layer fails two different ways (crash, hang) against the same
pinned model, and `npm run agent-e2e` has no timeout wrapper — unlike the Ollama
startup steps, which use `run_timeout`. A hang there pins a runner until the
6-hour job limit and blocks the release with no diagnostic output.

Three independent decisions are worth making deliberately rather than folding
into this change:

1. **Bound it.** Wrap `npm run agent-e2e` in `run_timeout` so a hang becomes a
   prompt, legible failure. Unambiguously correct, but on its own it converts a
   hang into a red deployment rather than a green one.
2. **Pin Ollama.** `ai-action/setup-ollama` is SHA-pinned but installs the
   *latest* Ollama, so the toolchain under test changes without a commit. A
   version pin makes the leg reproducible.
3. **Reconsider the gate.** A nondeterministic local-LLM test currently gates
   `publish-website` and therefore the whole release. Layers 1 and 2 (the
   connectivity smoke and the mock tool-call gate) are deterministic and worth
   gating on; layer 3 may be better as a non-gating signal.
