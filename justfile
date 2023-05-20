
set windows-shell := ["powershell.exe", "-c"]

# A quick check of the development environment
sanity:
  @just _sanity-{{os_family()}}


_sanity-windows:
  @"CHECK: compile the IronPLC compiler"
  cd compiler; just compile
  @"CHECK: compile VS code extension (does not include tests)"
  cd integrations\vscode; just compile
  @"CHECK: compile the docs"
  cd docs; just compile
  "SANITY PASSED"

_sanity-unix:
  @echo "CHECK: compile the IronPLC compiler"
  cd compiler && just compile
  @echo "CHECK: compile VS code extension (does not include tests)"
  cd integrations/vscode && just compile
  @echo "CHECK: compile the docs"
  cd docs && just compile
  @echo "SANITY PASSED"

# Simulate the workflow that runs to validate a commit (as best as is possible via Docker)
ci-commit-workflow:
  @just _ci-commit-workflow-{{os_family()}}
  "TIP - this only ran the Linux tests"

_ci-commit-workflow-windows:
  act --workflows ./.github/workflows/commit.yaml --env IRONPLC_INSTALL_DEPS=true

_ci-commit-workflow-unix:
  act --workflows ./.github/workflows/commit.yaml

# Sets the version number for all components. Must be a "bare" version number, such as 0.0.1 or 1.0.1.
version version:
  @just _version-{{os_family()}} {{version}}

# Used by the publishing workflow. Sets the version number and then creates a commit with the updated version.
publish-version version:
  cargo install cargo-release
  @just _version-{{os_family()}} {{version}}
  @git add *
  @git commit -m "Update version number to {{version}}"
  @git tag v{{version}}

_version-windows version:
  cd compiler; just version {{version}}
  cd integrations\vscode; just version {{version}}
  cd docs; just version {{version}}

_version-unix version:
  cd compiler && just version {{version}}
  cd integrations/vscode && just version {{version}}
  cd docs && just version {{version}}
