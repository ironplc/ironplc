# Claude Code Instructions

This file provides entry points for Claude Code when working on the IronPLC project.

## Steering Files

Before making changes, read the relevant steering files in `specs/steering/`:

- **[Development Standards](specs/steering/development-standards.md)** - Core project conventions, testing patterns, error handling, and documentation standards
- **[Compiler Architecture](specs/steering/compiler-architecture.md)** - Patterns for implementing language features, module organization, and semantic analysis
- **[Common Tasks](specs/steering/common-tasks.md)** - Build commands, testing workflows, and justfile-based development tasks
- **[Problem Code Management](specs/steering/problem-code-management.md)** - Guidelines for error handling and diagnostic creation (especially relevant for `compiler/problems/` files)
- **[IEC 61131-3 Compliance](specs/steering/iec-61131-3-compliance.md)** - Standards compliance and validation rules (especially relevant for `**/analyzer/**` files)
- **[Steering File Guidelines](specs/steering/steering-file-guidelines.md)** - How to create and maintain steering files (for AI assistants updating documentation)

## Quick Reference

### Key Commands
- `cd compiler && just` - Run full CI pipeline (compile, test, coverage, lint)
- `cd compiler && just compile` - Build the compiler
- `cd compiler && just test` - Run all tests
- `cd compiler && just coverage` - Run tests with coverage (requires 85%)
- `just devenv-smoke` - Quick environment check

See [specs/steering/common-tasks.md](specs/steering/common-tasks.md) for complete command reference.

### Project Structure
- `compiler/` - Rust compiler (multiple crates)
- `integrations/vscode/` - VS Code extension
- `docs/` - Sphinx documentation website

### Critical Rules
1. **BDD-style test names**: `function_when_condition_then_result`
2. **Module size limit**: Max 1000 lines per module
3. **Problem codes**: Must be documented in `docs/compiler/problems/P####.rst`
4. **Version numbers**: Automatically managed - do not edit manually
