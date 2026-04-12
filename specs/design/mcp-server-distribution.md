# MCP Server Distribution Design

## Overview

This document describes how the `ironplcmcp` binary is distributed to end users across Windows, macOS (Homebrew), and Linux. It is intentionally separate from the MCP server design (`specs/design/mcp-server.md`), which covers the server's tool surface and architecture. Distribution concerns — packaging, installer changes, Homebrew formula updates, and CI pipeline changes — are expected to evolve independently as new platforms and package managers are added.

## Background

The IronPLC project currently distributes two binaries:

- `ironplcc` — the compiler CLI
- `ironplcvm` — the VM CLI

These are bundled together in three distribution channels:

| Channel | Platform | Mechanism |
|---------|----------|-----------|
| Windows installer | Windows x86_64, aarch64 | NSIS `.exe` installer |
| Homebrew tap | macOS x86_64, aarch64 + Linux x86_64 | `ironplc/homebrew-brew` tap |
| GitHub Release tarball | macOS + Linux | `.tar.gz` downloaded directly |

The MCP server produces a third binary, `ironplcmcp`, that must be distributed alongside the existing two. Users who install IronPLC to get the VS Code extension or the CLI should automatically get the MCP server without any additional steps.

## Goals

- `ironplcmcp` is installed alongside `ironplcc` and `ironplcvm` on every supported platform.
- No new distribution channel is introduced; the existing three channels are extended.
- The binary name and install location follow the same conventions as the existing binaries.
- The Homebrew formula, NSIS script, and CI packaging recipes are the only files that need to change.

## Non-Goals

- Standalone MCP server packages (e.g. a separate Homebrew formula, a separate installer, or an npm/pip wrapper) are out of scope for this design. They may be addressed in a future design.
- Auto-update or version pinning for the MCP server is out of scope.
- Signing or notarization changes beyond what already applies to the other binaries are out of scope.

## Design

### Binary Name

**REQ-DST-001** The MCP server binary is named `ironplcmcp` on macOS and Linux, and `ironplcmcp.exe` on Windows. This follows the existing `ironplcc` / `ironplcvm` naming convention.

The name is already set in `compiler/mcp/Cargo.toml`:

```toml
[[bin]]
name = "ironplcmcp"
path = "src/main.rs"
```

No change is needed to the crate.

### Windows (NSIS Installer)

The NSIS script (`compiler/setup.nsi`) currently installs `ironplcc.exe` and `ironplcvm.exe` into `$INSTDIR\bin`. The MCP binary is added to the same directory.

**REQ-DST-010** The NSIS installer defines a `MCPFILE` constant analogous to `APPFILE` and `VMFILE`.

**REQ-DST-011** The NSIS installer copies `ironplcmcp.exe` from `ARTIFACTSDIR` into `$INSTDIR\bin` in the same `SetOutPath "$INSTDIR\bin"` block as the other two binaries.

**REQ-DST-012** The NSIS uninstaller removes `ironplcmcp.exe` as part of the `RMDir /r /REBOOTOK $INSTDIR` call that already removes the entire install directory. No additional uninstall step is required.

The `APPFILE` registry key (`REGPATH_APPPATHSUBKEY`) points to `ironplcc.exe` and adds `$INSTDIR\bin` to the `Path` value. Because all three binaries share the same `bin` directory, `ironplcmcp.exe` is automatically on the user's `Path` after installation. No additional registry entry is needed.

### macOS and Linux (Homebrew / tarball)

The `_package-macos` and `_package-linux` justfile recipes build a `.tar.gz` archive from the release binaries. The Homebrew formula's `install` block extracts the archive and copies the binaries into `bin`.

**REQ-DST-020** The `_package-macos` and `_package-linux` justfile recipes include `ironplcmcp` in the archive alongside `ironplcc` and `ironplcvm`.

**REQ-DST-021** The Homebrew formula's `install` block installs `ironplcmcp` into `bin` alongside the other two binaries.

After these changes, `brew install ironplc/brew/ironplc` installs all three binaries and all three are on the user's `PATH`.

### CI Pipeline

The `partial_compiler.yaml` workflow builds the binaries and packages them. The `cargo build --release` step already builds all workspace binaries, including `ironplcmcp`, because the `ironplc-mcp` crate is part of the workspace. The only changes needed are in the packaging steps.

**REQ-DST-030** The `just package` recipe (and its platform-specific variants `_package-windows`, `_package-macos`, `_package-linux`) include `ironplcmcp` / `ironplcmcp.exe` in the produced artifact.

**REQ-DST-031** The NSIS `MCPFILE` variable is passed to `makensis` in the same way as `APPFILE` and `VMFILE`. Because the binary is already built by `cargo build --release`, no additional build step is required.

**REQ-DST-032** The end-to-end smoke test in `partial_integration_test.yaml` verifies that `ironplcmcp` is present and responds correctly to a real MCP protocol exchange after installation on Windows. See the End-to-End Smoke Test section below for the full test design.

### End-to-End Smoke Test

The existing smoke test (`endtoend-smoke-test` in the root `justfile`) installs the compiler and runs `ironplcc help` to confirm the binary is present and executable. The MCP server cannot be tested the same way: it has no `--version` or `--help` flag — it speaks MCP JSON-RPC over stdio and exits when the client disconnects.

The appropriate test is therefore a minimal MCP protocol exchange: send a `tools/list` request on stdin and assert that the response contains the expected tool names. This is the lowest-cost test that proves (a) the binary is installed, (b) it starts without error, and (c) it speaks the MCP protocol correctly.

**REQ-DST-040** The end-to-end smoke test sends a `tools/list` JSON-RPC request to `ironplcmcp` via stdin and asserts that the response is valid JSON containing at least one tool entry. This confirms the binary is installed, starts, and speaks MCP.

**REQ-DST-041** The `tools/list` request requires the full MCP handshake: an `initialize` request, a `notifications/initialized` notification, then the `tools/list` request. All three are written to stdin as newline-separated JSON objects. The test reads all stdout output after stdin is closed, then checks for the expected tool name in the combined response.

**REQ-DST-042** The smoke test is implemented as a PowerShell script fragment in the `endtoend-smoke-test` justfile recipe, consistent with the existing Windows-only test approach. The script:
1. Starts `ironplcmcp.exe` with stdin/stdout redirected.
2. Writes the `tools/list` request JSON to stdin and closes stdin to signal end-of-input.
3. Reads all stdout output.
4. Asserts the output contains `"list_options"` (a tool that is always present).
5. Exits non-zero if the assertion fails.

The `tools/list` request body:
```json
{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}
```

**REQ-DST-043** The smoke test does not attempt to call `check` or `parse` end-to-end. Functional correctness of those tools is covered by the unit and integration tests in the `ironplc-mcp` crate. The smoke test's only job is to confirm the installed binary is present and starts correctly.

### Homebrew Formula Template

The formula template (`compiler/homebrew/Formula/ironplc.rb`) currently installs two binaries. After this change it installs three.

**REQ-DST-040** The Homebrew formula template adds `bin.install "ironplcmcp"` to the `install` block.

**REQ-DST-041** No new template variables are required. The formula downloads the same `.tar.gz` archive as before; the archive now contains the additional binary.

### Summary of File Changes

| File | Change |
|------|--------|
| `compiler/setup.nsi` | Add `MCPFILE` constant; add `File` directive for `ironplcmcp.exe` |
| `compiler/justfile` | Add `ironplcmcp` to `_package-macos` and `_package-linux` tar commands; add MCP smoke-test step to `endtoend-smoke-test` |
| `compiler/homebrew/Formula/ironplc.rb` | Add `bin.install "ironplcmcp"` |
| `.github/workflows/partial_compiler.yaml` | Pass `MCPFILE` to `makensis` |

No new crates, no new workflow files, and no new distribution channels are introduced.

## Future Work

- **Standalone MCP package**: Some users (e.g. those using the MCP server from a non-IronPLC IDE) may want to install only `ironplcmcp` without the compiler or VM. A separate Homebrew formula, a PyPI wrapper (`uvx`-compatible), or an npm package could address this.
- **ARM Linux**: The current Linux distribution targets `x86_64-unknown-linux-musl`. An `aarch64-unknown-linux-musl` target would follow the same pattern as the macOS aarch64 target.
- **Package manager integrations**: `winget`, `scoop`, `apt`/`deb`, and `rpm` are not currently supported. Adding `ironplcmcp` to those channels would follow the same "include alongside the other binaries" principle described here.
