
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
