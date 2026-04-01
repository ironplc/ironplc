# Claude Code Instructions

This file provides entry points for Claude Code when working on the IronPLC project.

## Steering Files

Before making changes, read the relevant steering files in `specs/steering/`:

- **[Development Standards](specs/steering/development-standards.md)** - Core project conventions, testing patterns, error handling, and documentation standards
- **[Compiler Architecture](specs/steering/compiler-architecture.md)** - Patterns for implementing language features, module organization, and semantic analysis
- **[IEC 61131-3 Compliance](specs/steering/iec-61131-3-compliance.md)** - Standards compliance and validation rules (especially relevant for `**/analyzer/**` files)
- **[PLCopen XML Module](specs/steering/plcopen-xml-module.md)** - Architecture and patterns for the PLCopen XML parsing module (especially relevant for `compiler/sources/src/xml/` files)
- **[Syntax Support Guide](specs/steering/syntax-support-guide.md)** - Checklist and patterns for adding new syntax support, including `--allow-x` flags, plc2plc round-trip tests, and end-to-end execution tests (especially relevant for `**/parser/**`, `**/codegen/**`, `**/plc2plc/**` files)

## Skills (Slash Commands)

Use these commands for common development tasks. Each skill includes fallback commands for when `just` is not available.

- `/project:build` - Build the compiler
- `/project:test` - Run tests (with coverage options)
- `/project:ci` - **Full CI pipeline (REQUIRED before creating any PR)**
- `/project:format` - Auto-fix formatting and lint issues

For full details, see [specs/steering/common-tasks.md](specs/steering/common-tasks.md).

## MANDATORY: Git Workflow

**NEVER commit or push directly to `main`.** Always create a feature branch and open a pull request. This ensures CI validates all changes before they reach main.

### Workflow

1. Create a feature branch from `main`
2. **Write an implementation plan** in `specs/plans/` and commit it to the branch (see [Development Standards — Planning Requirement](specs/steering/development-standards.md#planning-requirement))
3. Implement the changes following the plan
4. Run the full CI pipeline: `cd compiler && just`
5. Push the feature branch and create a PR via `gh pr create`

> **Skip the plan** for mechanical changes: typo fixes, formatting, dependency bumps, single-line bug fixes, or documentation-only edits.

### Before Creating a PR

**You MUST run the full CI pipeline and verify it passes before creating any PR:**

```bash
cd compiler && just
```

This runs compile, coverage (which includes tests), AND lint (clippy + fmt). **All checks must pass.**

If any check fails:
1. Fix the issues
2. Re-run `cd compiler && just`
3. Only create the PR after all checks pass

**Common failures:**
- **Clippy warnings** - Fix all clippy issues; the lint step runs `cargo clippy`
- **Format issues** - Run `cd compiler && just format` to auto-fix
- **Coverage below 85%** - Add tests for uncovered code

## Quick Reference

### Key Commands
- `cd compiler && just` - **Run full CI pipeline (REQUIRED before PR)**
- `cd compiler && just compile` - Build the compiler
- `cd compiler && just test` - Run all tests
- `cd compiler && just coverage` - Run tests with coverage (requires 85%)
- `cd compiler && just lint` - Run clippy and format checks
- `just devenv-smoke` - Quick environment check

See [specs/steering/common-tasks.md](specs/steering/common-tasks.md) for complete command reference.

### Project Structure
- `compiler/` - Rust compiler (multiple crates)
- `integrations/vscode/` - VS Code extension
- `docs/` - Sphinx documentation website
- `playground/` - Interactive playground (browser-based editor/runner, built from `compiler/playground/` WASM crate)

### Critical Rules
1. **NEVER push directly to `main`** - Always use a feature branch and pull request
2. **Plan first** - Non-trivial changes must start with a plan in `specs/plans/` committed before implementation code
3. **Run `cd compiler && just` before creating any PR** - This runs clippy, tests, and all checks
4. **BDD-style test names**: `function_when_condition_then_result`
5. **Module size limit**: Max 1000 lines per module
6. **Problem codes**: Must be documented in `docs/compiler/problems/P####.rst`
7. **Version numbers**: Automatically managed - do not edit manually
