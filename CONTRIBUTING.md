# Contributing

Contributions are very welcome. This guide will help you understand how to
contribute to IronPLC. The guide assumes you are familiar with Git source code
control, especially on GitHub.

There are several components to IronPLC and you can think of this repository
as a single repository that hosts all of components:

* the compiler
* the Visual Studio Code Extension

See [compiler/CONTRIBUTING.md](compiler/CONTRIBUTING.md) and
[integrations/vscode/CONTRIBUTING.md](integrations/vscode/CONTRIBUTING.md) for
information about how to develop each component.

## Developing

Continuous integration tests enforce what is important and enable high-quality
weekly snapshots that are 100% hand-off. It just works.

The full test suite is defined in GitHub actions workflow. Using Docker and
[act](https://github.com/nektos/act), you can run the full Linux-only
integration tests locally. The full set of tests is slow to run - in most cases
you will want to run component-specific tests because they are much faster to
execute.

Follow the steps described in the [act](https://github.com/nektos/act)
repository to install `act`. Then run the following to execute the integration
tests:

```sh
# On Linux
act --workflows ./.github/workflows/commit.yaml

# On Windows
act --workflows ./.github/workflows/commit.yaml --env IRONPLC_INSTALL_DEPS=true
```

You can also run a specific job, for example:

```sh
act --workflows ./.github/workflows/commit.yaml --job vscode-extension-tests
```
