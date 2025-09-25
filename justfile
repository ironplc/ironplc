
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

ci-update-dependencies-workflow:
  act workflow_dispatch --workflows ./.github/workflows/update.yaml --verbose

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
  git checkout -b {{branch}}
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
  Expand-Archive ironplc.vsix
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
  Get-ChildItem "{{env_var('LOCALAPPDATA')}}"
  IF (Test-Path "{{env_var('LOCALAPPDATA')}}\ironplcc.log" -PathType Leaf) { exit 0 } ELSE { exit 1 }

_endtoend-smoke-unix:
  @echo "endtoend-smoke is not implemented for Unix family"
  exit 1