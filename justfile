
set windows-shell := ["powershell.exe", "-c"]

# A quick check of the development environment
devenv-smoke:
  @just _devenv-smoke-{{os_family()}}

_devenv-smoke-windows:
  @"CHECK: compile the IronPLC compiler"
  cd compiler; just compile
  @"CHECK: compile VS code extension (does not include tests)"
  cd integrations\vscode; just setup; just compile
  @"CHECK: compile the docs"
  cd docs; just ci
  "SMOKE PASSED"

_devenv-smoke-unix:
  @echo "CHECK: compile the IronPLC compiler"
  cd compiler && just compile
  @echo "CHECK: compile VS code extension (does not include tests)"
  cd integrations/vscode && just setup && just compile
  @echo "CHECK: compile the docs"
  cd docs && just ci
  @echo "SMOKE PASSED"

# Capture screenshots of the VS Code extension for documentation.
# Requires ironplcc to be installed and a display available (macOS or Linux with Xvfb).
# Compiles the extension, captures screenshots, then copies PNGs to docs/images/screenshots/.
screenshots:
  cd integrations/vscode && just compile
  cd integrations/vscode && just screenshots
  mkdir -p docs/images/screenshots
  cp integrations/vscode/out/screenshots/output/*.png docs/images/screenshots/
  cp integrations/vscode/out/screenshots/output/quickstart-animation.png images/quickstart-animation.png
  cp integrations/vscode/out/screenshots/output/quickstart-animation.png integrations/vscode/images/quickstart-animation.png

# Simulate the workflow that runs to validate a commit (as best as is possible via Docker)
ci-commit-workflow:
  @just _ci-commit-workflow-{{os_family()}}
  "TIP - this only ran the Linux tests"

_ci-commit-workflow-windows:
  act --workflows ./.github/workflows/commit.yaml --env IRONPLC_INSTALL_DEPS=true

_ci-commit-workflow-unix:
  act --workflows ./.github/workflows/commit.yaml

# Simulate the workflow that runs to validate a commit (as best as is possible via Docker)
ci-publish-workflow:
  @just _ci-publish-workflow-{{os_family()}}
  @"TIP - this only ran the Linux tests"

_ci-publish-workflow-windows:
  act workflow_dispatch --workflows .\.github\workflows\publish.yaml --env IRONPLC_INSTALL_DEPS=true

_ci-publish-workflow-unix:
  act workflow_dispatch --workflows ./.github/workflows/publish.yaml --verbose

ci-update-dependencies-workflow:
  act workflow_dispatch --workflows ./.github/workflows/update.yaml --verbose

# Lint the GitHub Actions workflows with actionlint. Run this to verify any
# changes to files under .github/workflows/.
check-actions:
  actionlint

get-next-version type:
  #! /bin/bash
  RE='[^0-9]*\([0-9]*\)[.]\([0-9]*\)[.]\([0-9]*\)\([0-9A-Za-z-]*\)'

  step="{{type}}"
  if [ -z "{{type}}" ]
  then
    step=patch
  fi

  base=$(git tag --sort=v:refname 2>/dev/null| tail -n 1)

  MAJOR=`echo $base | sed -e "s#$RE#\1#"`
  MINOR=`echo $base | sed -e "s#$RE#\2#"`
  PATCH=`echo $base | sed -e "s#$RE#\3#"`

  case "$step" in
    major)
      let MAJOR+=1
      let MINOR=0
      let PATCH=0
      ;;
    minor)
      let MINOR+=1
      let PATCH=0
      ;;
    patch)
      let PATCH+=1
      ;;
  esac

  echo "$MAJOR.$MINOR.$PATCH"

# Sets the version number for all components. Must be a "bare" version number, such as 0.0.1 or 1.0.1.
version version:
  # We need this specific package to do the update
  @cargo install cargo-release
  @just _version-{{os_family()}} {{version}}

_version-windows version:
  @"Set version number to {{version}}"
  cd compiler; just version {{version}}
  cd integrations\vscode; just version {{version}}
  cd docs; just version {{version}}

_version-unix version:
  @echo "Set version number to {{version}}"
  cd compiler && just version {{version}}
  cd integrations/vscode && just version {{version}}
  cd docs && just version {{version}}

commit-branch authorname authoremail branch message:
  git config --global user.name "{{authorname}}"
  git config --global user.email "{{authoremail}}"
  git fetch origin main
  git checkout -b {{branch}} origin/main
  git commit -a -m "{{message}}"

commit-version authorname authoremail version:
  git config --global user.name "{{authorname}}"
  git config --global user.email "{{authoremail}}"
  git commit -a -m "Create version {{version}}"
  git tag -a "v{{version}}" -m "Create tagged release v{{version}}"

# Updates dependencies to latest versions
update:
  cd compiler && just update
  cd integrations/vscode && just update

# This is only valid for Windows hosts
e2e_fspath := env_var_or_default('USERPROFILE', '') + "\\.vscode\\extensions\\"
e2e_external := "file:///" + replace(replace(e2e_fspath, "\\", "/"), ":", "%3A")
e2e_path := "/" + replace(e2e_fspath, "\\", "/")
# I'm pretty sure justfile doesn't handle multiple \\ correctly, and that's
# what is needed for valid JSON - so do in two steps.
e2e_fspathesc := replace(e2e_fspath, "\\", "*")

# End to end "smoke" test.
[windows]
endtoend-smoke compiler-version compilerfilename extension-version extensionfilename extension-name:
  # There are two parts to IronPLC - the compiler and the extension
  # This test ensures that they actually work together (out of the box).
  # The test supports different versions of the extension and compiler to
  # check for compatibility between versions.
  #
  # extension-version: a semantic version number, such as "0.1.1"
  # compiler-version: a semantic version number, such as "0.1.1"
  # compilerfilename: the name of the compiler file in GitHub Releases
  # compilerfilename: the name of the compiler file in GitHub Releases
  @just endtoend-smoke-download v{{compiler-version}} {{compilerfilename}} v{{extension-version}} {{extensionfilename}}
  @just endtoend-smoke-test {{extension-version}} {{compiler-version}} {{extension-name}}

[windows]
endtoend-smoke-download compiler-release-tag compilerfilename extension-release-tag extensionfilename:
  Invoke-WebRequest -Uri "https://github.com/ironplc/ironplc/releases/download/{{compiler-release-tag}}/{{compilerfilename}}" -OutFile ironplcc.exe
  Invoke-WebRequest -Uri "https://code.visualstudio.com/sha/download?build=stable&os=win32-x64-user" -OutFile vscode.exe
  Invoke-WebRequest -Uri "https://github.com/ironplc/ironplc/releases/download/{{extension-release-tag}}/{{extensionfilename}}" -OutFile ironplc.vsix

[windows]
endtoend-smoke-test compiler-version extension-version extension-name:
  # Install the compiler
  Start-Process ironplcc.exe -ArgumentList "/S" -PassThru | Wait-Process -Timeout 60

  # Do a simple check that the application is runnable
  &"{{env_var('LOCALAPPDATA')}}\Programs\IronPLC Compiler\bin\ironplcc.exe" "help"

  # Install VS Code
  Start-Process vscode.exe -ArgumentList "/VERYSILENT /NORESTART /MERGETASKS=!runcode" -PassThru | Wait-Process -Timeout 600

  # Install the VS code extension
  
  # VS code does have a command line to install an extension, but after
  # many tries, I think it is broken, so instead, just install directly
  # Expands to a folder called "ironplc\extension"
  # Some versions of Expand-Archive only work with the ZIP file extension
  Copy-Item -Path "ironplc.vsix" -Destination "ironplc.zip"
  Expand-Archive ironplc.zip
  # Move the folder 
  New-Item -ItemType Directory -Force -Path "{{env_var('USERPROFILE')}}\.vscode\extensions\"
  Move-Item ironplc\extension "{{env_var('USERPROFILE')}}\.vscode\extensions\{{extension-name}}-{{extension-version}}"
  Get-ChildItem "{{env_var('USERPROFILE')}}\.vscode\extensions\{{extension-name}}-{{extension-version}}"

  # Create the extensions.json file that references this extension
  New-Item "{{env_var('USERPROFILE')}}\.vscode\extensions\extensions.json" -Force
  '[{"identifier":{"id":"{{extension-name}}"},"version":"{{extension-version}}","location":{"$mid":1,"fsPath":"{{e2e_fspathesc}}{{extension-name}}-{{extension-version}}","_sep":1,"external":"{{e2e_external}}{{extension-name}}-{{extension-version}}","path":"{{e2e_path}}{{extension-name}}-{{extension-version}}","scheme":"file"},"relativeLocation":"{{extension-name}}-{{extension-version}}","metadata":{"installedTimestamp":1695013253133}}]'.replace('*', '\\') | Set-Content "{{env_var('USERPROFILE')}}\.vscode\extensions\extensions.json"
  Get-Content -Path "{{env_var('USERPROFILE')}}\.vscode\extensions\extensions.json"

  # Create the settings.json with the configuration to enable trace level logging (that's the 4 -v's)
  # It would be better to use the temp directory, but that generates forward slashes that need to be escaped
  # and escaping them is a challenge. This avoid the problem.
  New-Item "{{env_var('APPDATA')}}\Code\User\settings.json" -Force
  Set-Content "{{env_var('APPDATA')}}\Code\User\settings.json" '{ "security.workspace.trust.enabled": false, "ironplc.logLevel": "TRACE", "ironplc.logFile": "C:\\ironplcc.log" }'
  Get-Content "{{env_var('APPDATA')}}\Code\User\settings.json"

  # Open an example file that is part of the compiler - this is a hard coded path
  # but that's also the point. We expect the installer to install here by default
  # so that the extension will find the compiler by default.
  Get-ChildItem "{{env_var('LOCALAPPDATA')}}\Programs\IronPLC Compiler\examples\"
  Start-Process "`"{{env_var('LOCALAPPDATA')}}\Programs\Microsoft VS Code\code.exe`"" -ArgumentList "`"{{env_var('LOCALAPPDATA')}}\Programs\IronPLC Compiler\examples\getting_started.st`""

  # Check that the log file was created (indicating that VS Code correctly started the
  # ironplcc language server). This path is a well-known path
  Start-Sleep -s 30
  Get-ChildItem "C:\\"

  # Verify ironplcmcp is installed and speaks MCP by performing the required
  # initialize handshake followed by a tools/list request, then checking that
  # the response contains a known tool name.
  # NOTE: each recipe line runs in a separate PowerShell process, so we cannot
  # pass variables between lines. Use just template expressions and inline values.
  Set-Content -Path "{{env_var('TEMP')}}\mcp-input.txt" -Value ('{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"smoke-test","version":"0.1"}}}' + "`n" + '{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}' + "`n" + '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}')
  $mcpResponse = (cmd /c """{{env_var('LOCALAPPDATA')}}\Programs\IronPLC Compiler\bin\ironplcmcp.exe""" "<" "{{env_var('TEMP')}}\mcp-input.txt") -join "`n"; if ($mcpResponse -notmatch "list_options") { Write-Error "ironplcmcp did not return expected tools/list response. Got: $mcpResponse"; exit 1 }

  IF (Test-Path "C:\\ironplcc.log" -PathType Leaf) { exit 0 } ELSE { exit 1 }

_endtoend-smoke-unix:
  @echo "endtoend-smoke is not implemented for Unix family"
  exit 1

# Install script smoke test - Unix only.
#
# Runs compiler/install.sh against a real GitHub release, verifies that the
# installed binaries run, then re-runs the installer (without clearing state)
# to confirm idempotency.
#
# compiler-version: empty to use the latest release; otherwise a bare version
#                   like "0.201.0" (without the leading "v").
[unix]
install-script-smoke compiler-version="":
  @just _install-script-smoke-clean
  @just _install-script-smoke-run "{{compiler-version}}"
  @just _install-script-smoke-verify
  @just _install-script-smoke-run "{{compiler-version}}"
  @just _install-script-smoke-verify

[unix]
_install-script-smoke-clean:
  rm -rf "$HOME/.ironplc"

[unix]
_install-script-smoke-run compiler-version:
  #!/usr/bin/env sh
  set -eu
  if [ -n "{{compiler-version}}" ]; then
    IRONPLC_VERSION="v{{compiler-version}}" sh ./compiler/install.sh --no-modify-path
  else
    sh ./compiler/install.sh --no-modify-path
  fi

[unix]
_install-script-smoke-verify:
  #!/usr/bin/env sh
  set -eu
  BIN="$HOME/.ironplc/bin"
  "$BIN/ironplcc" version
  "$BIN/ironplcc" help

  # Compatibility libraries ship beside the binaries so the loader finds them at
  # <bindir>/resources/libs. When present, verify a program reading the bundled
  # Tc2_System `PI` constant actually compiles -- this exercises the shipped
  # files, not the dev-tree fallback. The files are optional here because this
  # recipe also runs against the latest published release, which predates library
  # shipping; a release that ships them makes this a hard check on every later run.
  if [ -f "$BIN/resources/libs/Tc2_System/library.toml" ]; then
    _pi_src="$(mktemp -d)/pi.st"
    printf '%s\n' 'FUNCTION_BLOCK FB_Angle VAR d2r : LREAL := PI/180.0; END_VAR END_FUNCTION_BLOCK' > "$_pi_src"
    "$BIN/ironplcc" check --dialect twincat --allow-constant-initializer-expressions --library Tc2_System "$_pi_src"
  else
    echo "warning: compatibility libraries not installed (release predates library shipping); skipping PI check" >&2
  fi

  # ironplcvm and ironplcmcp are optional (older releases may not include them).
  if [ -x "$BIN/ironplcmcp" ]; then
    # MCP handshake: initialize -> notifications/initialized -> tools/list.
    # The response should contain a known tool name (list_options).
    printf '%s\n%s\n%s\n' \
      '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"smoke","version":"0.1"}}}' \
      '{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}' \
      '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
      | "$BIN/ironplcmcp" | grep -q list_options
  fi

[windows]
install-script-smoke compiler-version="":
  @echo "install-script-smoke is Unix-only; use endtoend-smoke on Windows"
  exit 1

# Library installer end-to-end test.
#
# Installs the published compiler with the *real* OS installer, then compiles AND
# runs programs that depend on the bundled compatibility libraries. This is the
# one test that proves the installer ships resources/libs beside the binary, the
# compiler resolves library symbols from the *installed* location (not the
# dev-tree fallback), and the whole toolchain (compile -> run) works on the
# shipped binaries. Each OS installs differently, so the install runs per OS:
#   [unix]      -- tarball + install.sh          -> $HOME/.ironplc/bin
#   [windows]   -- NSIS installer                -> %LOCALAPPDATA%\...\bin
#   [macos]     -- Homebrew formula (library-e2e-brew) -> libexec (symlinked)
#
# The *verification* does not differ per OS, and is deliberately not written per
# OS either: every leg below hands two binary paths to the single shared script
# tests/e2e/library/verify.sh. It used to exist twice -- once in sh, once in
# PowerShell -- and the copies drifted, so a green Linux run said nothing about
# Windows. Windows runs the
# same script through Git for Windows' bash. Keep new assertions in the script,
# never in a recipe.
#
# Structured like endtoend-smoke: the top recipe orchestrates a `-download`
# (acquire + install) step and a `-test` (compile + run + verify) step. Splitting
# them lets you reinstall once and re-run the verification repeatedly. Install
# lives in the download step because the Unix install.sh fuses fetch and install.
#
# Unlike install-script-smoke (which stays green against older releases), the
# library check here is a HARD assertion: this test exists to catch a release
# that fails to ship the libraries, so the target release must be one that does.
#
# Runs in deployment.yaml (via partial_library_e2e.yaml) alongside the other
# end-to-end tests. Also runnable manually, or from the Actions tab via
# partial_library_e2e.yaml's workflow_dispatch.
#
# compiler-version: a required, bare release version like "0.234.0" (no leading
#                   "v"). The version is always explicit -- never resolved to
#                   "latest" -- so a run always targets a known release.
[unix]
library-e2e compiler-version:
  @just library-e2e-download "{{compiler-version}}"
  @just library-e2e-test

# Download + install the published compiler via the tarball installer (install.sh).
[unix]
library-e2e-download compiler-version:
  @just _install-script-smoke-clean
  @just _install-script-smoke-run "{{compiler-version}}"

# Verify against the tarball layout: binaries and resources/libs in ~/.ironplc/bin.
[unix]
library-e2e-test:
  sh tests/e2e/library/verify.sh "$HOME/.ironplc/bin/ironplcc" "$HOME/.ironplc/bin/ironplcvm"

# macOS additionally ships through Homebrew, whose formula installs to libexec and
# symlinks the executables onto the PATH -- a different layout from the tarball.
# This variant tests that path. It is named separately so macOS can run both the
# tarball (library-e2e) and Homebrew (library-e2e-brew) installers.
[macos]
library-e2e-brew compiler-version:
  @just library-e2e-brew-download "{{compiler-version}}"
  @just library-e2e-brew-test

# Fill the repository's Homebrew formula for the requested release and install it.
# Homebrew has no way to pin a version on a plain tap, so we fill the formula
# template with the release's tarball + checksum and install that. `sed` (not
# `just publish`) avoids an envsubst/gettext dependency on macOS. The mac tarball
# matches the runner architecture; the install logic under test is arch-agnostic.
#
# Homebrew refuses to install a formula that is not in a tap ("Homebrew requires
# formulae to be in a tap"), so the filled-in formula goes into a throwaway local
# tap. The tap name is intentionally NOT the published ironplc/brew tap, so a
# local run cannot shadow or clobber the real one.
[macos]
library-e2e-brew-download compiler-version:
  #!/usr/bin/env sh
  set -eu
  case "$(uname -m)" in
    arm64|aarch64) MAC="ironplcc-aarch64-macos.tar.gz" ;;
    *)             MAC="ironplcc-x86_64-macos.tar.gz" ;;
  esac
  URL="https://github.com/ironplc/ironplc/releases/download/v{{compiler-version}}"
  SHA="$(curl -fsSL "$URL/$MAC.sha256" | cut -d' ' -f1)"
  # Start from a clean slate so the recipe can be re-run locally.
  brew uninstall --force ironplc >/dev/null 2>&1 || true
  brew untap --force ironplc/e2e >/dev/null 2>&1 || true
  brew tap-new --no-git ironplc/e2e
  TAP="$(brew --repository ironplc/e2e)"
  mkdir -p "$TAP/Formula"
  # The file name must match the formula's class name (Ironplc).
  sed -e "s#\${VERSION}#{{compiler-version}}#g" -e "s#\${MACFILENAME}#$MAC#g" \
      -e "s#\${MACSHA256}#$SHA#g" -e "s#\${LINUXFILENAME}#$MAC#g" -e "s#\${LINUXSHA256}#$SHA#g" \
      compiler/homebrew/Formula/ironplc.rb > "$TAP/Formula/ironplc.rb"
  brew install --formula ironplc/e2e/ironplc

# Compile + run against the Homebrew keg: binaries are symlinked into the keg bin
# and current_exe() resolves them back to libexec, where resources/libs lives.
[macos]
library-e2e-brew-test:
  PREFIX="$(brew --prefix ironplc)"; sh tests/e2e/library/verify.sh "$PREFIX/bin/ironplcc" "$PREFIX/bin/ironplcvm"

# Windows uses the NSIS installer, which installs to a fixed Program Files path.
# Each line is a separate PowerShell process (set windows-shell), so each step is
# a self-contained statement.
[windows]
library-e2e compiler-version:
  @just library-e2e-download "{{compiler-version}}"
  @just library-e2e-test

# Download + install the published compiler via the NSIS installer. The x86_64
# asset name is hard-coded because the GitHub runner is x86_64.
[windows]
library-e2e-download compiler-version:
  # Download the NSIS installer for the requested release.
  Invoke-WebRequest -Uri "https://github.com/ironplc/ironplc/releases/download/v{{compiler-version}}/ironplcc-x86_64-windows.exe" -OutFile ironplcc-setup.exe
  # Install silently.
  Start-Process ironplcc-setup.exe -ArgumentList "/S" -PassThru | Wait-Process -Timeout 120

# Verify against the NSIS layout, running the same shared script as the other
# platforms through Git for Windows' bash (always present on a machine that can
# clone this repository, and on the GitHub windows runners).
#
# The install path is spelled with forward slashes so it needs no backslash
# escaping inside the script's quoting, and the only PowerShell left is the
# $LASTEXITCODE propagation -- without it a failing script would be reported as a
# passing job. Assertions belong in verify.sh, not here.
[windows]
library-e2e-test:
  bash tests/e2e/library/verify.sh "{{ replace(env_var('LOCALAPPDATA'), '\', '/') }}/Programs/IronPLC Compiler/bin/ironplcc.exe" "{{ replace(env_var('LOCALAPPDATA'), '\', '/') }}/Programs/IronPLC Compiler/bin/ironplcvm.exe"; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# OpenCode integration end-to-end test - Unix only.
#
# Installs the published IronPLC compiler (which provides ironplcmcp), then
# verifies that the OpenCode agent works with the MCP server:
#   1. Connectivity smoke - `opencode mcp list` reports the server connected.
#   2. Agent end-to-end   - a local, key-free Ollama model invokes the `check`
#      tool and the compiler returns diagnostics.
#
# Tests the shipped binary rather than a local rebuild (mirrors
# install-script-smoke). Requires the Ollama CLI on PATH (installed in CI via
# ai-action/setup-ollama).
#
# compiler-version: empty to use the latest release; otherwise a bare version
#                   like "0.218.0" (without the leading "v").
# model:            the Ollama model used for the agent test.
[unix]
opencode-e2e compiler-version="" model="qwen2.5:1.5b":
  #!/usr/bin/env sh
  set -eu
  # Install the published compiler so we exercise the shipped ironplcmcp.
  just _install-script-smoke-clean
  just _install-script-smoke-run "{{compiler-version}}"
  BIN="$HOME/.ironplc/bin"
  if [ ! -x "$BIN/ironplcmcp" ]; then
    echo "ironplcmcp was not installed; the release must include it" >&2
    exit 1
  fi
  export IRONPLCMCP_BIN="$BIN/ironplcmcp"

  # Install the pinned OpenCode CLI.
  cd integrations/opencode
  npm ci

  # Layer 1: deterministic connectivity check (no model required).
  npm run smoke

  # Layer 2: deterministic tool-call gate against a fake model (no Ollama). This
  # proves OpenCode's real tool-call wiring end to end — read the catalog,
  # serialize the arguments, MCP round-trip to the real ironplcmcp — without any
  # model latency, so it is fast and never flaky.
  npm run mock-e2e

  # Layer 3: real-agent end-to-end against a local Ollama model. A larger
  # context window improves small models' tool-calling reliability: OpenCode's
  # system prompt plus the ironplc_check tool schema overflow the default
  # window, truncating the instructions so the model never calls the tool.
  #
  # OLLAMA_CONTEXT_LENGTH only takes effect when the server starts, and the CI
  # environment (ai-action/setup-ollama) has already started `ollama serve` with
  # the default window. A second `ollama serve` would just fail to bind with
  # "address already in use" and leave that default window in place, so stop any
  # running server first, then start our own with the larger window.
  # `timeout` is GNU coreutils: present on the Ubuntu CI runner but absent on
  # macOS (where it may exist as `gtimeout`). Fall back to a portable bounded
  # wait so a developer can run this recipe locally, not just in CI.
  run_timeout() {
    _secs="$1"; shift
    if command -v timeout >/dev/null 2>&1; then
      timeout "$_secs" "$@"
    elif command -v gtimeout >/dev/null 2>&1; then
      gtimeout "$_secs" "$@"
    else
      "$@" &
      _pid=$!
      _waited=0
      while kill -0 "$_pid" 2>/dev/null; do
        if [ "$_waited" -ge "$_secs" ]; then
          kill "$_pid" 2>/dev/null || true
          wait "$_pid" 2>/dev/null || true
          return 124
        fi
        _waited=$((_waited + 1))
        sleep 1
      done
      wait "$_pid"
    fi
  }

  pkill -x ollama 2>/dev/null || true
  run_timeout 30 sh -c 'while curl -sf http://localhost:11434/api/tags >/dev/null 2>&1; do sleep 1; done' || true
  OLLAMA_CONTEXT_LENGTH=16384 ollama serve >/tmp/ollama-serve.log 2>&1 &
  run_timeout 60 sh -c 'until curl -sf http://localhost:11434/api/tags >/dev/null 2>&1; do sleep 1; done'
  ollama pull "{{model}}"

  # On any failure below, surface the Ollama server log. The agent failure mode
  # we are guarding against ("Unexpected server error") originates in the model
  # provider, so this log is the other half of the picture alongside OpenCode's
  # own logs (which the agent test now prints on failure).
  dump_ollama_log() {
    status=$?
    if [ "$status" -ne 0 ]; then
      echo "==== ollama serve log (tail) ====" >&2
      tail -n 200 /tmp/ollama-serve.log >&2 || true
    fi
  }
  trap dump_ollama_log EXIT

  # Pre-flight: prove the model can do an OpenAI-compatible tool call directly,
  # independent of OpenCode. This isolates "the model/provider is broken" from
  # "the agent chose not to call the tool" — the ambiguity that makes a bare
  # agent-test failure hard to diagnose.
  echo "Pre-flight: probing {{model}} for OpenAI-compatible tool calling..."
  PROBE=$(curl -sf http://localhost:11434/v1/chat/completions \
    -H 'Content-Type: application/json' \
    -d '{
      "model": "{{model}}",
      "messages": [{"role": "user", "content": "Call the ping tool now."}],
      "tools": [{"type": "function", "function": {
        "name": "ping",
        "description": "Replies to a ping.",
        "parameters": {"type": "object", "properties": {}}
      }}],
      "tool_choice": "auto"
    }') || { echo "Pre-flight: the model did not respond to a tool-calling request." >&2; echo "$PROBE" >&2; exit 1; }
  echo "Pre-flight response (truncated):"
  printf '%s\n' "$PROBE" | head -c 2000; echo

  OPENCODE_E2E_MODEL="ollama/{{model}}" npm run agent-e2e

[windows]
opencode-e2e compiler-version="" model="qwen2.5:1.5b":
  @echo "opencode-e2e is Unix-only"
  exit 1