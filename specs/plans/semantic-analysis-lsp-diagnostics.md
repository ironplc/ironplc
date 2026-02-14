# Plan: Return SemanticContext Even When Semantic Analysis Has Diagnostics

## Problem

The `analyze()` function in `compiler/analyzer/src/stages.rs` returns `Result<SemanticContext, Vec<Diagnostic>>`. When any semantic validation rule finds an error, the entire result is `Err(Vec<Diagnostic>)` and the `SemanticContext` is discarded.

In `compiler/plc2x/src/project.rs`, `FileBackedProject::semantic()` only caches the `SemanticContext` on `Ok`:

```rust
match analyze(&all_libraries) {
    Ok(context) => {
        self.semantic_context = Some(context);
        Ok(())
    }
    Err(diagnostics) => {
        self.semantic_context = None;  // Context lost!
        all_diagnostics.extend(diagnostics);
        Err(all_diagnostics)
    }
}
```

The LSP uses the cached `SemanticContext` for IDE features such as document symbols. When semantic analysis reports any diagnostic, the LSP loses all symbol and type information. This degrades the user experience: the outline view goes blank, go-to-symbol stops working, and any future features built on `SemanticContext` (hover, go-to-definition) also break.

### Why This Matters

The analysis pipeline has two distinct phases:

1. **Type resolution** (`resolve_types`): Builds environments (types, functions, symbols) by processing declarations. Produces a `SemanticContext`.
2. **Validation** (`semantic`): Runs 14 validation rules against the library using the context. Produces diagnostics but does not modify the context.

When a validation rule fails (e.g., an undefined variable reference, a subrange with invalid limits), the `SemanticContext` from phase 1 is fully valid and contains useful information. Yet it is thrown away because `analyze()` returns `Err`.

## Design

### Core Change: Diagnostics Move Into SemanticContext

Add a `diagnostics: Vec<Diagnostic>` field to `SemanticContext`. Instead of returning diagnostics through the `Result` error channel, accumulate them in the context.

```rust
// In semantic_context.rs
pub struct SemanticContext {
    pub types: TypeEnvironment,
    pub functions: FunctionEnvironment,
    pub symbols: SymbolEnvironment,
    diagnostics: Vec<Diagnostic>,
}
```

Add methods to `SemanticContext`:

```rust
impl SemanticContext {
    /// Adds a diagnostic to the context.
    pub fn add_diagnostic(&mut self, diagnostic: Diagnostic) { ... }

    /// Adds multiple diagnostics to the context.
    pub fn add_diagnostics(&mut self, diagnostics: Vec<Diagnostic>) { ... }

    /// Returns the diagnostics collected during analysis.
    pub fn diagnostics(&self) -> &[Diagnostic] { ... }

    /// Returns true if there are any diagnostics (errors).
    pub fn has_diagnostics(&self) -> bool { ... }
}
```

### Change analyze() Return Type

Change `analyze()` to always return a `SemanticContext` when type resolution succeeds:

```rust
// Current
pub fn analyze(sources: &[&Library]) -> Result<SemanticContext, Vec<Diagnostic>>

// New
pub fn analyze(sources: &[&Library]) -> Result<SemanticContext, Vec<Diagnostic>>
```

The return type stays the same, but the semantics change:

- **`Ok(context)`**: Type resolution succeeded. The context contains all type, function, and symbol information. `context.diagnostics()` may contain validation errors. This is the normal case.
- **`Err(diagnostics)`**: Type resolution itself failed completely (e.g., no sources provided). No useful context could be built. This should be rare — see discussion below.

### Change the semantic() Function

The internal `semantic()` function currently returns `SemanticResult` (`Result<(), Vec<Diagnostic>>`). Change it to write diagnostics into the context:

```rust
// Current
pub(crate) fn semantic(library: &Library, context: &SemanticContext) -> SemanticResult

// New
pub(crate) fn semantic(library: &Library, context: &mut SemanticContext)
```

Each validation rule's diagnostics are added to the context rather than returned as errors.

### Change analyze() Implementation

```rust
pub fn analyze(sources: &[&Library]) -> Result<SemanticContext, Vec<Diagnostic>> {
    if sources.is_empty() {
        return Err(vec![...]);
    }
    let (library, mut context) = resolve_types(sources)?;
    semantic(&library, &mut context);    // No longer returns Result

    let type_table_result = type_table::apply(&library);
    // Handle type_table errors by adding to context.diagnostics if needed

    Ok(context)
}
```

### Change the Consumer: FileBackedProject

In `compiler/plc2x/src/project.rs`, `FileBackedProject::semantic()` changes to always cache the context when available:

```rust
fn semantic(&mut self) -> Result<(), Vec<Diagnostic>> {
    self.semantic_context = None;

    let mut all_libraries = vec![];
    let mut all_diagnostics: Vec<Diagnostic> = vec![];

    for source in self.source_project.sources_mut() {
        match source.library() {
            Ok(library) => all_libraries.push(library),
            Err(diagnostics) => {
                for diagnostic in diagnostics {
                    all_diagnostics.push(diagnostic.clone());
                }
            }
        }
    }

    match analyze(&all_libraries) {
        Ok(context) => {
            // Always cache the context — it's useful even with diagnostics
            all_diagnostics.extend(context.diagnostics().to_vec());
            self.semantic_context = Some(context);

            if all_diagnostics.is_empty() {
                Ok(())
            } else {
                Err(all_diagnostics)
            }
        }
        Err(diagnostics) => {
            all_diagnostics.extend(diagnostics);
            Err(all_diagnostics)
        }
    }
}
```

### Change the Consumer: LspProject

In `compiler/plc2x/src/lsp_project.rs`, `LspProject::semantic()` continues to work — it already extracts diagnostics from the `Err` case. The change is that `document_symbols()` and future IDE features now work even when the file has semantic errors, because `semantic_context()` returns `Some` instead of `None`.

### Change Validation Rule Signatures

Each `rule_*.rs` module currently has:

```rust
pub fn apply(lib: &Library, context: &SemanticContext) -> SemanticResult
```

Change to:

```rust
pub fn apply(lib: &Library, context: &mut SemanticContext)
```

Each rule adds its diagnostics directly to the context via `context.add_diagnostic()` or `context.add_diagnostics()` instead of collecting them in a local vec and returning `Err`.

### No Change to resolve_types()

`resolve_types()` continues to return `Result<(Library, SemanticContext), Vec<Diagnostic>>`. If type resolution fails (circular dependencies, unresolvable types), it is a hard failure and no useful context is available. This is the only path that produces `Err` from `analyze()`.

Note: In the future, `resolve_types()` could also be made more resilient (accumulating errors and building a partial context), but this is out of scope for this change.

## When Does analyze() Return Err?

After this change, `Err` only occurs when:

1. **No sources**: `sources.is_empty()` — the caller provided nothing to analyze.
2. **Type resolution failure**: One of the `xform_*` transforms in `resolve_types()` fails. This happens when:
   - Circular type dependencies are detected (`xform_toposort_declarations`)
   - A type cannot be resolved (`xform_resolve_type_decl_environment`)
   - A late-bound expression kind cannot be resolved (`xform_resolve_late_bound_expr_kind`)
   - A type initializer cannot be resolved (`xform_resolve_late_bound_type_initializer`)
   - Symbols/functions cannot be resolved (`xform_resolve_symbol_and_function_environment`)
   - Type aliases cannot be resolved (`xform_resolve_type_aliases`)

These are cases where the program structure is fundamentally broken and the environments cannot be reliably built. The LSP would show parse/resolution errors to the user and the lack of symbol information is expected.

## Affected Files

### Analyzer Crate (`compiler/analyzer/`)

| File | Change |
|------|--------|
| `src/semantic_context.rs` | Add `diagnostics` field and methods |
| `src/result.rs` | Remove or repurpose `SemanticResult` type alias |
| `src/stages.rs` | Change `semantic()` to write to context; update `analyze()` |
| `src/rule_decl_struct_element_unique_names.rs` | Change to write diagnostics to context |
| `src/rule_decl_subrange_limits.rs` | Change to write diagnostics to context |
| `src/rule_enumeration_values_unique.rs` | Change to write diagnostics to context |
| `src/rule_function_block_invocation.rs` | Change to write diagnostics to context |
| `src/rule_function_call_declared.rs` | Change to write diagnostics to context |
| `src/rule_program_task_definition_exists.rs` | Change to write diagnostics to context |
| `src/rule_stdlib_type_redefinition.rs` | Change to write diagnostics to context |
| `src/rule_use_declared_enumerated_value.rs` | Change to write diagnostics to context |
| `src/rule_use_declared_symbolic_var.rs` | Change to write diagnostics to context |
| `src/rule_unsupported_stdlib_type.rs` | Change to write diagnostics to context |
| `src/rule_var_decl_const_initialized.rs` | Change to write diagnostics to context |
| `src/rule_var_decl_const_not_fb.rs` | Change to write diagnostics to context |
| `src/rule_var_decl_global_const_requires_external_const.rs` | Change to write diagnostics to context |
| `src/rule_pou_hierarchy.rs` | Change to write diagnostics to context |

### Project/LSP Crate (`compiler/plc2x/`)

| File | Change |
|------|--------|
| `src/project.rs` | Cache context even when there are diagnostics |
| `src/lsp_project.rs` | No functional change needed (already handles both paths) |

## Implementation Order

1. Add `diagnostics` field and methods to `SemanticContext`
2. Update `SemanticContextBuilder` to initialize empty diagnostics
3. Change `semantic()` in `stages.rs` to take `&mut SemanticContext` and write diagnostics into it
4. Update each `rule_*.rs` to accept `&mut SemanticContext` and push diagnostics directly
5. Update `analyze()` to return `Ok(context)` after validation (with diagnostics in context)
6. Update `FileBackedProject::semantic()` to always cache context from `Ok`
7. Update tests throughout

## Testing

### Analyzer Tests

- Existing tests in `stages.rs` that check `assert!(res.is_err())` need updating. These should instead check `assert!(res.is_ok())` and then `assert!(!res.unwrap().diagnostics().is_empty())`.
- Each rule's tests that verify diagnostics need similar updates.

### LSP/Project Integration Tests

- Add a test: semantic analysis with errors still produces a cached `SemanticContext`.
- Add a test: document symbols are available even when the file has semantic errors.
- Existing tests that check `semantic().is_err()` should verify diagnostics are present in the context rather than in the `Err` variant.

### Behavioral Verification

- Verify the VS Code outline view stays populated when a file has semantic errors.
- Verify diagnostics (error squiggles) still appear correctly in the editor.
