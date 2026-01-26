# IronPLC Common Development Tasks

This file provides quick reference for common development tasks and commands in the IronPLC project.

> **Note**: This is a reference guide for AI-assisted development. For complete setup instructions and contribution workflow, see [CONTRIBUTING.md](../../CONTRIBUTING.md).

## Build System Overview

IronPLC uses [just](https://github.com/casey/just) as its command runner. All build commands are defined in justfiles that handle cross-platform differences automatically. The justfiles are useful references for to understand the project configuration.

### Justfile Locations

- **Root `justfile`**: Cross-component tasks (CI simulation, versioning, end-to-end tests)
- **`compiler/justfile`**: Compiler build, test, coverage, lint, package
- **`docs/justfile`**: Documentation build and publishing
- **`integrations/vscode/justfile`**: VS Code extension tasks

## Most Common Commands

**All components support these standard commands:**

```bash
cd [component]   # compiler, docs, or integrations/vscode
just             # Run the default CI pipeline for this component
just compile     # Build the component
just test        # Run tests (or validation for docs)
just lint        # Check for style/formatting issues
just clean       # Remove build artifacts
```

### Component-Specific Details

#### Compiler

```bash
cd compiler
just             # Runs: compile, test, coverage, lint
just coverage    # Run tests with coverage (requires 85% line coverage)
just format      # Auto-fix linting errors
just clean       # Remove build artifacts (target/, lcov.info)
```

#### Documentation

```bash
cd docs
just             # Runs: compile
just test        # Validate documentation (links, syntax)
just clean       # Remove built files (_build/)
```

#### VS Code Extension

```bash
cd integrations/vscode
just             # Runs: compile, lint
just ci          # Full CI: compile, lint, test
just clean       # Remove built files (out/, *.vsix)
```

### Cross-Component Tasks

From the repository root:

```bash
just devenv-smoke              # Quick smoke test of all components
just ci-commit-workflow        # Simulate GitHub commit workflow locally
just update                    # Update dependencies across all components
```

## Before Suggesting Commands

**Critical workflow for AI assistants**:

1. **Check the relevant justfile first** - Don't guess at commands
2. **Use `just` commands when they exist** - They handle cross-platform differences
3. **Only use raw `cargo` commands** for tasks not in justfiles (like running specific tests)
4. **Reference the justfile location** when suggesting commands users might want to customize

### Example: Suggesting Test Commands

✅ **Good**:
```
Run the tests with:
  cd compiler && just test

To run only analyzer tests:
  cd compiler && cargo test --package analyzer
```

❌ **Bad**:
```
Run the tests with:
  cargo test --all
```

## Coverage Requirements

The project enforces **85% line coverage** as a quality gate:

```bash
cd compiler
just coverage  # Fails if coverage drops below 85%
```

The coverage command:
- Ignores certain files (cargo internals, dsl_macro_derive, rustup)
- Shows missing lines in the output
- Generates `lcov.info` for coverage reporting tools

## Packaging and Release

### Creating Platform Packages

```bash
cd compiler
just package VERSION FILENAME TARGET
```

Examples:
- Windows: `just package 0.1.0 ironplc-0.1.0-windows.exe x86_64-pc-windows-msvc`
- macOS: `just package 0.1.0 ironplc-0.1.0-macos.tar.gz aarch64-apple-darwin`
- Linux: `just package 0.1.0 ironplc-0.1.0-linux.tar.gz x86_64-unknown-linux-gnu`

### Version Management

**Important**: Version numbers are managed automatically by the build system. Do not manually edit version numbers.

```bash
# From repository root
just version 0.2.0  # Sets version across all components
```

## Development Environment

### Dev Container

The project includes a dev container configuration in `.devcontainer/`. This provides a consistent development environment with all dependencies pre-installed.

### Quick Environment Check

```bash
just devenv-smoke
```

This command:
- Compiles the compiler
- Compiles the VS Code extension
- Compiles the documentation
- Verifies the basic development environment is working

## Troubleshooting Common Issues

### "Command not found: just"

Install just:
- macOS: `brew install just`
- Linux: `cargo install just`
- Windows: `cargo install just` or `scoop install just`

### Coverage Fails Below 85%

The coverage command will fail if line coverage drops below 85%. To see which lines are missing coverage:

```bash
cd compiler
just coverage  # Shows missing lines in output
```

Add tests for the uncovered lines or justify why they can't be tested.

### Cross-Platform Build Issues

The justfiles handle platform differences automatically. If you encounter platform-specific issues:

1. Check if there's a platform-specific recipe (e.g., `_command-windows`, `_command-unix`)
2. Ensure you're using `just` commands rather than raw shell commands
3. Test in the dev container for a consistent Linux environment

## Integration with CI/CD

### Local CI Simulation

You can simulate GitHub Actions workflows locally using [act](https://github.com/nektos/act):

```bash
just ci-commit-workflow   # Simulate commit validation workflow
just ci-publish-workflow  # Simulate release workflow
```

**Note**: This requires Docker and only runs Linux tests.

### What CI Checks

The commit workflow runs:
1. Compilation of all components
2. Full test suite
3. Coverage check (85% threshold)
4. Linting and formatting checks
5. Documentation build

All of these can be run locally with `just` commands before pushing.
