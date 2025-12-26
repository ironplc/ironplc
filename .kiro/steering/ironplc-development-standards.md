# IronPLC Development Standards

This steering file defines the core development standards and patterns for the IronPLC project, a Rust-based PLC compiler implementing the IEC 61131-3 standard.

> **Note**: This file provides detailed implementation guidance for AI-assisted development. For development workflow, setup instructions, and contribution processes, see the main [CONTRIBUTING.md](../CONTRIBUTING.md) and component-specific contributing guides.

## Project Structure

IronPLC consists of three primary components that must be kept in sync:

1. **Compiler** (`compiler/`) - The core Rust compiler with multiple crates
2. **VS Code Extension** (`integrations/vscode/`) - Language server and IDE integration  
3. **Documentation Website** (`docs/`) - Sphinx-based documentation

**Critical**: The build will fail if these components get out of sync. Always ensure version numbers, problem codes, and language features are synchronized across all three components.

## Code Organization

### Module Structure
- Follow the existing pattern of organizing related functionality in subdirectories (e.g., `analyzer/src/intermediates/`)
- Use descriptive module names that reflect their purpose in the compilation pipeline
- Keep modules focused on a single responsibility

### Naming Conventions
- Use `snake_case` for functions, variables, and module names
- Use `PascalCase` for types, structs, and enums
- Use `SCREAMING_SNAKE_CASE` for constants
- Problem codes follow the pattern `P####` (e.g., P0044)
- Problem enum variants use descriptive names (e.g., `SubrangeOutOfBounds`)

## Testing Standards

### Test Naming
**Always use BDD-style test names** following the pattern:
```rust
#[test]
fn function_name_when_condition_then_expected_result() {
    // Test implementation
}
```

Examples:
- `validate_subrange_bounds_with_various_types_then_validates_correctly`
- `try_from_with_invalid_range_then_p0004_error`

### Test Implementation Rules
- **No branching logic** in tests (no `if`, `match`, loops)
- **No global state dependencies** - each test must be self-contained
- **Terminate on failure** - use `assert!`, `assert_eq!`, etc. rather than continuing
- **One assertion per logical concept** - but multiple assertions for the same concept are fine

### Test Organization
- Group related tests in the same module
- Use descriptive test function names that explain the scenario and expected outcome
- Include both positive and negative test cases
- Test edge cases and boundary conditions

## Error Handling

### Problem Codes
Error handling is **critical** for developer experience. Follow these rules:

1. **Unique codes**: Each problem gets its own unique P#### code - never reuse codes
2. **Descriptive names**: Problem enum variants should clearly describe the issue
3. **Shared definitions**: Problem codes are defined in `compiler/problems/resources/problem-codes.csv`
4. **Documentation required**: Every problem code MUST have documentation in `docs/compiler/problems/P####.rst`

### Problem Code Format
```csv
Code,Name,Message
P0044,SubrangeOutOfBounds,Subrange is outside base type bounds
```

### Error Messages
- Provide clear, actionable error messages
- Include context about what was expected vs. what was found
- Use `Diagnostic::problem()` with appropriate `Label::span()` for source location

## Documentation Standards

### Problem Documentation Format
Each problem code must have a corresponding `.rst` file in `docs/compiler/problems/` with:

```rst
=====
P####
=====

.. problem-summary:: P####

[Clear description of when this error occurs]

Example
-------

The following code will generate error P####:

.. code-block::

   [Example that triggers the error]

[Explanation of why this is an error]

To fix this error, [solution]:

.. code-block::

   [Corrected example]
```

### Code Documentation
- **Best effort** documentation for now, but focus on public APIs
- Use Rust doc comments (`///`) for public functions and types
- Include examples in documentation when helpful
- Document complex algorithms or IEC 61131-3 specific behavior

### Example Synchronization
**Important**: Examples in documentation should also exist as tests in the Rust compiler to ensure documentation accuracy. Follow the existing naming conventions for test examples.

## IEC 61131-3 Compliance

### Compliance Levels
The compiler supports various levels of IEC 61131-3 compliance:

- **Parse everything**: The compiler should be able to parse any 61131-3 code
- **Compatibility flags**: Users pass flags/options to enable/disable specific syntax validation
- **Graceful degradation**: Invalid syntax should be parsed but flagged with appropriate problem codes

### Implementation Approach
- Design for flexibility in compliance checking
- Use feature flags or configuration options for different compliance levels
- Ensure error messages reference the relevant IEC 61131-3 standard sections when applicable

## Build System Integration

### Just Commands
Use `just` for all build tasks. Key commands:
- `just` - Run tests for the current component
- `just compile` - Build the component
- `just ci` - Run full CI pipeline
- `just devenv-smoke` - Quick environment check

For complete setup and development workflow instructions, see [CONTRIBUTING.md](../CONTRIBUTING.md).

### Synchronization Checks
The build system enforces synchronization between components:
- Version numbers must match across all components
- Problem codes must be documented
- Examples in docs must have corresponding tests

### Cross-Platform Support
- Support Windows, macOS, and Linux
- Use platform-specific just recipes when needed (`_command-{{os_family()}}`)
- Test in Dev Container environment when possible

## Performance Considerations

### Memory Usage
- Design for embedded/PLC contexts where memory may be constrained
- Use appropriate data structures for the compilation pipeline
- Consider memory layout for type representations (see `ByteSized` enum)

### Compilation Speed
- Optimize for reasonable compilation times
- Use efficient algorithms for type checking and semantic analysis
- Profile performance-critical paths when needed

## Code Quality

### Rust Best Practices
- Use `#[allow(dead_code)]` sparingly and only when justified
- Prefer `Result<T, E>` for error handling over panics
- Use appropriate visibility modifiers (`pub`, `pub(crate)`, etc.)
- Follow Rust naming conventions and idioms

### Safety
- Leverage Rust's safety guarantees
- Avoid `unsafe` code unless absolutely necessary
- Use strong typing to prevent logic errors (e.g., `TypeName` vs `String`)

### Dependencies
- Keep dependencies minimal and well-justified
- Use workspace dependencies for consistency
- Regular dependency updates via `just update`