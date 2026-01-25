# IEC 61131-3 Compliance Guidelines

This steering file provides guidance for implementing IEC 61131-3 standard compliance in the IronPLC compiler. It applies when working with semantic analysis and language feature implementation.

> **Note**: This file focuses on standard compliance principles. For general development setup and compiler workflow, see [compiler/CONTRIBUTING.md](../../compiler/CONTRIBUTING.md).

## Applies To

This guidance is particularly relevant when working with:
- Files in `**/analyzer/**` directories

## Compliance Philosophy

IronPLC follows a **permissive parsing, configurable validation** approach:

1. **Parse everything**: The compiler should parse any syntactically valid IEC 61131-3 code
2. **Configurable validation**: Use compatibility flags to enable/disable specific semantic rules
3. **Clear diagnostics**: When validation fails, provide clear problem codes and explanations

## Standard Compliance Levels

IEC 61131-3 compliance levels relate to vendor extensions and restrictions, not progressive feature implementation:

### Base Standard Compliance
- Full IEC 61131-3 standard implementation
- All required language constructs supported
- Standard-compliant semantic validation
- Complete type system as defined by the standard

### Vendor Extensions (Additive)
- Additional data types beyond the standard
- Extended function libraries
- Enhanced language constructs
- Proprietary features that don't conflict with standard

### Vendor Restrictions (Subtractive)
- Subset implementations that omit certain standard features
- Platform-specific limitations (e.g., memory constraints)
- Simplified language profiles for specific use cases
- Restricted feature sets for safety-critical applications

### Configuration Approach
The compiler should support configurable compliance through flags and profiles rather than hard-coded behavior.

## Type System Compliance

### Elementary Types
Support all IEC 61131-3 elementary types:
- Boolean types (BOOL)
- Bit string types (BYTE, WORD, DWORD, LWORD)
- Integer types (SINT, INT, DINT, LINT, USINT, UINT, UDINT, ULINT)
- Real number types (REAL, LREAL)
- Time and date types (TIME, DATE, etc.)
- String types (STRING, WSTRING)

### Derived Types
Implement standard-compliant derived types:
- **Subrange types**: Numeric ranges with proper bounds validation
- **Enumerated types**: Named value sets with unique identifiers
- **Array types**: Fixed-size collections with compile-time bounds
- **Structure types**: Named field collections with proper scoping

### Type Compatibility
Follow IEC 61131-3 assignment and operator compatibility rules:
- Strict type checking by default
- Well-defined implicit conversion rules
- Clear error messages for type mismatches
- Support for subrange-to-base-type compatibility

## Language Feature Compliance

### Variable Declarations
Enforce standard variable declaration rules:
- Proper scoping (local shadows global)
- Required initialization for constants
- Correct qualifier usage (INPUT, OUTPUT, IN_OUT, etc.)
- Standard default value behavior

### Functions and Function Blocks
Implement standard POU (Program Organization Unit) rules:
- **Functions**: Pure, stateless, return-value-required
- **Function Blocks**: Stateful, instance-based, multiple outputs allowed
- **Programs**: Top-level execution units
- Proper parameter passing semantics

### Standard Library
Support required standard library functions:
- Mathematical operations
- Type conversion functions
- String manipulation
- Comparison and selection functions

## Validation Principles

### Error Handling
- Use shared problem code system for compliance violations
- Reference general IEC 61131-3 compliance requirements in error messages
- Provide clear, actionable diagnostic information
- Support multiple error collection rather than fail-fast

### Testing Approach
- Create original test examples that demonstrate IEC 61131-3 compliance
- Test both compliant and non-compliant code patterns
- Verify correct problem codes are generated
- Include edge cases and boundary conditions

### Configurable Validation
Design validation to be configurable rather than fixed:
- Support different compliance profiles
- Allow enabling/disabling specific validation rules
- Provide clear configuration interfaces
- Maintain backward compatibility

## Implementation Guidelines

### Compliance Checking
- Separate parsing from semantic validation
- Make compliance rules configurable
- Support incremental validation
- Cache validation results when beneficial

### Standard Evolution
- Plan for multiple IEC 61131-3 standard versions
- Support feature flags for new standard additions
- Maintain compatibility with older versions
- Document compliance level clearly

### Vendor Extensions
- Provide clear extension points
- Avoid conflicts with standard features
- Document extension behavior
- Support gradual migration paths

## Future Considerations

### Tooling Integration
- Support IDE compliance checking
- Provide structured diagnostic information
- Enable compliance profile switching
- Support documentation generation

### Performance
- Minimize compliance checking overhead
- Design for reasonable compilation times
- Support fast paths for common cases
- Profile compliance-critical code paths

### Maintenance
- Keep compliance rules up to date with standard
- Create original test examples that demonstrate compliance
- Document compliance decisions and rationale
- Support compliance testing and certification
