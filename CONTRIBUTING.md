# Contributing

There are several components to IronPLC:

* the compiler
* the Visual Studio Code Extension

See CONTRIBUTING.md in `compiler` and `integrations` for more information about
how to develop each component.

## Developing

Continuous integration tests enforce what is important.

The full test suite is defined in GitHub actions workflow. You can the full
tests locally using [act](https://github.com/nektos/act) (requires Docker).

Follow the steps described in the [act](https://github.com/nektos/act)
repository to install `act`.

```sh
act --workflows ./.github/workflows/commit.yaml
```
