# IronPLC Development Standards

This steering file defines the core development standards and patterns for the IronPLC project, a Rust-based PLC compiler implementing the IEC 61131-3 standard.

> **Note**: This file provides detailed implementation guidance for AI-assisted development. For development workflow, setup instructions, and contribution processes, see the main [CONTRIBUTING.md](../../CONTRIBUTING.md) and component-specific contributing guides.

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
- Problem codes follow the pattern `P####` (e.g., P2016)
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
- **No panic! in tests** - Use `assert!` macros instead of `panic!()` for test failures
  - ❌ Bad: `match result { Ok(x) => assert_eq!(x, 5), _ => panic!("Expected Ok") }`
  - ✅ Good: `assert!(result.is_ok()); assert_eq!(result.unwrap(), 5);`
  - ✅ Better: `assert!(matches!(result, Ok(5)))`

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
P2016,SubrangeOutOfBounds,Subrange is outside base type bounds
```

### Error Messages
- Provide clear, actionable error messages
- Include context about what was expected vs. what was found
- Use `Diagnostic::problem()` with appropriate `Label::span()` for source location

## Steering Files for AI Assistants

IronPLC uses **steering files** to guide AI assistants in working with the codebase. These files follow a specific two-file pattern:

- **Pointer files** in `.kiro/steering/` - Lightweight references loaded automatically by Kiro
- **Detailed docs** in `specs/steering/` - Complete guidance that works with any AI system

### Creating and Maintaining Steering Files

When creating or updating steering files:

1. **Use the two-file pattern** - Create detailed doc in `specs/steering/`, pointer in `.kiro/steering/`
2. **Keep pointers minimal** - 3-5 lines with a reference to the detailed doc
3. **Make detailed docs self-contained** - Should work when copied to any AI system
4. **Update CLAUDE.md** - Add references to new steering files
5. **Choose appropriate inclusion** - `always`, `fileMatch`, or `manual`

For complete guidance on steering files, see [steering-file-guidelines.md](./steering-file-guidelines.md).

## Documentation Standards

### Documentation Quadrants Framework
All IronPLC documentation follows the **Documentation Quadrants** approach, organizing content into four distinct types:

#### 1. Tutorials (Learning-Oriented)
- **Purpose**: Guide newcomers through their first successful experience
- **Audience**: People studying and learning
- **Content**: Step-by-step lessons that work reliably
- **Examples**: "Getting Started with IronPLC", "Your First PLC Program"
- **Location**: `docs/tutorials/`

#### 2. How-To Guides (Problem-Oriented)
- **Purpose**: Show how to solve specific real-world problems
- **Audience**: Practitioners at work who need to accomplish something
- **Content**: Series of steps focused on achieving a goal
- **Examples**: "How to Debug Compilation Errors", "How to Add a New Data Type"
- **Location**: `docs/how-to/`

#### 3. Technical Reference (Information-Oriented)
- **Purpose**: Describe the machinery and how to operate it
- **Audience**: Practitioners at work who need accurate information
- **Content**: Structured descriptions of APIs, commands, and features
- **Examples**: "Compiler CLI Reference", "Problem Code Reference", "Language Grammar"
- **Location**: `docs/reference/`

#### 4. Explanation (Understanding-Oriented)
- **Purpose**: Clarify and illuminate topics for deeper understanding
- **Audience**: People studying who want to understand concepts
- **Content**: Discussions of design decisions, alternatives, and context
- **Examples**: "IEC 61131-3 Compliance Strategy", "Compiler Architecture Overview"
- **Location**: `docs/explanation/`

### Documentation Relationships
- **Tutorials + How-To Guides**: Both describe practical steps
- **How-To Guides + Reference**: Both serve practitioners at work
- **Reference + Explanation**: Both provide theoretical knowledge
- **Tutorials + Explanation**: Both support learning and study

### RST Annotation Conventions

All Sphinx documentation must use the correct RST roles for consistent rendering. **Never use plain text or double backticks for elements that have a dedicated role.**

| Element | Role | Example |
|---------|------|---------|
| Menu paths | `:menuselection:` | `:menuselection:\`File --> New File...\`` |
| UI elements (buttons, panels) | `:guilabel:` | `:guilabel:\`Install\`` |
| Keyboard shortcuts | `:kbd:` | `:kbd:\`Ctrl+Shift+P\`` |
| File names and extensions | `:file:` | `:file:\`main.st\``, `:file:\`.st\`` |
| Commands and executables | `:program:` | `:program:\`ironplcc --version\`` |
| Code keywords | `:code:` | `:code:\`PROGRAM\`` |
| User-typed text | `:samp:` | `:samp:\`IronPLC\`` |
| Cross-document links | `:doc:` | `:doc:\`/compiler/problems/index\`` |

**Menu paths** use ` --> ` as separator: `:menuselection:\`File --> Preferences --> Settings\``

**Platform-specific keyboard shortcuts** use separate `:kbd:` roles: `:kbd:\`Ctrl+Shift+X\`` for Windows/Linux, `:kbd:\`⌘+Shift+X\`` for macOS.

### Documentation Content Guidelines

- **Do not document architecture or internals** in user-facing reference docs. Architecture belongs in `docs/explanation/` if anywhere.
- **Do not explain standard VS Code concepts** (e.g., workspace vs. user settings). Assume the reader knows VS Code.
- **Use platform tabs** (via `sphinx_inline_tabs`) for platform-specific instructions.

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

### Supported File Format Synchronization
File extensions and format details are listed in **two** canonical locations. All other docs cross-reference these rather than repeating extension lists. When adding or modifying a supported source file format, update:

1. **Compiler source** - `compiler/sources/src/file_type.rs` (the source of truth for detection)
2. **VS Code extension** - `integrations/vscode/package.json` (language contributions) and `integrations/vscode/src/extension.ts` (document selector)
3. **Source format reference page** - the format-specific page in `docs/compiler/source-formats/` (e.g., `twincat.rst`)
4. **VS Code overview** - `docs/vscode/overview.rst` (Supported Languages section)

### README Synchronization
The project has multiple README files that must stay synchronized:

- **Root `README.md`**: Main project overview, mission, progress, and capabilities
- **`integrations/vscode/README.md`**: VS Code Extension specific documentation for the Marketplace

**When updating the main README:**
1. Review if the extension README needs corresponding updates
2. The extension README should reflect the same capabilities/limitations
3. Keep the "warning" banner (`⚠`) consistent between both files
4. Ensure feature lists match (e.g., syntax highlighting, analysis capabilities)

**When updating the extension README:**
1. Keep it focused on VS Code-specific usage and features
2. Include extension settings, commands, and configuration
3. Reference the main documentation website for detailed information

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
- `just` - Run full CI pipeline for the current component
- `just compile` - Build the component
- `just test` - Run tests
- `just lint` - Run linting (clippy + fmt for Rust)
- `just devenv-smoke` - Quick environment check

For complete setup and development workflow instructions, see [CONTRIBUTING.md](../../CONTRIBUTING.md).

### CRITICAL: Git Workflow and Pre-PR Quality Gate

**NEVER commit or push directly to `main`.** Always create a feature branch and open a pull request. This ensures CI validates all changes before they reach main.

**Before creating any pull request, you MUST run and pass the full CI pipeline:**

```bash
cd compiler && just
```

This runs compile, test, coverage, and lint. The **lint step includes clippy**, which catches common Rust issues. PRs that fail clippy will be rejected by CI.

**Do not:**
- Push directly to `main` — always use a feature branch and PR
- Skip running `just` before creating a PR
- Suppress clippy warnings with `#[allow(...)]` unless justified
- Create a PR if any check fails

See [common-tasks.md](./common-tasks.md) for detailed pre-PR requirements and troubleshooting.

### Version Management
**Version numbers are generated and incremented automatically** - no manual version management is required:

- **Automated versioning**: The build system handles version increments automatically
- **No manual updates**: Do not manually edit version numbers in `Cargo.toml` or other files
- **Synchronization**: The build system ensures version numbers stay synchronized across all components
- **Release process**: Version bumps happen as part of the automated release workflow

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
