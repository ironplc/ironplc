# Cursor Agent Instructions

This file is the IronPLC entry point for Cursor. Detailed guidance lives in **`specs/steering/`** (single source of truth, shared with Kiro, Claude Code, and other tools).

## Steering Files

Before making changes, read the relevant steering files in `specs/steering/`:

- **[Glossary](specs/steering/glossary.md)** - Authoritative definitions of core vocabulary (dialect, vendor, extension, edition); resolve terminology questions here before coining a new term
- **[Development Standards](specs/steering/development-standards.md)** - Development process (plan, prefactor and core change PRs), specs directory structure, prefactoring, and duplication rules
- **[Compiler Architecture](specs/steering/compiler-architecture.md)** - Patterns for implementing language features, module organization, and semantic analysis
- **[IEC 61131-3 Compliance](specs/steering/iec-61131-3-compliance.md)** - Standards compliance and validation rules (especially relevant for `**/analyzer/**` files)
- **[PLCopen XML Module](specs/steering/plcopen-xml-module.md)** - Architecture and patterns for the PLCopen XML parsing module (especially relevant for `compiler/sources/src/xml/` files)
- **[Syntax Support Guide](specs/steering/syntax-support-guide.md)** - Checklist and patterns for adding new syntax support, including `--allow-x` flags, plc2plc round-trip tests, and end-to-end execution tests (especially relevant for `**/parser/**`, `**/codegen/**`, `**/plc2plc/**` files)
- **[Problem Code Management](specs/steering/problem-code-management.md)** - Problem codes, diagnostics, and `compiler/problems/` workflows
- **[Extension Testing Requirements](specs/steering/extension-testing-requirements.md)** - VS Code extension CI gates and invariants (especially relevant for `integrations/vscode/**`)
- **[Steering File Guidelines](specs/steering/steering-file-guidelines.md)** - How IronPLC maintains steering docs and the pointer pattern
- **[Coming-from Guide Authoring](specs/steering/coming-from-guide-authoring.md)** - Standard page set, slugs, URL-stability policy, and content rules for the "Coming from X" how-to sections of the docs website (especially relevant for `docs/how-to-guides/**` files)

## Skills (Slash Commands) — Claude Code

This repository also defines Claude Code slash commands under `.claude/commands/` (e.g. `/project:build`). Cursor does not load those automatically; use the equivalent **`just`** targets from [specs/steering/common-tasks.md](specs/steering/common-tasks.md).

## MANDATORY: Git Workflow

**Follow the [Development Process](specs/steering/development-standards.md#development-process) for every change, and read it before non-trivial work.** Never push to `main`. Run `cd compiler && just` before opening any PR.

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

1. **Follow the [Development Process](specs/steering/development-standards.md#development-process)** - Never push to `main`; never cite a plan from code, docs or workflows (`cd specs && just` enforces this)
2. **BDD-style test names**: `function_when_condition_then_result`
3. **Module size limit**: Max 1000 lines per module
4. **No duplicated content** - Including in documentation; share via `docs/includes/` and `.. include::` ([Avoid Duplication](specs/steering/development-standards.md#avoid-duplication))
5. **Problem codes**: Must be documented in `docs/compiler/problems/P####.rst`
6. **Version numbers**: Automatically managed - do not edit manually
