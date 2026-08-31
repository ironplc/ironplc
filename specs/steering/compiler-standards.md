# Compiler Standards

This steering file defines the coding standards and conventions for the IronPLC
compiler — the Rust workspace under `compiler/`. It covers module organization,
testing, error handling, performance, and Rust-specific rules.

> **Note**: This file covers *how to write compiler code*. For the compiler's
> structure and pipeline, see [compiler-architecture.md](compiler-architecture.md).
> For problem-code mechanics, see
> [problem-code-management.md](problem-code-management.md). For adding syntax, see
> [syntax-support-guide.md](syntax-support-guide.md). For build/test commands and
> the pre-PR CI gate, see [common-tasks.md](common-tasks.md).

## Applies To

This guidance is relevant when working with Rust code in `compiler/**` (all
crates). Cross-component process rules — planning, prefactoring, the specs
directory, the git workflow — live in
[development-standards.md](development-standards.md).

## Code Organization

### Module Structure
- Follow the existing pattern of organizing related functionality in subdirectories (e.g., `analyzer/src/intermediates/`)
- Use descriptive module names that reflect their purpose in the compilation pipeline
- Keep modules focused on a single responsibility
- **Maximum 1000 lines per module.** Split a module that grows past the limit
  into smaller, focused modules (see
  [compiler-architecture.md](compiler-architecture.md#size-constraints))

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
- **No panic! in tests** - Use `assert!` macros instead of `panic!()` for test failures
  - ❌ Bad: `match result { Ok(x) => assert_eq!(x, 5), _ => panic!("Expected Ok") }`
  - ✅ Good: `assert!(result.is_ok()); assert_eq!(result.unwrap(), 5);`
  - ✅ Better: `assert!(matches!(result, Ok(5)))`

### Test Organization
- Group related tests in the same module
- Use descriptive test function names that explain the scenario and expected outcome
- Include both positive and negative test cases
- Test edge cases and boundary conditions

For where each kind of test lives (VM, codegen, plc2plc, parser) and which leg
asserts what, see [compiler-architecture.md](compiler-architecture.md#testing-architecture)
and [syntax-support-guide.md](syntax-support-guide.md).

## Error Handling

Every user-facing error message **must** have a unique `P####` problem code. The
full lifecycle — choosing a code, the CSV registry, the documentation template,
and the diagnostic-construction patterns — lives in
[problem-code-management.md](problem-code-management.md). Do not restate that
material here.

When writing compiler code:
- Prefer `Result<T, Diagnostic>` for fallible operations; propagate with `?`
- Collect multiple diagnostics rather than failing on the first, where practical
- Provide clear, actionable error messages: include what was expected vs. found
- Use `Diagnostic::problem()` with an appropriate `Label::span()` for source location
- Match IEC 61131-3 terminology in messages

## Code Documentation
- **Best effort** documentation for now, but focus on public APIs
- Use Rust doc comments (`///`) for public functions and types
- Include examples in documentation when helpful
- Document complex algorithms or IEC 61131-3 specific behavior
- A comment carrying a decision states what was chosen and why, not only what
  the code does — the next reader's question is whether they may change it
- Nothing checks that a comment is still true. A comment asserting behaviour is
  as capable of going stale as a design document, and is read more often; when
  you change behaviour, re-read the comments around it

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
- Do not suppress clippy warnings with `#[allow(...)]` — fix the underlying code instead. The only acceptable exception is `#[allow(dead_code)]` or `#[allow(unused_*)]` for in-progress code that is not yet wired up; remove these suppressions once the code is complete
- Prefer `Result<T, E>` for error handling over panics
- Use appropriate visibility modifiers (`pub`, `pub(crate)`, etc.)
- Follow Rust naming conventions and idioms

### Safety
- Leverage Rust's safety guarantees
- **`unsafe` code is rejected at compile time.** The workspace sets `unsafe_code = "deny"` in `[workspace.lints.rust]` (root `compiler/Cargo.toml`), and every member crate inherits it via `[lints] workspace = true`. Any `unsafe` block, function, trait, or impl in IronPLC code fails the build
- **Do not bypass the check with `#[allow(unsafe_code)]`.** The standards already forbid `#[allow(...)]` suppressions (see [Rust Best Practices](#rust-best-practices)); `unsafe_code` is no exception. `deny` (rather than `forbid`) is the chosen level only so that proc-macros which wrap unsafe internally — e.g. `ctor::ctor`, whose expansion includes `#[allow(unsafe_code)]` — keep working. If a feature appears to require `unsafe`, raise it for discussion
- Use strong typing to prevent logic errors (e.g., `TypeName` vs `String`)

### Dependencies
- Keep dependencies minimal and well-justified
- Use workspace dependencies for consistency
- Regular dependency updates via `just update`

## Cross-Platform Support
- Support Windows, macOS, and Linux
- Use platform-specific just recipes when needed (`_command-{{os_family()}}`)
- Test in Dev Container environment when possible

## IEC 61131-3 Compliance

The compiler follows a **permissive parsing, configurable validation** approach.
The compliance philosophy, type-system expectations, and validation rules live in
[iec-61131-3-compliance.md](iec-61131-3-compliance.md) (especially relevant for
`**/analyzer/**`). Reference the relevant IEC 61131-3 standard sections in error
messages when applicable.
