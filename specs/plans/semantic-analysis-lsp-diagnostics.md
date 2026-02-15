# Plan: Preserve SemanticContext When Analysis Has Diagnostics

## Overview

Preserve the `SemanticContext` (types, functions, symbols) even when semantic analysis produces diagnostics. Currently, any error in type resolution or validation discards the entire context, breaking LSP features like document symbols, outline view, and future features (hover, go-to-definition). This change makes the analysis pipeline resilient to partial failures so the context is always available to the LSP.

## Current State

### What Exists

- **`analyze()` entry point**: `compiler/analyzer/src/stages.rs` — returns `Result<SemanticContext, Vec<Diagnostic>>`
- **Two-phase pipeline**: Phase 1 (`resolve_types`) builds environments; Phase 2 (`semantic`) runs 14 validation rules
- **`type_table::apply()`**: `compiler/analyzer/src/type_table.rs` — runs after `semantic()` in `analyze()`, also uses `?`
- **`SemanticContext`**: `compiler/analyzer/src/semantic_context.rs` — bundles `TypeEnvironment`, `FunctionEnvironment`, `SymbolEnvironment`
- **`SemanticResult` type alias**: `compiler/analyzer/src/result.rs` — `Result<(), Vec<Diagnostic>>`
- **`FileBackedProject`**: `compiler/plc2x/src/project.rs` — caches `SemanticContext` only on `Ok`
- **`LspProject`**: `compiler/plc2x/src/lsp_project.rs` — uses cached context for document symbols; converts diagnostics to LSP format

### What's Missing

- No way to return both a valid `SemanticContext` and diagnostics from `analyze()`
- `resolve_types()` aborts entirely when any transform fails — the context is never built
- `FileBackedProject` discards the context whenever any step fails
- LSP features (document symbols, outline) go blank when a file has any semantic error
- Future LSP features (hover, go-to-definition) will have the same problem

### Architecture

```
Source Files
       ↓
  Parser (per file)
       ↓
  Library (AST)
       ↓
  resolve_types()          ← Builds SemanticContext; aborts on any transform error
    ├─ xform_toposort_declarations
    ├─ xform_resolve_type_decl_environment
    ├─ xform_resolve_late_bound_expr_kind        ← Fold; consumes Library on error
    ├─ xform_resolve_late_bound_type_initializer ← Fold; consumes Library on error
    ├─ xform_resolve_symbol_and_function_environment
    └─ xform_resolve_type_aliases
       ↓
  semantic()               ← Runs 14 validation rules; returns Err on any finding
       ↓
  type_table::apply()      ← Builds TypeTable; uses ?
       ↓
  Result<SemanticContext, Vec<Diagnostic>>
       ↓
  FileBackedProject        ← Caches context only on Ok; discards on Err
       ↓
  LspProject               ← Uses cached context for IDE features
```

After this change:

```
Source Files
       ↓
  Parser (per file)
       ↓
  Library (AST)
       ↓
  resolve_types()          ← Builds SemanticContext; resilient to transform failures
    ├─ xform_toposort_declarations               ← Still hard failure (can't proceed)
    ├─ xform_resolve_type_decl_environment       ← Clone-and-recover on failure
    ├─ xform_resolve_late_bound_expr_kind        ← Clone-and-recover on failure
    ├─ xform_resolve_late_bound_type_initializer ← Clone-and-recover on failure
    ├─ xform_resolve_symbol_and_function_environment ← Clone-and-recover on failure
    └─ xform_resolve_type_aliases                ← Clone-and-recover on failure
       ↓
  semantic()               ← Runs 14 validation rules; unchanged signature
       ↓
  type_table::apply()      ← Errors captured into context
       ↓
  analyze()                ← Always returns Ok; diagnostics inside context
       ↓
  Ok(SemanticContext)      ← Always available; diagnostics inside
       ↓
  FileBackedProject        ← Always caches context; extracts diagnostics
       ↓
  LspProject               ← IDE features always work
```

### Design Rationale

**Why clone-and-recover for transforms**: The Fold-based transforms (`xform_resolve_type_decl_environment`, `xform_resolve_late_bound_expr_kind`, `xform_resolve_late_bound_type_initializer`) consume the Library on error — the original is gone. To continue the pipeline after a failure, we clone the Library before the transform. If the transform succeeds, we use its result; if it fails, we fall back to the clone and capture the diagnostics. This is simple, safe, and avoids modifying transform internals.

**Why keep only toposort as a hard failure**: `xform_toposort_declarations` establishes declaration ordering that all subsequent transforms depend on. Without it, later transforms would process declarations in the wrong order.

**Why xform_resolve_type_decl_environment is recoverable**: Although this transform populates the `TypeEnvironment`, it also performs validation (e.g., subrange bounds checking via P2002) that can fail. Making it recoverable means that a single invalid type declaration doesn't prevent the rest of the pipeline from producing useful results for the LSP. The type environment will be partially populated with whatever succeeded before the error.

**Why not change rule signatures**: The `semantic()` function already collects all diagnostics from all 14 validation rules into a single `Vec<Diagnostic>`. `analyze()` simply catches these and stores them in the context. No rule files need modification.

---

## Phase 1: Add Diagnostics to SemanticContext

**Goal**: `SemanticContext` can hold diagnostics alongside environment data.

**Status**: Complete.

### 1.1 Add Diagnostics Field

- [x] Add `diagnostics: Vec<Diagnostic>` field to `SemanticContext` in `compiler/analyzer/src/semantic_context.rs`
- [x] Add `add_diagnostics(&mut self, diagnostics: Vec<Diagnostic>)` method
- [x] Add `diagnostics(&self) -> &[Diagnostic]` accessor
- [x] Add `has_diagnostics(&self) -> bool` convenience method
- [x] Update `SemanticContext::new()` to initialize empty diagnostics vec
- [x] Update `SemanticContextBuilder::build()` — no change needed (uses `new()`)

### 1.2 Update SemanticContext Tests

- [x] Add test: `semantic_context_when_add_diagnostics_then_has_diagnostics`
- [x] Add test: `semantic_context_when_no_diagnostics_then_has_diagnostics_false`

**Phase 1 Milestone**: `SemanticContext` can store and return diagnostics.

---

## Phase 2: Update analyze() and resolve_types() to Capture Diagnostics

**Goal**: `analyze()` always returns `Ok(SemanticContext)` with diagnostics inside, except when the program cannot be parsed or sorted at all.

### 2.1 Make resolve_types() resilient in stages.rs

The transforms in `resolve_types()` fall into two categories:

**Hard failures** (abort the pipeline — no useful context is possible):
- `TypeEnvironmentBuilder::build()` — stdlib initialization; should never fail in practice
- `xform_toposort_declarations` — establishes declaration order; without it, subsequent transforms produce garbage

**Recoverable failures** (capture diagnostics, continue with pre-transform state):
- `xform_resolve_type_decl_environment` — Fold; consumes Library; clone before, fallback on error
- `xform_resolve_late_bound_expr_kind` — Fold; consumes Library; clone before, fallback on error
- `xform_resolve_late_bound_type_initializer` — Fold; consumes Library; clone before, fallback on error
- `xform_resolve_symbol_and_function_environment` — takes Library by value; clone before, fallback on error
- `xform_resolve_type_aliases` — takes Library by value; clone before, fallback on error

Implementation:
- [x] Accumulate diagnostics in `resolve_types()` and store them in the context before returning
- [x] Keep `?` for hard failures (toposort, TypeEnvironment build)
- [x] For all recoverable transforms: clone Library before calling, catch `Err`, capture diagnostics, fall back to clone

### 2.2 Update analyze() in stages.rs

- [x] Capture `Err` from `semantic()` into `context.add_diagnostics()`
- [x] Capture `Err` from `type_table::apply()` into `context.add_diagnostics()`
- [x] Return `Ok(context)` after all steps
- [x] Keep `Err` path only for hard failures in `resolve_types()` and empty sources

### 2.3 Update Analyzer Tests

- [x] Update `analyze_when_first_steps_semantic_error_then_result_is_err` → `analyze_when_first_steps_semantic_error_then_ok_with_diagnostics`
- [x] Update `rule_use_declared_enumerated_value` test to check `Ok` with diagnostics
- [x] Update `rule_decl_subrange_limits` test to check `Ok` with diagnostics
- [x] Verify all other existing passing tests remain green

**Phase 2 Status**: Complete.

---

## Phase 3: Update LSP Integration

**Goal**: The LSP always has access to `SemanticContext` when analysis returns `Ok`.

### 3.1 Update FileBackedProject

- [x] In `FileBackedProject::semantic()`, always cache context from `Ok(context)`
- [x] After caching, extract diagnostics from `context.diagnostics()` and append to `all_diagnostics`
- [x] Return `Err(all_diagnostics)` when diagnostics are present, `Ok(())` when clean
- [x] Preserve existing `Err` path for `analyze()` returning `Err` (hard failures)

### 3.2 Document Updated Project::semantic() Contract

The `Project::semantic()` return type remains `Result<(), Vec<Diagnostic>>`, but the contract changes:

- `Ok(())` — analysis succeeded with no diagnostics; context is cached
- `Err(diagnostics)` — either (a) hard failure and context is *not* cached, or (b) analysis completed with diagnostics, context *is* cached

This means callers should use `semantic_context()` to check whether the context is available rather than relying on the `Ok`/`Err` distinction.

- [x] Update doc comments on `Project::semantic()` to reflect the new contract
- [x] Update doc comment on `Project::semantic_context()` to clarify it may return `Some` even after `semantic()` returns `Err`

### 3.3 Verify LspProject

- [x] Verify `LspProject::semantic()` correctly reports diagnostics (no functional change — it already reads from the `Err` path)
- [x] Verify `LspProject::document_symbols()` works when file has semantic errors (returns symbols instead of empty)

### 3.4 Integration Tests

- [x] Add test: `semantic_when_validation_error_then_context_cached` — semantic analysis with errors still populates `semantic_context`
- [x] Add test: `document_symbols_when_semantic_errors_then_returns_symbols` — document symbols available even with semantic errors

**Phase 3 Status**: Complete.

---

## Phase 4: Verification

**Goal**: Confirm correct behavior end-to-end.

### 4.1 Run Full CI Pipeline

- [ ] Run `cd compiler && just` — all checks must pass
- [ ] Verify coverage meets 85% threshold
- [ ] Verify clippy passes with no warnings

### 4.2 Manual Verification

- [ ] Verify VS Code outline view stays populated when a file has semantic errors
- [ ] Verify diagnostics (error squiggles) still appear correctly in the editor
- [ ] Verify diagnostics clear when errors are fixed

**Phase 4 Milestone**: All tests pass, CI green, LSP behavior verified.

---

## Files to Modify

### Analyzer Crate (`compiler/analyzer/`)

| File | Action | Description |
|------|--------|-------------|
| `src/semantic_context.rs` | Done | Added `diagnostics` field and methods |
| `src/stages.rs` | Modify | Make `resolve_types()` resilient; update `analyze()` to capture all diagnostics |

### Project/LSP Crate (`compiler/plc2x/`)

| File | Action | Description |
|------|--------|-------------|
| `src/project.rs` | Modify | Always cache context from `Ok`; extract diagnostics from context; update doc comments |
| `src/lsp_project.rs` | Verify | No functional change expected |

### Files NOT Modified

| File | Reason |
|------|--------|
| `src/result.rs` | `SemanticResult` stays in use by all 14 rules and `semantic()` |
| `src/rule_*.rs` (14 files) | Rules continue to return `SemanticResult`; `analyze()` handles the conversion |
| `src/xform_*.rs` (6 files) | Transform internals unchanged; `resolve_types()` wraps them with clone-and-recover |

## Dependencies

- No new crate dependencies required
- No new problem codes required

## Scope Exclusions

The following are explicitly **out of scope** for this plan:

1. **Diagnostic severity levels** — All diagnostics are currently treated as errors. Adding warning/info severity is a separate concern.
2. **Incremental analysis** — Re-analyzing only changed files is out of scope.
3. **Changing rule signatures** — The 14 validation rules keep their current `fn apply(&Library, &SemanticContext) -> SemanticResult` signature.
4. **Changing transform internals** — The `xform_*` modules are not modified. Resilience is achieved by cloning before Fold-based transforms and catching errors at the `resolve_types()` level.

## When Does analyze() Return Err?

After this change, `Err` only occurs when:

1. **No sources**: `sources.is_empty()` — the caller provided nothing to analyze.
2. **Hard type resolution failure**:
   - `TypeEnvironmentBuilder::build()` fails (stdlib initialization error — should not happen in practice)
   - Circular type dependencies (`xform_toposort_declarations`)
   - Unresolvable type declarations (`xform_resolve_type_decl_environment`)

All other failures (late-bound resolution, symbol resolution, validation rules, type table) are captured as diagnostics inside the returned `SemanticContext`.

## Success Criteria

1. `analyze()` returns `Ok(context)` even when late-bound resolution or validation rules fail — diagnostics are in `context.diagnostics()`
2. `FileBackedProject` always caches the `SemanticContext` when declaration sorting and type environment building succeed
3. Document symbols are available in the LSP even when the file has semantic errors
4. Diagnostics (error squiggles) still appear correctly in the editor
5. All existing tests pass (with updated assertions)
6. CI pipeline passes (`cd compiler && just`)

## Summary

| Phase | Tasks | Files | Goal |
|-------|-------|-------|------|
| 1 | 8 | 1 | Add diagnostics to SemanticContext (done) |
| 2 | 10 | 1 | Make resolve_types() resilient; capture diagnostics in analyze() |
| 3 | 7 | 1-2 | LSP always has context |
| 4 | 6 | 0 | End-to-end verification |
| **Total** | **31** | **3-4** | Diagnostics in context, LSP always works |
