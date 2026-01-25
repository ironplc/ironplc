# Claude Code Instructions

This file provides entry points for Claude Code when working on the IronPLC project.

## Steering Files

Before making changes, read the relevant steering files in `specs/steering/`:

- **[Development Standards](specs/steering/development-standards.md)** - Core project conventions, testing patterns, error handling, and documentation standards
- **[Compiler Architecture](specs/steering/compiler-architecture.md)** - Patterns for implementing language features, module organization, and semantic analysis
- **[Problem Code Management](specs/steering/problem-code-management.md)** - Guidelines for error handling and diagnostic creation (especially relevant for `compiler/problems/` files)
- **[IEC 61131-3 Compliance](specs/steering/iec-61131-3-compliance.md)** - Standards compliance and validation rules (especially relevant for `**/analyzer/**` files)

## Quick Reference

### Key Commands
- `just` - Run tests for current component
- `just compile` - Build the component
- `just ci` - Run full CI pipeline
- `just devenv-smoke` - Quick environment check

### Project Structure
- `compiler/` - Rust compiler (multiple crates)
- `integrations/vscode/` - VS Code extension
- `docs/` - Sphinx documentation website

### Critical Rules
1. **BDD-style test names**: `function_when_condition_then_result`
2. **Module size limit**: Max 1000 lines per module
3. **Problem codes**: Must be documented in `docs/compiler/problems/P####.rst`
4. **Version numbers**: Automatically managed - do not edit manually
