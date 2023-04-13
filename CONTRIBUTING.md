# Contributing

There are several components to IronPLC:

* the compiler
* the Visual Studio Code Extension

See `CONTRIBUTING.md` in `compiler` and `integrations` for more information about
how to develop each component.

## Developing

Continuous integration tests enforce what is important.

The full test suite is defined in GitHub actions workflow. You can the full
Linux-only tests locally using [act](https://github.com/nektos/act)
(requires Docker).

Follow the steps described in the [act](https://github.com/nektos/act)
repository to install `act`. Then run the following to execute the integration
tests:

```sh
# On Linux
act --workflows ./.github/workflows/commit.yaml

# On Windows
act --workflows ./.github/workflows/commit.yaml --env IS_WSL=true
```

You can also run a specific job, for example:

```sh
act --workflows ./.github/workflows/commit.yaml --job vscode-extension-tests

```
