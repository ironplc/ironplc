# Claude Code Instructions

This file provides entry points for Claude Code when working on the IronPLC project.

## Steering Files

Before making changes, read the relevant steering files in `specs/steering/`:

- **[Glossary](specs/steering/glossary.md)** - Authoritative definitions of core vocabulary (dialect, vendor, extension, edition); resolve terminology questions here before coining a new term
- **[Development Standards](specs/steering/development-standards.md)** - Cross-component process and conventions that apply to all work: specs directory structure, planning, prefactoring, and duplication rules
- **[Compiler Standards](specs/steering/compiler-standards.md)** - Rust coding standards: module structure, testing, error handling, performance, `unsafe`/clippy rules (especially relevant for `compiler/**` files)
- **[Documentation Standards](specs/steering/doc-standards.md)** - Documentation website standards: quadrants, writing style, RST roles, playground directives (especially relevant for `docs/**` files)
- **[Extension Standards](specs/steering/extension-standards.md)** - VS Code extension coding standards: README sync, testing gates, `E####` error codes (especially relevant for `integrations/vscode/**` files)
- **[Compiler Architecture](specs/steering/compiler-architecture.md)** - Patterns for implementing language features, module organization, and semantic analysis (especially relevant for `compiler/**` files)
- **[IEC 61131-3 Compliance](specs/steering/iec-61131-3-compliance.md)** - Standards compliance and validation rules (especially relevant for `**/analyzer/**` files)
- **[PLCopen XML Module](specs/steering/plcopen-xml-module.md)** - Architecture and patterns for the PLCopen XML parsing module (especially relevant for `compiler/sources/src/xml/` files)
- **[Syntax Support Guide](specs/steering/syntax-support-guide.md)** - Checklist and patterns for adding new syntax support, including `--allow-x` flags, plc2plc round-trip tests, and end-to-end execution tests (especially relevant for `**/parser/**`, `**/codegen/**`, `**/plc2plc/**` files)
- **[Compatibility Library Authoring](specs/steering/compatibility-library-authoring.md)** - Licensing risk tiers, allowed/forbidden inputs, and the clean-room-with-AI workflow for authoring bundled compatibility libraries (especially relevant for `compiler/sources/resources/compat-libraries/` files)
- **[Coming-from Guide Authoring](specs/steering/coming-from-guide-authoring.md)** - Standard page set, slugs, URL-stability policy, and content rules for the "Coming from X" how-to sections of the docs website (especially relevant for `docs/how-to-guides/**` files)
- **[External Corpus Defect Sourcing](specs/steering/external-corpus-defect-sourcing.md)** - How a defect found by running third-party IEC 61131-3 code becomes a change here: automation stays in a separate repository, findings cross as prose-only issues, fixes are authored from the issue alone (relevant whenever an issue is labelled as corpus-sourced)

## Skills (Slash Commands)

Use these commands for common development tasks. Each skill includes fallback commands for when `just` is not available.

- `/project:build` - Build the compiler
- `/project:test` - Run tests (with coverage options)
- `/project:ci` - **Full CI pipeline (REQUIRED before creating any PR)**
- `/project:format` - Auto-fix formatting and lint issues
- `/project:reconcile-spec` - Reconcile one spec section with implementation (add REQ IDs and tests)

For full details, see [specs/steering/common-tasks.md](specs/steering/common-tasks.md).

## MANDATORY: Git Workflow

**NEVER commit or push directly to `main`.** Always create a feature branch and open a pull request. This ensures CI validates all changes before they reach main.

### Workflow

1. Create a feature branch from `main`
2. **Write an implementation plan** in `specs/plans/` and commit it as the first commit on the branch. If the work spans more than one PR, open an issue first and reference it from the plan (see [Development Standards — Planning Requirement](specs/steering/development-standards.md#planning-requirement))
3. **Prefactor first** — simplify the existing code so the change drops in, in its own commit, before adding new behaviour (see [Development Standards — Prefactoring](specs/steering/development-standards.md#prefactoring))
4. Implement the changes following the plan
5. Land any decision worth keeping as an ADR or `specs/design/` update, and open an issue for anything the plan describes that you are not delivering — it is about to be deleted
6. **`git rm` the plan file** — plans are deleted before merge, so no plan content reaches `main`
7. Run the full CI pipeline: `cd compiler && just`
8. Push the feature branch and create a PR via `gh pr create`

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
2. **Plan first, then delete it** - Non-trivial changes start with a plan in `specs/plans/`, committed before implementation code and removed before merge; work spanning more than one PR must also have an issue; never cite a plan from code, docs or workflows (`cd specs && just` enforces this)
3. **Prefactor before adding** - Every change looks for a simplification to make first; the plan says what it is, or why none is needed
4. **Run `cd compiler && just` before creating any PR** - This runs clippy, tests, and all checks
5. **BDD-style test names**: `function_when_condition_then_result`
6. **Module size limit**: Max 1000 lines per module
7. **No duplicated content** - Including in documentation; share via `docs/includes/` and `.. include::` ([Avoid Duplication](specs/steering/development-standards.md#avoid-duplication))
8. **Problem codes**: Must be documented in `docs/compiler/problems/P####.rst`
9. **Version numbers**: Automatically managed - do not edit manually
