# Plan: MCP Server Distribution

## Goal

Include `ironplcmcp` in the existing Windows, macOS (Homebrew), and Linux distribution channels so that users who install IronPLC automatically get the MCP server binary alongside `ironplcc` and `ironplcvm`.

## Design doc reference

- `specs/design/mcp-server-distribution.md` — requirements REQ-DST-001 through REQ-DST-041

## Architecture

The `ironplcmcp` binary is already built by `cargo build --release` because `ironplc-mcp` is part of the workspace. The only changes needed are in the packaging layer:

- **Windows**: add `ironplcmcp.exe` to the NSIS installer script
- **macOS / Linux**: add `ironplcmcp` to the `tar` commands in the justfile
- **Homebrew formula**: add `bin.install "ironplcmcp"` to the `install` block
- **CI**: pass the new binary name to `makensis` and add a smoke-test assertion

No new crates, no new workflow files, and no new distribution channels are introduced.

## File map

| File | Action |
|------|--------|
| `compiler/setup.nsi` | Add `MCPFILE` constant; add `File` directive for `ironplcmcp.exe` |
| `compiler/justfile` | Add `ironplcmcp` to `_package-macos` and `_package-linux` tar commands; add MCP smoke-test to `endtoend-smoke-test` |
| `compiler/homebrew/Formula/ironplc.rb` | Add `bin.install "ironplcmcp"` |

## Tasks

### Step 1: Windows NSIS installer (`compiler/setup.nsi`)

- [x] Add `!define MCPFILE "ironplcmcp${EXTENSION}"` alongside the existing `APPFILE` and `VMFILE` defines
- [x] Add `File "${ARTIFACTSDIR}\${MCPFILE}"` in the `SetOutPath "$INSTDIR\bin"` block, after the existing `VMFILE` line

### Step 2: macOS and Linux packaging (`compiler/justfile`)

- [x] In `_package-macos`: add `ironplcmcp` to the `tar` command alongside `ironplcc` and `ironplcvm`
- [x] In `_package-linux`: add `ironplcmcp` to the `tar` command alongside `ironplcc` and `ironplcvm`

### Step 3: Homebrew formula (`compiler/homebrew/Formula/ironplc.rb`)

- [x] Add `bin.install "ironplcmcp"` to the `install` block after the existing `bin.install "ironplcvm"` line

### Step 4: End-to-end smoke test (`justfile`)

The MCP server has no `--version` or `--help` flag — it speaks MCP JSON-RPC over stdio and exits when the client disconnects. The smoke test therefore performs a minimal real MCP protocol exchange rather than a simple process invocation check.

- [x] In the `endtoend-smoke-test` recipe (Windows PowerShell), after the existing `ironplcc help` check, add a block that:
  1. Pipes the `tools/list` JSON-RPC request to `ironplcmcp.exe` via stdin
  2. Reads all stdout output
  3. Asserts the output contains `"list_options"` (a tool always present in the server)
  4. Exits non-zero if the assertion fails

The PowerShell fragment to add:

```powershell
# Verify ironplcmcp is installed and speaks MCP by performing the required
# initialize handshake followed by a tools/list request, then checking that
# the response contains a known tool name.
$mcpBin = "$env:LOCALAPPDATA\Programs\IronPLC Compiler\bin\ironplcmcp.exe"
$mcpInput = @(
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"smoke-test","version":"0.1"}}}',
  '{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}',
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
) -join "`n"
$mcpResponse = $mcpInput | & $mcpBin
if ($mcpResponse -notmatch "list_options") {
  Write-Error "ironplcmcp did not return expected tools/list response. Got: $mcpResponse"
  exit 1
}
```

## Verification

1. Build the Windows installer locally (or via CI dry-run) and confirm `ironplcmcp.exe` is present in `$INSTDIR\bin` after installation
2. Build the macOS tarball and confirm `ironplcmcp` is in the archive: `tar -tzf <artifact>.tar.gz | grep ironplcmcp`
3. Build the Linux tarball and confirm `ironplcmcp` is in the archive
4. Run `brew install --build-from-source` against the updated formula and confirm `ironplcmcp` is on `PATH`
5. CI dry-run (`workflow_dispatch` with `dryrun: true`) passes all build and smoke-test steps including the MCP protocol exchange
