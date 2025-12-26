# IronPLC Compiler Architecture

This steering file provides high-level architectural guidance and principles for the IronPLC compiler. It focuses on structural patterns rather than specific implementation details.

> **Note**: This file focuses on architectural principles and patterns. For compiler development setup, debugging tools, and workflow, see [compiler/CONTRIBUTING.md](../../compiler/CONTRIBUTING.md).

## Compiler Pipeline

The IronPLC compiler follows a traditional multi-stage compilation pipeline:

1. **Parser** (`parser/`) - Converts source text to AST
2. **Analyzer** (`analyzer/`) - Semantic analysis and type checking
3. **Code Generation** (future) - Generate target code

## Architectural Principles

### Single Responsibility
- Each module should have one clear purpose
- Avoid mixing unrelated functionality
- Prefer composition over large monolithic modules

### Separation of Concerns
- Parse syntax separately from semantic validation
- Keep type checking separate from code generation
- Isolate error handling from business logic

### Fail Fast and Clear
- Validate inputs early in the pipeline
- Provide clear, actionable error messages
- Use the shared problem code system consistently

## Module Organization

### Size Constraints
**Critical**: Keep analyzer/transformation modules small and tightly scoped:

- **Maximum 1000 lines of code** per module (except where absolutely necessary)
- **Single responsibility**: Each module should handle one specific aspect of analysis
- **Focused purpose**: Avoid combining unrelated functionality in the same module
- **Split when needed**: If a module grows beyond 1000 lines, split it into smaller, focused modules

### Naming Conventions
- `xform_*` modules handle transformations
- `intermediate_*` modules define data structures
- `*_environment` modules manage symbol tables and contexts
- Use descriptive names that reflect the module's purpose

### Directory Structure
- Group related functionality in subdirectories (e.g., `intermediates/`)
- Keep the module hierarchy shallow and intuitive
- Organize by compilation phase or language feature

## Semantic Analysis Patterns

### Validation Functions
Create focused validation functions that:
- Take specific input types and contexts
- Return clear success/failure results
- Use the shared diagnostic system for errors
- Handle one validation concern at a time

### Transform Functions
Use consistent patterns for AST transformations:
- `try_from` pattern for fallible conversions
- Match on AST variants systematically
- Return structured intermediate results
- Propagate errors using `?` operator

### Error Handling
- Use `Result<T, Diagnostic>` for fallible operations
- Provide rich diagnostic information with source spans
- Collect multiple errors when possible
- Use appropriate problem codes from the shared system

## Testing Architecture

### Test Organization
- Follow BDD-style naming conventions
- Group tests by the functionality they validate
- Use helper functions for common test setup
- Keep tests focused and independent

### Test Coverage
- Test both success and failure cases
- Include edge cases and boundary conditions
- Verify error codes and messages
- Create original IEC 61131-3 compliant test examples

For information on running tests, coverage analysis, and debugging tools, see [compiler/CONTRIBUTING.md](../../compiler/CONTRIBUTING.md).

## Performance Guidelines

### Memory Management
- Use `Box<T>` for recursive type definitions
- Avoid `Rc<T>` or `Arc<T>` unless sharing is essential
- Prefer owned data over reference counting
- Multiple compilation passes are acceptable for clarity

### Compilation Efficiency
- Design for reasonable compilation times
- Profile performance-critical paths when needed
- Optimize for maintainability over micro-optimizations
- Cache expensive computations when beneficial

## Extension Guidelines

### Adding New Language Features
1. **Parser**: Update to recognize new syntax
2. **AST**: Add nodes for new constructs
3. **Analyzer**: Implement semantic validation
4. **Tests**: Add comprehensive test coverage
5. **Documentation**: Update problem codes and docs

### Adding New Analysis Passes
1. Create focused modules under 1000 lines
2. Use consistent transformation patterns
3. Integrate with existing error handling
4. Add appropriate test coverage
5. Document the analysis purpose and scope

### Adding New Problem Codes
Follow the established problem code lifecycle:
1. Add to shared CSV definition
2. Create documentation
3. Implement diagnostic usage
4. Add verification tests

## Future Considerations

### Incremental Compilation
- Design modules to support incremental analysis
- Consider caching intermediate results
- Plan for language server integration

### Multiple Targets
- Keep analysis separate from code generation
- Design intermediate representations for flexibility
- Plan for different compliance levels and profiles

### Tooling Integration
- Support IDE features through clear interfaces
- Provide structured diagnostic information
- Design for interactive development workflows