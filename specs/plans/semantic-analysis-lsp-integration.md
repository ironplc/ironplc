# Fix Semantic Analysis / LSP Integration

This document specifies the changes needed to fix the integration between
semantic analysis and the LSP so that useful information (symbols, types,
functions) is available to IDE features even when semantic analysis produces
diagnostics.

## Problem Statement

Currently, `analyze()` returns `Result<SemanticContext, Vec<Diagnostic>>`.
The LSP integration in `FileBackedProject::semantic()` caches the
`SemanticContext` only on `Ok`:

```rust
// project.rs:140-150
match analyze(&all_libraries) {
    Ok(context) => {
        self.semantic_context = Some(context);
        Ok(())
    }
    Err(diagnostics) => {
        // Context is NOT cached — IDE features stop working
        all_diagnostics.extend(diagnostics);
        Err(all_diagnostics)
    }
}
```

When analysis returns `Err`, `semantic_context` is `None`, which causes
every feature that depends on the context to degrade:

- `document_symbols()` returns an empty list (`lsp_project.rs:127-129`)
- Any future go-to-definition, hover, or find-references would also fail

This is an "all or nothing" behavior: a single semantic error anywhere in
the project disables all IDE support for every file.

### Why the context is still useful on error

The `analyze()` pipeline has two phases:

1. **`resolve_types()`** — Builds the `SemanticContext` by running
   transformations that populate the type, function, and symbol
   environments.
2. **`semantic()`** — Runs validation rules against the fully-built context.

When a **validation rule** in phase 2 finds an error (e.g., undeclared
variable, type mismatch), the `SemanticContext` is already complete. The
diagnostics describe problems in the user's code, not problems with the
context itself. There is no reason to discard the context.

Even when a **transformation** in phase 1 fails partway through, the
environments may be partially populated with useful information (e.g., some
types resolved successfully).

## Proposed Design

### Core change: diagnostics move into `SemanticContext`

Add a `diagnostics` field to `SemanticContext`:

```rust
pub struct SemanticContext {
    pub types: TypeEnvironment,
    pub functions: FunctionEnvironment,
    pub symbols: SymbolEnvironment,
    pub diagnostics: Vec<Diagnostic>,  // NEW
}
```

### Change `analyze()` return type

Change the signature of `analyze()` from:

```rust
pub fn analyze(sources: &[&Library]) -> Result<SemanticContext, Vec<Diagnostic>>
```

to:

```rust
pub fn analyze(sources: &[&Library]) -> Result<SemanticContext, Vec<Diagnostic>>
```

The signature stays the same, but the semantics change:

- **`Ok(context)`** — Analysis completed. `context.diagnostics` may be
  non-empty (the user's code may have errors). The context is valid and
  useful for IDE features.
- **`Err(diagnostics)`** — Analysis failed completely. This should only
  occur in scenarios where no useful context can be constructed at all.

In practice, the `Err` case should be rare. The one existing case is when
`sources` is empty (the `NoContent` check at the top of `analyze()`). It is
not clear that there are other scenarios that require this, but the
`Result` is kept as a safety valve.

### Change `semantic()` to accumulate rather than fail

Currently the internal `semantic()` function returns
`SemanticResult` (i.e., `Result<(), Vec<Diagnostic>>`). Change it to return
`Vec<Diagnostic>` directly:

```rust
pub(crate) fn semantic(library: &Library, context: &SemanticContext) -> Vec<Diagnostic> {
    let mut all_diagnostics = vec![];
    for func in functions {
        match func(library, context) {
            Ok(_) => {}
            Err(diagnostics) => {
                all_diagnostics.extend(diagnostics);
            }
        }
    }
    all_diagnostics
}
```

The individual rules can keep their existing `SemanticResult` return type
since they are self-contained checks. Only the orchestrating function
changes.

### Change `resolve_types()` to return partial results on failure

Currently `resolve_types()` uses `?` to short-circuit on the first
transformation error:

```rust
let mut library = xform_toposort_declarations::apply(library)?;
// ...
library = xform(library, &mut type_environment)?
// ...
library = xform_resolve_symbol_and_function_environment::apply(...)?;
```

Change this to accumulate diagnostics and return a partial context. The
environments are populated incrementally — even if a later transformation
fails, earlier transformations may have already added useful entries to
`type_environment`, `symbol_environment`, and `function_environment`.

When a transformation fails:

1. Accumulate the diagnostics.
2. Stop running subsequent transformations (they likely depend on the failed
   one).
3. Build a `SemanticContext` from whatever environments exist at that point.
4. Return `Ok(context)` with the accumulated diagnostics inside
   `context.diagnostics`.

The only transformation failure that cannot return a partial context is
`TypeEnvironmentBuilder::build()` for stdlib types, which is an internal
error that should not occur in practice.

### Change `type_table::apply()` similarly

The `type_table::apply()` call in `analyze()` also uses `?`:

```rust
let type_table_result = type_table::apply(&library)?;
```

This should follow the same pattern: accumulate diagnostics into the
context rather than short-circuiting.

### Update `Project::semantic()` return type

Change from:

```rust
fn semantic(&mut self) -> Result<(), Vec<Diagnostic>>;
```

to:

```rust
fn semantic(&mut self) -> Vec<Diagnostic>;
```

The implementation always caches the context (when one is returned) and
always returns diagnostics (which may be empty):

```rust
fn semantic(&mut self) -> Vec<Diagnostic> {
    self.semantic_context = None;

    let mut all_diagnostics: Vec<Diagnostic> = vec![];

    // ... parse sources, accumulate parse errors ...

    match analyze(&all_libraries) {
        Ok(context) => {
            all_diagnostics.extend(context.diagnostics.clone());
            self.semantic_context = Some(context);
        }
        Err(diagnostics) => {
            all_diagnostics.extend(diagnostics);
        }
    }

    all_diagnostics
}
```

### Update `LspProject::semantic()`

Simplify from matching on `Result` to using the returned `Vec` directly:

```rust
pub(crate) fn semantic(&mut self, uri: &Uri) -> Vec<lsp_types::Diagnostic> {
    let path = to_path_buf(uri);
    if let Ok(path) = path {
        let file_id = FileId::from_path(&path);
        let diagnostics = self.wrapped.semantic();

        return diagnostics
            .into_iter()
            .filter(|d| d.file_ids().contains(&file_id))
            .map(|d| map_diagnostic(d, self.wrapped.as_ref()))
            .collect();
    }
    // ...
}
```

## What changes and what does not

### Changes

| Component | Current | Proposed |
|---|---|---|
| `SemanticContext` | No diagnostics field | Has `diagnostics: Vec<Diagnostic>` |
| `analyze()` | Returns `Err` on any semantic error | Returns `Ok` with diagnostics in context; `Err` only for complete failure |
| `semantic()` (internal) | Returns `Result<(), Vec<Diagnostic>>` | Returns `Vec<Diagnostic>` |
| `resolve_types()` | Short-circuits on first transform error | Accumulates diagnostics, returns partial context |
| `Project::semantic()` | Returns `Result<(), Vec<Diagnostic>>` | Returns `Vec<Diagnostic>` |
| `FileBackedProject` | Caches context only on `Ok` | Always caches context when `analyze()` returns `Ok` |
| `LspProject::semantic()` | Matches on `Result` | Uses `Vec<Diagnostic>` directly |

### Does NOT change

- Individual rule functions (`rule_*.rs`) — they keep their
  `SemanticResult` return type.
- Individual transformation functions (`xform_*.rs`) — their signatures
  stay the same.
- The `Diagnostic` type itself.
- The LSP notification flow (still calls `semantic()` on document change,
  still publishes diagnostics).
- `SemanticContextBuilder` — it does not deal with diagnostics.

## Implementation order

### Phase 1: Add diagnostics to SemanticContext

1. Add `diagnostics: Vec<Diagnostic>` to `SemanticContext`.
2. Update `SemanticContext::new()` to accept diagnostics (or default to
   empty).
3. Update `SemanticContextBuilder::build()` to produce an empty diagnostics
   vec.
4. Update existing tests.

### Phase 2: Change `semantic()` orchestration in `stages.rs`

1. Change `semantic()` to return `Vec<Diagnostic>` instead of
   `SemanticResult`.
2. In `analyze()`, append the result of `semantic()` to
   `context.diagnostics` instead of using `?`.
3. Do the same for `type_table::apply()`.
4. Update tests in `stages.rs`.

### Phase 3: Change `resolve_types()` to return partial context

1. Change `resolve_types()` to catch transformation errors and accumulate
   diagnostics.
2. When a transformation fails, stop subsequent transformations but still
   build a `SemanticContext` from the partially-populated environments.
3. Return `Ok((library_or_none, context))` or restructure the return type
   as needed.
4. Update `analyze()` to handle the case where the library is not available
   (skip `semantic()` rules and `type_table` if there is no library).
5. Update tests.

### Phase 4: Update `Project` trait and implementations

1. Change `Project::semantic()` to return `Vec<Diagnostic>`.
2. Update `FileBackedProject::semantic()` to always cache the context.
3. Update `LspProject::semantic()` to use the new return type.
4. Update tests in `project.rs` and `lsp_project.rs`.

### Phase 5: Verify end-to-end

1. Run the full CI pipeline (`cd compiler && just`).
2. Verify that document symbols work when there are semantic errors.
3. Verify that diagnostics are still correctly reported and filtered by
   file.

## Design decisions and trade-offs

### Why keep `Result` at all?

The `Err` variant serves as a safety valve for truly unrecoverable
situations (no sources, internal errors). Removing it entirely would mean
`analyze()` always returns a `SemanticContext`, which would require
constructing a default/empty context even in degenerate cases. Keeping
`Result` is marginally cleaner for those edge cases.

If no additional `Err` scenarios emerge during implementation, a future
change could simplify `analyze()` to always return `SemanticContext`.

### Why not put diagnostics alongside the context (e.g., return a tuple)?

Putting diagnostics inside `SemanticContext` is simpler than returning
`(SemanticContext, Vec<Diagnostic>)` because:

- The context is passed around and cached as a single value.
- Consumers don't need to track diagnostics separately.
- It is natural for the context to "know" what issues were found during its
  construction.

### Why stop transformations after a failure instead of continuing?

The transformations in `resolve_types()` are sequential and depend on each
other. For example, `xform_resolve_late_bound_expr_kind` needs types
resolved by `xform_resolve_type_decl_environment`. Running later
transformations after an earlier one fails would likely produce cascading
errors that are confusing rather than helpful.

Stopping early and returning a partial context provides the best trade-off:
useful partial information without misleading diagnostics.

### What about cloning diagnostics out of the context?

In the `Project::semantic()` implementation, diagnostics need to be
returned to the caller, but the context is also cached. This requires
either cloning the diagnostics or draining them from the context. Cloning
is straightforward; draining is an optimization that could be done later if
diagnostics become large.

## Files affected

| File | Change |
|---|---|
| `compiler/analyzer/src/semantic_context.rs` | Add `diagnostics` field |
| `compiler/analyzer/src/stages.rs` | Change `analyze()`, `semantic()`, `resolve_types()` |
| `compiler/analyzer/src/result.rs` | May become unused or simplified |
| `compiler/plc2x/src/project.rs` | Change `Project::semantic()` signature and implementation |
| `compiler/plc2x/src/lsp_project.rs` | Simplify `LspProject::semantic()` |
| `compiler/plc2x/src/lsp.rs` | Minor — adapts to new return type from `semantic()` |
