# IronPLC Compiler Architecture

This steering file provides high-level architectural guidance and principles for the IronPLC compiler. It focuses on structural patterns rather than specific implementation details.

> **Note**: This file focuses on architectural principles and patterns. For compiler development setup, debugging tools, and workflow, see [compiler/CONTRIBUTING.md](../../compiler/CONTRIBUTING.md).

## Compiler Pipeline

The IronPLC compiler follows a traditional multi-stage compilation pipeline:

1. **Parser** (`parser/`) - Converts source text to AST
2. **Analyzer** (`analyzer/`) - Semantic analysis and type checking
3. **Code Generation** (`codegen/`) - Generates bytecode for the IronPLC VM.
   `ironplc_codegen::compile` takes a `CleanAnalysis`, which the analyzer
   makes only from a semantic context that holds no diagnostics. Its doc
   comment says what that does and does not guarantee.

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
- `semantic_*` modules define data structures
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

**A semantic rule cannot fail.** A rule's visitor implements
`DiagnosticVisitor` (`compiler/analyzer/src/rule_support.rs`), whose error type
is `Infallible`, and `rule_support::run_rule` drives the walk. Report a problem
by pushing onto the visitor's `diagnostics` field, which `into_diagnostics`
surrenders; there is no `Err` to return. A rule that meets a node it cannot
analyse records a diagnostic and stops descending into *that node*, never the
walk --- otherwise every problem after it goes unreported. See
[ADR-0048](../adrs/0048-semantic-rules-cannot-fail.md).

This does not apply to the `xform_*` passes, which must produce a `Library`.
Each runs under one of two failure policies, named by the helper it is called
through in `stages::resolve_types`:

| Signature | Called through | On a problem |
|---|---|---|
| `Result<(Library, Vec<Diagnostic>), Vec<Diagnostic>>` | `run_best_effort` | The transformed library is kept and the diagnostics collected alongside it. `Err` means the pass had no library to return at all. |
| `Result<Library, Vec<Diagnostic>>` | `run_reverting_on_error` | The pre-pass clone is restored, discarding every transformation the pass had already completed. |

**A new pass reports per-declaration problems through `run_best_effort`.** A
pass that accumulates diagnostics and then returns `Err` throws away every
unrelated declaration it had already transformed, which is how a source that
analyzed cleanly alone came to fail once merged with unrelated code. Revert is
for a pass whose whole output is meaningless when any part of it failed, and
the call site says why.

Best effort is a per-declaration decision, not a licence to emit a
half-transformed node. A declaration the pass could not transform is left in a
state later passes already handle — unchanged, or normalized to a placeholder —
and nothing the author wrote is dropped from it.

### Name and Type Lookup

**A rule looks a name up in the environments; it never builds a table of its
own.** Resolution builds one answer to "what does this name declare, and what is
its type", and every rule reads that answer:

- `SymbolEnvironment::find(name, scope)` (`symbol_environment.rs`) resolves a
  name from a scope: the enclosing scopes innermost first, then the function
  blocks the outermost one `EXTENDS`, then the global scope. Its `SymbolInfo`
  carries the variable's section, declared qualifier, address and `type_id`. A
  function's or method's own name is its `ResultVariable`.
- `ScopeTracker` names the scope a visitor is in. Feed it from `enter_scope`
  and `exit_scope`, and pass `current()` to `find`.
- `variable_type::declared` gives a name's declared `SemanticType`, and
  `variable_type::of` walks a reference such as `s.field[i]` to the element it
  names. `TypeEnvironment::get_by_id` answers for any `type_id`.

A rule that copies declarations into a `HashMap` or `ScopedTable` of its own as
it walks gets the scoping wrong in ways the environment already gets right: it
sees only what it has visited so far, lets one method's locals leak into the
next, or misses an inherited field. When the environment lacks something a rule
needs, add it to the environment, where every rule then has it, rather than
working around it in the rule.

Test such a rule against the context resolution builds: the `rule_ok!`,
`rule_err!` and `rule_err_at!` macros in `test_macros.rs`, and the helpers in
`test_helpers.rs`, all do. See [Rule Tests](compiler-standards.md#rule-tests)
for what a rule test asserts.

## Testing Architecture

### Test Organization
- Follow BDD-style naming conventions
- Group tests by the functionality they validate
- Use helper functions for common test setup
- Keep tests focused and independent

### External Test Files for VM and Codegen

Operator/opcode-specific tests live in **external test files**, not inline `#[cfg(test)]` modules. Inline tests are reserved for infrastructure concerns (VM state transitions, general error paths, private API unit tests).

#### VM crate (`compiler/vm/tests/`)

Per-opcode integration tests that exercise the VM directly with hand-crafted bytecode:

| File pattern | Purpose | Example |
|---|---|---|
| `execute_<op>_i32.rs` | Tests for a single opcode | `execute_add_i32.rs`, `execute_div_i32.rs` |
| `common/mod.rs` | Shared helpers (`VmBuffers`, `single_function_container`, `assert_trap`) | — |
| `scenarios.rs` | Multi-scan, multi-task, scope tests | — |
| `steel_thread.rs` | Serialization roundtrip | — |

Template for a new opcode file:
```rust
//! Integration tests for the <OP>_I32 opcode.
mod common;
use common::{assert_trap, single_function_container, VmBuffers};
use ironplc_vm::error::Trap;
use ironplc_vm::Vm;
```

#### Codegen crate (`compiler/codegen/tests/`)

Codegen behavior is tested **end-to-end** (parse → compile → VM run → check
variable values). A wrong opcode, offset, or layout produces a wrong runtime
result, which the end-to-end test catches — so a separate per-operator bytecode
test is redundant for *behavior*.

| File pattern | Purpose | Example |
|---|---|---|
| `end_to_end_<op>.rs` | Runtime assertions — the primary behavioral layer | `end_to_end_add.rs`, `end_to_end_div.rs` |
| `end_to_end.rs` | General infrastructure tests (assignment, scan behavior) | — |
| `wire_format.rs` | **Backwards-compatibility guard**: pins every opcode's byte value + a completeness test | — |
| `compile_<op>.rs` | Bytecode assertions — **only for structure end-to-end cannot localize** (jump/branch offsets, struct/array/frame offsets, operand widths, peephole), and variable layout | `compile_loops.rs`, `compile_struct.rs`, `compile_system_uptime.rs` |
| `vm_api_<api>.rs` | The VM's embedder API, which addresses variables by slot; slots are looked up by name with `vm_var_index` | `vm_api_write_variable_raw.rs` |
| `common/` | Shared helpers: the `e2e!`/`e2e_*!` macros, `assert_run`, `Snapshot`, `run_scans`, `drive_fb`, `parse`, and `parse_and_compile`, which builds the `CleanAnalysis` that `compile` takes | — |

**The one thing end-to-end cannot catch is a consistent opcode *renumber*** (the
compiler emits and the VM reads the new value, so a from-source compile+run still
passes) — that would silently break already-compiled `.iplc` containers.
`wire_format.rs` is the single canonical guard: it pins every opcode's byte value
and has a completeness test (backed by `container::opcode::is_assigned`) that fails
if an opcode is added, removed, or renumbered without updating the pins. Do **not**
add per-operator `compile_<op>.rs` files just to assert "the right opcode was
emitted" — that is covered by `end_to_end_<op>.rs` + `wire_format.rs`. Add a
`compile_<op>.rs` only when you need to pin *structure* (offsets/widths/peephole).

Template for a structural bytecode test file:
```rust
//! Bytecode-level integration tests for <OP> — structure only
//! (offsets/widths/peephole). Behavior is covered by end_to_end_<op>.rs.
use ironplc_parser::options::CompilerOptions;

use crate::common::{bc, parse_and_compile};
```

Template for a new end-to-end test file (variables are read by name, see
[End-to-End Test Observation](../design/end-to-end-test-observation.md)):
```rust
//! End-to-end integration tests for the <OP> operator.

e2e_i32!(
    end_to_end_when_<op>_then_<result>,
    "PROGRAM main VAR x : DINT; END_VAR x := <expression>; END_PROGRAM",
    &[("x", <expected>)],
);
```

#### What stays inline

- **`emit.rs`**: Emitter unit tests (private module — `Emitter` accessed via `super::*`)
- **`compile.rs`**: General compiler tests (assignment, error paths, constant dedup)
- **`vm.rs`**: VM lifecycle tests (state transitions, generic trap paths, empty bytecode)

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
3. Look names and types up in the environments (see [Name and Type Lookup](#name-and-type-lookup))
4. Integrate with existing error handling
5. Add appropriate test coverage
6. Document the analysis purpose and scope

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
