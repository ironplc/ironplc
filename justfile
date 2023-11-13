
set windows-shell := ["powershell.exe", "-c"]

# A quick check of the development environment.
devenv-smoke:
  @just _devenv-smoke-{{os_family()}}

_devenv-smoke-windows:
  @"CHECK: compile the IronPLC compiler"
  cd compiler; just compile
  @"CHECK: compile VS code extension (does not include tests)"
  cd integrations\vscode; just setup; just compile
  @"CHECK: compile the docs"
  cd docs; just compile
  "SMOKE PASSED"

_devenv-smoke-unix:
  @echo "CHECK: compile the IronPLC compiler"
  cd compiler && just compile
  @echo "CHECK: compile VS code extension (does not include tests)"
  cd integrations/vscode && just setup && just compile
  @echo "CHECK: compile the docs"
  cd docs && just compile
  @echo "SMOKE PASSED"

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

# Sets the version number for all components. Must be a "bare" version number, such as 0.0.1 or 1.0.1.
version version:
  # We need this specific package to do the update
  @cargo install cargo-release
  @just _version-{{os_family()}} {{version}}

_version-windows version:
  cd compiler; just version {{version}}
  cd integrations\vscode; just version {{version}}
  cd docs; just version {{version}}

_version-unix version:
  cd compiler && just version {{version}}
  cd integrations/vscode && just version {{version}}
  cd docs && just version {{version}}

# Updates dependencies to latest versions.
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

# End to end smoke test install specified components and validates compatibility.
[windows]
endtoend-smoke compiler-version extension-version compilerfilename extensionfilename ext:
  # There are two parts to IronPLC - the compiler and the extension
  # This test ensures that they actually work together (out of the box)
  #
  # compiler-version is the compiler version to download from GitHub Releases
  # extension-version is the VS Code version to download from GitHub Releases
  # compilerfilename is the name of the compiler installer
  # compilerfilename is the name of the extension VSIX
  # extension-id is the ID of the extension
  @just endtoend-smoke-download {{compiler-version}} {{extension-version}} {{compilerfilename}} {{extensionfilename}}
  @just endtoend-smoke-test {{compiler-version}} {{extension-version}} {{extension-id}}

[windows]
endtoend-smoke-download compiler-version extension-version compilerfilename extensionfilename:
  Invoke-WebRequest -Uri "https://github.com/ironplc/ironplc/releases/download/{{compiler-version}}/{{compilerfilename}}" -OutFile ironplcc.exe
  Invoke-WebRequest -Uri "https://code.visualstudio.com/sha/download?build=stable&os=win32-x64-user" -OutFile vscode.exe
  Invoke-WebRequest -Uri "https://github.com/ironplc/ironplc/releases/download/{{extension-version}}/{{extensionfilename}}" -OutFile ironplc.vsix

[windows]
endtoend-smoke-test compiler-version extension-version extension-id:
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
  Expand-Archive ironplc.vsix
  # Move the folder 
  New-Item -ItemType Directory -Force -Path "{{env_var('USERPROFILE')}}\.vscode\extensions\"
  Move-Item ironplc\extension "{{env_var('USERPROFILE')}}\.vscode\extensions\{{extension-id}}-{{compiler-version}}"
  Get-ChildItem "{{env_var('USERPROFILE')}}\.vscode\extensions\{{extension-id}}-{{compiler-version}}"

  # Create the extensions.json file that references this extension
  New-Item "{{env_var('USERPROFILE')}}\.vscode\extensions\extensions.json" -Force
  '[{"identifier":{"id":"{{extension-id}}"},"version":"{{compiler-version}}","location":{"$mid":1,"fsPath":"{{e2e_fspathesc}}{{extension-id}}-{{compiler-version}}","_sep":1,"external":"{{e2e_external}}{{extension-id}}-{{compiler-version}}","path":"{{e2e_path}}{{extension-id}}-{{compiler-version}}","scheme":"file"},"relativeLocation":"{{extension-id}}-{{compiler-version}}","metadata":{"installedTimestamp":1695013253133}}]'.replace('*', '\\') | Set-Content "{{env_var('USERPROFILE')}}\.vscode\extensions\extensions.json"
  Get-Content -Path "{{env_var('USERPROFILE')}}\.vscode\extensions\extensions.json"

  # Create the settings.json with the configuration to enable trace level logging (that's the 4 -v's)
  New-Item "{{env_var('APPDATA')}}\Code\User\settings.json" -Force
  Set-Content "{{env_var('APPDATA')}}\Code\User\settings.json" '{ "security.workspace.trust.enabled": false, "ironplc.logLevel": "TRACE" }'
  Get-Content "{{env_var('APPDATA')}}\Code\User\settings.json"

  # Open an example file that is part of the compiler - this is a hard coded path
  # but that's also the point. We expect the installer to install here by default
  # so that the extension will find the compiler by default.
  Get-ChildItem "{{env_var('LOCALAPPDATA')}}\Programs\IronPLC Compiler\examples\"
  Start-Process "`"{{env_var('LOCALAPPDATA')}}\Programs\Microsoft VS Code\code.exe`"" -ArgumentList "`"{{env_var('LOCALAPPDATA')}}\Programs\IronPLC Compiler\examples\getting_started.st`""

  # Check that the log file was created (indicating that VS Code correctly started the
  # ironplcc language server). This path is a well-known path
  Start-Sleep -s 30
  Get-ChildItem "{{env_var('LOCALAPPDATA')}}\Temp\"
  Get-ChildItem "{{env_var('LOCALAPPDATA')}}\Temp\ironplcc"
  IF (Test-Path "{{env_var('LOCALAPPDATA')}}\Temp\ironplcc\ironplcc.log" -PathType Leaf) { exit 0 } ELSE { exit 1 }

_endtoend-smoke-unix:
  @echo "endtoend-smoke is not implemented for Unix family"
  exit 1