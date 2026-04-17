# Plan: Linux Install Script

## Goal

Provide a one-liner install path for IronPLC on Linux and macOS that works without sudo, Homebrew, or a manual tarball download. AI agents (CI containers, cloud sandboxes) can't use tools they can't install, and Linux is currently the only tier-1 IronPLC target without a scripted installer. Match the ergonomics of `rustup`, `deno`, and `bun`.

End state:

```sh
curl -fsSL https://www.ironplc.com/install.sh | sh
```

downloads the latest release, verifies the SHA-256, installs `ironplcc` (and, if present, `ironplcvm` and `ironplcmcp`) into `$HOME/.ironplc/bin`, and adds that directory to the user's `PATH`.

## Architecture

Three pieces, wired together so a broken installer never reaches users:

1. **Script**: `compiler/install.sh` — POSIX `sh`, shellcheck clean. Source of truth lives next to the other packaging assets (`setup.nsi`, `homebrew/Formula/ironplc.rb`).
2. **Hosting**: the docs Sphinx build (`docs/justfile`) copies `compiler/install.sh` into `_build/install.sh`, so it is served from `https://www.ironplc.com/install.sh` via the existing `gh-pages` publish.
3. **CI gating**: a new workflow, `partial_install_script_test.yaml`, installs the script against a real release on Ubuntu and macOS. It is added to `deployment.yaml` as a dependency of `publish-website`, so a failing installer blocks publishing the docs site — and therefore never changes what `curl | sh` users receive. The same workflow runs on every PR via `integration.yaml` against the last published release, so regressions are caught pre-merge. `integration.yaml` also gains a `shellcheck` lint job.

Hosting on `ironplc.com` rather than `raw.githubusercontent.com/main/...` is the central design choice: it lets the smoke test gate the script's rollout, matching the rustup/deno/bun pattern.

## File Map

| File | Change |
|------|--------|
| `compiler/install.sh` | New: POSIX install script with platform detect, SHA-256 verification, idempotent PATH update |
| `justfile` (root) | New `install-script-smoke` recipe (Unix) that runs the installer, verifies binaries, and re-runs for idempotency |
| `docs/justfile` | `compile` recipe now copies `../compiler/install.sh` into `_build/install.sh` |
| `.github/workflows/partial_install_script_test.yaml` | New reusable workflow; matrix over `ubuntu-latest` and `macos-latest` |
| `.github/workflows/deployment.yaml` | New `install-script-smoke-test` job; added to `publish-website`'s `needs` |
| `.github/workflows/integration.yaml` | New `install-script-lint` (shellcheck) + `install-script-smoke` (last-release) jobs for PR CI |
| `docs/quickstart/installation.rst` | Linux tab: `curl \| sh` instructions; macOS tab: same as alternative to Homebrew; supported-platforms list includes Linux |
| `README.md` | Quick Start section leads with the one-liner |

## Design notes

- **Install location**: `$HOME/.ironplc/bin`, overridable via `IRONPLC_INSTALL` or `--install-dir`. No sudo needed; no conflict with Homebrew's `/opt/homebrew/bin` or `/usr/local/bin`.
- **Required vs. optional binaries**: `ironplcc` is required; `ironplcvm` and `ironplcmcp` are installed if present in the archive and skipped otherwise, so the script keeps working against older releases that predate those binaries.
- **Version selection**: accepts `IRONPLC_VERSION` / `--version` with or without a leading `v`. Otherwise looks up the `tag_name` from `/releases/latest`, with a fallback to parsing the `Location` header of the `/releases/latest` redirect for when the API rate-limits (anonymous, 60/hour shared per IP on CI).
- **Checksum**: compare the first whitespace field of `<artifact>.sha256` to a locally computed hash, case-insensitively. Avoids `sha256sum -c` vs `shasum -c` portability pitfalls across Linux and macOS.
- **PATH setup**: idempotent, delimited block (`# >>> ironplc >>>` / `# <<< ironplc <<<`) appended to whichever of `~/.bashrc`, `~/.bash_profile`, `~/.profile`, `~/.zshrc` (respecting `$ZDOTDIR`), and `~/.config/fish/config.fish` exist; `fish_add_path` for fish. `--no-modify-path` skips entirely (used by CI).
- **Unsupported platforms**: Linux aarch64 is not currently built by `partial_compiler.yaml`. The script detects it and exits with a clear error pointing at the issue tracker. Adding the target is tracked as a separate follow-up. Windows under Git Bash / MSYS / Cygwin gets a clean error pointing at the NSIS installer.

## Verification

Local:

- `shellcheck -s sh compiler/install.sh` (clean)
- `sh compiler/install.sh --install-dir /tmp/ironplc --no-modify-path` followed by `/tmp/ironplc/bin/ironplcc version` — expect `ironplcc version X.Y.Z`
- `IRONPLC_VERSION=v0.186.0 sh compiler/install.sh --install-dir /tmp/old --no-modify-path` — expect warnings about missing optional binaries but still succeeds (regression guard for older releases)
- Re-run the same command — expect "already installed" short-circuit
- `just install-script-smoke 0.201.0` — runs install + verify + reinstall + re-verify; exercises the MCP handshake

CI:

- `integration.yaml` runs `install-script-lint` and `install-script-smoke ""` (latest release) on every PR touching any repo file — catches regressions before merge.
- `deployment.yaml` runs `install-script-smoke-test` against the just-built version on Ubuntu + macOS; `publish-website` depends on it, so a failure prevents `install.sh` from being published to the docs site.

## Tasks

- [x] Write plan
- [x] `compiler/install.sh` with all supported flags and shellcheck clean
- [x] Root `justfile` `install-script-smoke` recipe (Unix + Windows stub)
- [x] `docs/justfile` copies the script into `_build`
- [x] `partial_install_script_test.yaml` reusable workflow
- [x] Wire into `deployment.yaml` with `publish-website` dependency
- [x] Wire lint + smoke jobs into `integration.yaml`
- [x] Update `docs/quickstart/installation.rst`
- [x] Update `README.md` Quick Start
- [x] Run `cd compiler && just` — passes
- [x] Commit and push

## Out of scope

- `aarch64-unknown-linux-musl` build target (requires compiler matrix + Homebrew formula changes; separate PR).
- Native package managers (apt, yum, apk).
- `uninstall.sh` / `ironplcc self uninstall` subcommand.
