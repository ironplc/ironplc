# Contributing

Contributions are very welcome. This guide will help you understand how to
contribute to IronPLC. The guide assumes you are familiar with Git source code
control, especially on GitHub.

There are several components to IronPLC and you can think of this repository
as a single repository that hosts all of components:

* the [compiler](compiler/CONTRIBUTING.md)
* the [Visual Studio Code Extension](integrations/vscode/CONTRIBUTING.md)
* the [documentation website](docs/CONTRIBUTING.md)

See below for common recommendations or follow the links above for information
about how to develop each component.

## Code Standards and AI-Assisted Development

IronPLC uses AI-assisted development with detailed coding standards and architectural patterns defined in steering files. These files guide both human and AI contributors to maintain consistency and quality:

* **[Development Standards](specs/steering/development-standards.md)** - Core project conventions, testing patterns, and error handling
* **[Compiler Architecture](specs/steering/compiler-architecture.md)** - Patterns for implementing language features and semantic analysis
* **[Problem Code Management](specs/steering/problem-code-management.md)** - Guidelines for error handling and diagnostic creation
* **[IEC 61131-3 Compliance](specs/steering/iec-61131-3-compliance.md)** - Standards compliance and validation rules

When contributing code, these steering files provide the detailed implementation guidance, while this CONTRIBUTING.md focuses on the development workflow and setup process.

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
   just devenv-smoke
   ```

   💡 Running directly on your local machine (as opposed to the
      docker container) requires multiple other dependencies.

   When the task completes, you will see

   ```sh
   "SMOKE PASSED"
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

## Code Quality Expectations

### Testing Standards
All code must follow BDD-style test naming conventions and include comprehensive test coverage. See the development standards steering file for specific patterns and examples.

### Error Handling
Error handling is critical for developer experience. All errors must use the shared problem code system with proper documentation. See the problem code management steering file for the complete lifecycle and requirements.

### Architecture Compliance
The compiler follows specific architectural patterns for semantic analysis and type checking. Modules must remain focused and under 1000 lines of code. See the compiler architecture steering file for detailed guidance.

### IEC 61131-3 Compliance
All language features must follow IEC 61131-3 standard compliance rules with configurable validation levels. See the compliance steering file for implementation requirements.
