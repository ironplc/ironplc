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

* **[Development Standards](specs/steering/development-standards.md)** - Core project conventions, testing patterns, and error handling
* **[Compiler Architecture](specs/steering/compiler-architecture.md)** - Patterns for implementing language features and semantic analysis
* **[Problem Code Management](specs/steering/problem-code-management.md)** - Guidelines for error handling and diagnostic creation
* **[IEC 61131-3 Compliance](specs/steering/iec-61131-3-compliance.md)** - Standards compliance and validation rules
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

## Planning Non-Trivial Changes

**Non-trivial changes must start with a plan committed to `specs/plans/`.**
This keeps design discussion in the open and makes review easier.

Workflow:

1. Create a feature branch from `main`. Do not commit directly to `main`.
1. Write an implementation plan and save it under `specs/plans/` (follow the
   naming convention used by existing files there).
1. Commit the plan to the feature branch before the implementation code.
1. Implement the changes following the plan.
1. Run the pre-PR checks described below.
1. Push the branch and open a pull request.

You may **skip the plan** for mechanical changes: typo fixes, formatting,
dependency bumps, single-line bug fixes, or documentation-only edits.

## Before You Open a PR

You must run the full CI pipeline locally and see it pass before opening a
pull request:

```sh
cd compiler && just
```

This runs compile, coverage (which includes tests), and lint (clippy + fmt).
All checks must pass.

Common failures:

* **Clippy warnings** - Fix all clippy issues.
* **Format issues** - Run `cd compiler && just format` to auto-fix.
* **Coverage below 85%** - Add tests for uncovered code (`just coverage`
  enforces `--fail-under-lines 85`).

If you touched the VS Code extension, the docs, or the playground, also run
the component's own CI recipe (`just ci` from that component's directory).

Full cross-platform integration tests run in GitHub Actions when you push.

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
