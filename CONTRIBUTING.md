# Contributing

Contributions are very welcome. This guide will help you understand how to
contribute to IronPLC. The guide assumes you are familiar with Git source code
control, especially on GitHub.

There are several components to IronPLC and you can think of this repository
as a single repository that hosts all of components:

* the [compiler](compiler/CONTRIBUTING.md)
* the [Visual Studio Code Extension](integrations/vscode/CONTRIBUTING.md)
* the [documentation website](docs/CONTRIBUTING.md)
* the [interactive playground](playground/CONTRIBUTING.md)

See below for common recommendations or follow the links above for information
about how to develop each component.

## Code Standards and Project Conventions

IronPLC has detailed coding standards and architectural patterns defined in
steering files under `specs/steering/`. These apply to all contributors:

* **[Development Standards](specs/steering/development-standards.md)** - Development process, specs directory structure, prefactoring, and duplication rules
* **[Compiler Architecture](specs/steering/compiler-architecture.md)** - Patterns for implementing language features and semantic analysis
* **[Problem Code Management](specs/steering/problem-code-management.md)** - Guidelines for error handling and diagnostic creation
* **[IEC 61131-3 Compliance](specs/steering/iec-61131-3-compliance.md)** - Standards compliance and validation rules
* **[Compatibility Library Authoring](specs/steering/compatibility-library-authoring.md)** - Licensing risk tiers, allowed/forbidden inputs, and the clean-room provenance record required for bundled compatibility libraries
* **[Common Tasks](specs/steering/common-tasks.md)** - Full command reference for day-to-day development

The steering files provide the detailed implementation guidance; this
CONTRIBUTING.md focuses on the development workflow and setup process.

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

## Planning and Opening Pull Requests

Follow the [Development Process](specs/steering/development-standards.md#development-process).
It says which pull requests to open, in what order, and what to run before
opening each one. People and AI assistants follow the same process.

## Automated Changes

We allow certain well-established automated systems:

* Dependabot (dependency updates)
* Internal CI/CD systems
* GitHub Actions from this repository

All other changes must have a human as the author. This includes:

* Custom bots or scripts
* External services
* Automated agents

If you use an LLM or AI tool to write code, you (the human) must submit the PR
under your own account.

## Code Quality Expectations

### Testing Standards

All tests use BDD-style naming:

```
function_when_condition_then_expected_result
```

Example:

```rust
#[test]
fn parse_when_input_is_empty_then_returns_error() {
    // ...
}
```

Coverage is enforced at **85%** by `just coverage` in the compiler. Add tests
for any new code paths.

### Error Handling

All compiler errors use the shared problem code system. Every new problem
code requires documentation at
`docs/reference/compiler/problems/P####.rst`. See the [Problem Code
Management](specs/steering/problem-code-management.md) steering file for the
full lifecycle and requirements.

### Architecture Compliance

The compiler follows specific architectural patterns for semantic analysis
and type checking. **Modules must stay under 1000 lines of code.** See the
[Compiler Architecture](specs/steering/compiler-architecture.md) steering
file for detailed guidance.

### IEC 61131-3 Compliance

All language features must follow IEC 61131-3 standard compliance rules with
configurable validation levels. See the [IEC 61131-3
Compliance](specs/steering/iec-61131-3-compliance.md) steering file for
implementation requirements.
