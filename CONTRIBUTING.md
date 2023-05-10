# Contributing

Contributions are very welcome. This guide will help you understand how to
contribute to IronPLC. The guide assumes you are familiar with Git source code
control, especially on GitHub.

There are several components to IronPLC and you can think of this repository
as a single repository that hosts all of components:

* the [compiler](compiler/CONTRIBUTING.md)
* the [Visual Studio Code Extension](integrations/vscode/CONTRIBUTING.md)
* the [documentation website](docs/CONTRIBUTING.md)

See below for common recommdations or follow the links above for information
about how to develop each component.

## Developing

Cross-platform development is an exercise in patience and frustration. If easy
isn't possible, then we've tried to make it straightforward. The following
steps outline a process that should work on any environment provided. You need
to install:

* Git (obviously)
* Docker
* Visual Studio Code with the Dev Containers extension

Things are even easier if you also install:

* [Just command runner](https://just.systems/man/en/)
* [act](https://github.com/nektos/act)

Then follow these steps to check that you have a working environment:

1. Checkout this repository to a local directory.
1. Open the project in Visual Studio Code. Visual Studio Code should prompt
   to enable the Dev Container.
1. After the container loads, then in the Visual Studio Code Terminal, execute
   the following to run some tests:

   ```sh
   just sanity
   ```

   💡 Running directly on your local machine (as opposed to the
      docker container) requires multiple other dependencies.

   When the task completes, you will see

   ```sh
   "SANITY PASSED"
   ```

   indicating you have a mostly (or perhaps 100%) working environment.

Follow the steps for each component to continue your development
environment.

Once your are done, return here for instructions on how to run continuous
integration tests locally before creating a pull request (or do it now just to
see how it works).

## Local Integration Testing

As described above, cross-platform development is hard. Unfortunately I don't
know of a great way to run integration tests across all platforms locally.

The best offer here is to run the "on-commit" tests on a Ubuntu Docker image.
The on-commit tests are slow to run because they test are extensive.
You will want to run component-specific tests because they are much faster to
execute. Nevertheless, if you want to reproduce the GitHub commit checks, this
is the way.

Execute the following to run what you can locally:

```sh
just ci-commit-workflow
```
