# Problem Code Management

This steering file provides specific guidance for managing problem codes in IronPLC. It applies when working with error handling in the compiler or the VS Code extension.

> **Note**: This file covers the technical implementation of error handling. For general development workflow and build processes, see [CONTRIBUTING.md](../../CONTRIBUTING.md).

## Fundamental Rule

**Every user-facing error message MUST have a unique problem code.** This applies to all components — the compiler, the VS Code extension, and any other tooling. Error codes allow users to look up documentation, search for solutions, and report issues precisely. Never display an error to the user without an associated code.

- **Compiler errors** use the `P####` prefix (e.g., P2016)
- **VS Code extension errors** use the `E####` prefix (e.g., E0001)

## Applies To

This guidance is particularly relevant when working with:
- Files in `compiler/problems/*`
- Files in `integrations/vscode/src/*` (extension error handling)
- Files in `integrations/vscode/resources/problem-codes.csv`

## Problem Code Lifecycle

### Adding New Problem Codes

1. **Choose the next available code**: Use the next sequential P#### number
2. **Add to CSV**: Update `compiler/problems/resources/problem-codes.csv`
3. **Create documentation**: Add `docs/compiler/problems/P####.rst`
4. **Implement usage**: Use the problem code in diagnostic creation
5. **Add tests**: Verify the error is generated correctly

### CSV Format
```csv
Code,Name,Message
P2016,SubrangeOutOfBounds,Subrange is outside base type bounds
```

- **Code**: P#### format (e.g., P2016)
- **Name**: PascalCase enum variant name (e.g., SubrangeOutOfBounds)
- **Message**: Brief, generic description of the error class

### Documentation Template

Create `docs/compiler/problems/P####.rst`:

```rst
=====
P####
=====

.. problem-summary:: P####

[Clear description of when this error occurs and why it's a problem]

Example
-------

The following code will generate error P####:

.. code-block::

   TYPE
       [Example that demonstrates the error]
   END_TYPE

[Explanation of why this specific code triggers the error]

To fix this error, [specific guidance]:

.. code-block::

   TYPE
       [Corrected version of the example]
   END_TYPE

[Optional: Additional examples or edge cases]
```

## Problem Code Categories

### Compiler Problem Codes (P-prefix)

#### Parsing Errors (P0001-P1999)
- Syntax errors, unexpected tokens, malformed input
- Examples: P0001 (OpenComment), P0002 (SyntaxError)

#### Type System Errors (P2000-P3999)
- Type declaration issues, type compatibility problems
- Examples: P2002 (SubrangeMinStrictlyLessMax), P2016 (SubrangeOutOfBounds)

#### Semantic Analysis Errors (P4000-P5999)
- Variable scoping, function calls, semantic validation
- Examples: P4007 (VariableUndefined), P4012 (FunctionBlockNotInScope)

#### File System Errors (P6000-P7999)
- File I/O, path resolution, encoding issues
- Examples: P6001 (CannotCanonicalizePath), P6004 (CannotReadFile)

#### Internal Errors (P9000+)
- Compiler bugs and unimplemented features
- Examples: P9998 (InternalError), P9999 (NotImplemented)

### VS Code Extension Error Codes (E-prefix)

Extension errors use the `E####` format and are tracked separately from compiler problem codes.

- **Registry**: `integrations/vscode/resources/problem-codes.csv`
- **Documentation**: `docs/vscode/problems/E####.rst`
- **Index**: `docs/vscode/problems/index.rst`

#### Adding New Extension Error Codes

1. **Choose the next available code**: Use the next sequential E#### number
2. **Add to CSV**: Update `integrations/vscode/resources/problem-codes.csv`
3. **Run the build**: The `generate-problems` script auto-generates `src/problems.ts` from the CSV
4. **Create documentation**: Add `docs/vscode/problems/E####.rst`
5. **Update index**: Add the new entry to `docs/vscode/problems/index.rst`
6. **Use the generated constants**: Import `ProblemCode` and `formatProblem` from `./problems`

#### Extension Error Message Pattern

Error messages are generated from `integrations/vscode/resources/problem-codes.csv`
at build time. The generated `src/problems.ts` module provides typed constants and
a formatting helper. Use `formatProblem()` to compose messages:

```typescript
import { ProblemCode, formatProblem } from './problems';

// In VS Code notification messages:
vscode.window.showErrorMessage(
  formatProblem(ProblemCode.NoCompiler, 'IronPLC is not installed or not configured.'),
);

// In webview HTML error displays:
getErrorHtml(
  formatProblem(ProblemCode.ViewerCompilerNotFound, 'Install the compiler to view .iplc files.'),
);
```

The output format is: `{code} - {primary message}[. {contextual detail}]`

**Do not** hardcode error code strings (e.g., `'E0001 - ...'`) — always use the
generated constants so that messages stay in sync with the CSV and documentation.

#### Extension Error Documentation Template

Create `docs/vscode/problems/E####.rst`:

```rst
=====
E####
=====

.. problem-summary:: E####

[Clear description of when this error occurs]

**Solutions**:

1. [First solution with clear steps]
2. [Additional solutions]
```

## Implementation Patterns

### Creating Diagnostics

```rust
// Simple error with primary location
Diagnostic::problem(
    Problem::SubrangeOutOfBounds,
    Label::span(
        type_name.span(),
        format!(
            "Subrange [{}, {}] is outside base type bounds [{}, {}]",
            min_value, max_value, type_min, type_max
        ),
    ),
)

// Error with additional context
Diagnostic::problem(
    Problem::ParentTypeNotDeclared,
    Label::span(node_name.span(), "Subrange declaration"),
)
.with_secondary(Label::span(base_type_name.span(), "Base type not found"))
```

### Error Message Guidelines

1. **Be specific**: Include actual values when helpful
2. **Be actionable**: Tell the user what they need to fix
3. **Provide context**: Use secondary labels for related information
4. **Use consistent terminology**: Match IEC 61131-3 language

### Testing Error Conditions

```rust
#[test]
fn function_when_invalid_condition_then_p####_error() {
    // Arrange
    let invalid_input = create_invalid_scenario();

    // Act
    let result = function_under_test(invalid_input);

    // Assert
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(Problem::SpecificError.code(), error.code);
}
```

## Documentation Requirements

### Problem Documentation Must Include:

1. **Clear trigger condition**: When does this error occur?
2. **Concrete example**: Real code that triggers the error
3. **Explanation**: Why is this problematic?
4. **Solution**: How to fix the issue
5. **Corrected example**: Working version of the code

### Example Synchronization

**Critical**: Examples in problem documentation should have corresponding tests in the compiler to ensure accuracy.

```rust
#[test]
fn apply_when_subrange_out_of_bounds_then_error() {
    let program = "
TYPE
OUT_OF_BOUNDS : SINT (-200..200) := 0;
END_TYPE
    ";
    // This example should match the documentation for P2016
}
```

## Build Integration

### Automatic Validation

The build system automatically:
- Generates `Problem` enum from CSV
- Validates that all problem codes have documentation
- Ensures examples compile correctly

For complete build system usage and CI workflow, see [CONTRIBUTING.md](../../CONTRIBUTING.md).

### Build Failure Conditions

The build will fail if:
- Problem code is used but not defined in CSV
- Problem code is defined but not documented
- Documentation examples contain syntax errors
- Problem codes are duplicated

## Maintenance Guidelines

### Deprecating Problem Codes

**Never remove problem codes** - they may be referenced in user documentation or tooling.

Instead:
1. Mark as deprecated in CSV comments
2. Update documentation to indicate deprecation
3. Redirect users to replacement codes if applicable

### Refactoring Problem Codes

When refactoring code that uses problem codes:
1. Ensure the same logical errors still generate the same codes
2. Update tests to verify error code consistency
3. Update documentation if error conditions change

### Version Compatibility

Problem codes are part of the public API:
- Codes should remain stable across versions
- New codes can be added freely
- Existing codes should not change meaning
- Document any behavioral changes in release notes

## Quality Checklist

### Compiler Problem Codes (P-prefix)

Before adding a new compiler problem code:

- [ ] Code follows P#### format
- [ ] Name is descriptive and follows PascalCase
- [ ] Message is clear and generic
- [ ] Documentation file created with proper format
- [ ] Example code is realistic and demonstrates the error
- [ ] Corrected example shows proper solution
- [ ] Test exists that verifies the error is generated
- [ ] Test verifies the correct problem code is returned
- [ ] Build passes with new problem code

### Extension Error Codes (E-prefix)

Before adding a new extension error code:

- [ ] Code follows E#### format
- [ ] Added to `integrations/vscode/resources/problem-codes.csv`
- [ ] Build generates updated `src/problems.ts` (run `npm run generate-problems`)
- [ ] Error message uses `formatProblem(ProblemCode.Name, context)` — no hardcoded strings
- [ ] Documentation file created in `docs/vscode/problems/E####.rst`
- [ ] Added to `docs/vscode/problems/index.rst`
- [ ] Documentation describes the error condition and solutions

## Common Patterns

### Range Validation Errors
```rust
if min_value > max_value {
    return Err(Diagnostic::problem(
        Problem::SubrangeMinStrictlyLessMax,
        Label::span(node_name.span(), "Subrange instance"),
    ));
}
```

### Type Not Found Errors
```rust
let base_type = type_environment.get(&base_type_name).ok_or_else(|| {
    Diagnostic::problem(
        Problem::ParentTypeNotDeclared,
        Label::span(node_name.span(), "Type declaration"),
    )
    .with_secondary(Label::span(base_type_name.span(), "Type not found"))
})?;
```

### Bounds Checking Errors
```rust
if value < type_min || value > type_max {
    return Err(Diagnostic::problem(
        Problem::SubrangeOutOfBounds,
        Label::span(
            location.span(),
            format!("Value {} is outside type bounds [{}, {}]", value, type_min, type_max),
        ),
    ));
}
```
