
set windows-shell := ["powershell.exe", "-c"]

# A quick check of the development environment
sanity:
  just _sanity-{{os()}}
  "SANITY PASSED"

_sanity-windows:
  "CHECK: compile the IronPLC compiler"
  cd compiler; just compile
  "CHECK: compile VS code extension (does not include tests)"
  cd integrations\vscode; just compile
  "CHECK: compile the docs"
  cd docs; just compile

_sanity-macos:
  echo "CHECK: compile the IronPLC compiler"
  cd compiler && just compile
  echo "CHECK: compile VS code extension (does not include tests)"
  cd integrations/vscode && just compile
  echo "CHECK: compile the docs"
  cd docs && just compile

_sanity-linux:
  echo "CHECK: compile the IronPLC compiler"
  cd compiler && just compile
  echo "CHECK: compile VS code extension (does not include tests)"
  cd integrations/vscode ** just compile
  echo "CHECK: compile the docs"
  cd docs && just compile

ci-commit-workflow:
  just _ci-commit-workflow-{{os()}}
  "TIP - this only ran the Linux tests"

_ci-commit-workflow-windows:
  act --workflows ./.github/workflows/commit.yaml --env IRONPLC_INSTALL_DEPS=true

_ci-commit-workflow-macos:
  act --workflows ./.github/workflows/commit.yaml

_ci-commit-workflow-linux:
  act --workflows ./.github/workflows/commit.yaml

# Sets the version number for all components. Must be a "bare" version number, such as 0.0.1 or 1.0.1.
version version:
  just _version-{{os()}} {{version}}

_version-windows version:
  cd compiler; just version {{version}}
  cd integrations\vscode; just version {{version}}
  cd docs; just version {{version}}

_version-macos version:
  cd compiler && just version {{version}}
  cd integrations/vscode && just version {{version}}
  cd docs && just version {{version}}

_version-linux version:
  cd compiler && just version {{version}}
  cd integrations/vscode && just version {{version}}
  cd docs && just version {{version}}
