# Plan: Preserve SemanticContext When Analysis Has Diagnostics

## Overview

Preserve the `SemanticContext` (types, functions, symbols) even when semantic validation rules produce diagnostics. Currently, any validation error discards the entire context, breaking LSP features like document symbols, outline view, and future features (hover, go-to-definition). This change moves diagnostics into the `SemanticContext` so the context is always available to the LSP when type resolution succeeds.

## Current State

### What Exists

- **`analyze()` entry point**: `compiler/analyzer/src/stages.rs` — returns `Result<SemanticContext, Vec<Diagnostic>>`
- **Two-phase pipeline**: Phase 1 (`resolve_types`) builds environments; Phase 2 (`semantic`) runs 14 validation rules
- **`SemanticContext`**: `compiler/analyzer/src/semantic_context.rs` — bundles `TypeEnvironment`, `FunctionEnvironment`, `SymbolEnvironment`
- **`SemanticResult` type alias**: `compiler/analyzer/src/result.rs` — `Result<(), Vec<Diagnostic>>`
- **`FileBackedProject`**: `compiler/plc2x/src/project.rs` — caches `SemanticContext` only on `Ok`
- **`LspProject`**: `compiler/plc2x/src/lsp_project.rs` — uses cached context for document symbols; converts diagnostics to LSP format

### What's Missing

- No way to return both a valid `SemanticContext` and diagnostics from `analyze()`
- `FileBackedProject` discards the context whenever any validation rule fails
- LSP features (document symbols, outline) go blank when a file has semantic errors
- Future LSP features (hover, go-to-definition) will have the same problem

### Architecture

```
Source Files
       ↓
  Parser (per file)
       ↓
  Library (AST)
       ↓
  resolve_types()          ← Builds SemanticContext (types, functions, symbols)
       ↓
  semantic()               ← Runs 14 validation rules; currently returns Err on any finding
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
  resolve_types()          ← Builds SemanticContext (types, functions, symbols)
       ↓
  semantic()               ← Runs 14 validation rules; writes diagnostics into context
       ↓
  Result<SemanticContext, Vec<Diagnostic>>   ← Ok now includes diagnostics in context
       ↓
  FileBackedProject        ← Always caches context from Ok (even with diagnostics)
       ↓
  LspProject               ← IDE features always work when context is available
```

---

## Phase 1: Add Diagnostics to SemanticContext

**Goal**: `SemanticContext` can hold diagnostics alongside environment data.

### 1.1 Add Diagnostics Field

- [ ] Add `diagnostics: Vec<Diagnostic>` field to `SemanticContext` in `compiler/analyzer/src/semantic_context.rs`
- [ ] Add `add_diagnostic(&mut self, diagnostic: Diagnostic)` method
- [ ] Add `add_diagnostics(&mut self, diagnostics: Vec<Diagnostic>)` method
- [ ] Add `diagnostics(&self) -> &[Diagnostic]` accessor
- [ ] Add `has_diagnostics(&self) -> bool` convenience method
- [ ] Update `SemanticContext::new()` to initialize empty diagnostics vec
- [ ] Update `SemanticContextBuilder::build()` to initialize empty diagnostics vec

### 1.2 Update SemanticContext Tests

- [ ] Add test: `semantic_context_when_add_diagnostic_then_has_diagnostics`
- [ ] Add test: `semantic_context_when_no_diagnostics_then_has_diagnostics_false`
- [ ] Add test: `semantic_context_when_add_diagnostics_then_all_present`

**Phase 1 Milestone**: `SemanticContext` can store and return diagnostics.

---

## Phase 2: Change Validation Rules to Write Into Context

**Goal**: Validation rules write diagnostics into `SemanticContext` instead of returning them.

### 2.1 Update semantic() in stages.rs

- [ ] Change `semantic()` signature from `fn semantic(library: &Library, context: &SemanticContext) -> SemanticResult` to `fn semantic(library: &Library, context: &mut SemanticContext)`
- [ ] Update the rule dispatch loop to pass `&mut context` and collect diagnostics into context
- [ ] Remove `SemanticResult` return from `semantic()`

### 2.2 Update Each Validation Rule

Each `rule_*.rs` module changes from returning `SemanticResult` to writing diagnostics into the context:

- [ ] `rule_decl_struct_element_unique_names.rs` — change `apply` to accept `&mut SemanticContext`, add diagnostics to context
- [ ] `rule_decl_subrange_limits.rs` — change `apply` to accept `&mut SemanticContext`, add diagnostics to context
- [ ] `rule_enumeration_values_unique.rs` — change `apply` to accept `&mut SemanticContext`, add diagnostics to context
- [ ] `rule_function_block_invocation.rs` — change `apply` to accept `&mut SemanticContext`, add diagnostics to context
- [ ] `rule_function_call_declared.rs` — change `apply` to accept `&mut SemanticContext`, add diagnostics to context
- [ ] `rule_program_task_definition_exists.rs` — change `apply` to accept `&mut SemanticContext`, add diagnostics to context
- [ ] `rule_stdlib_type_redefinition.rs` — change `apply` to accept `&mut SemanticContext`, add diagnostics to context
- [ ] `rule_use_declared_enumerated_value.rs` — change `apply` to accept `&mut SemanticContext`, add diagnostics to context
- [ ] `rule_use_declared_symbolic_var.rs` — change `apply` to accept `&mut SemanticContext`, add diagnostics to context
- [ ] `rule_unsupported_stdlib_type.rs` — change `apply` to accept `&mut SemanticContext`, add diagnostics to context
- [ ] `rule_var_decl_const_initialized.rs` — change `apply` to accept `&mut SemanticContext`, add diagnostics to context
- [ ] `rule_var_decl_const_not_fb.rs` — change `apply` to accept `&mut SemanticContext`, add diagnostics to context
- [ ] `rule_var_decl_global_const_requires_external_const.rs` — change `apply` to accept `&mut SemanticContext`, add diagnostics to context
- [ ] `rule_pou_hierarchy.rs` — change `apply` to accept `&mut SemanticContext`, add diagnostics to context

### 2.3 Remove or Repurpose SemanticResult

- [ ] Evaluate whether `SemanticResult` type alias in `result.rs` is still needed
- [ ] Remove if unused, or repurpose for internal use

### 2.4 Update analyze()

- [ ] Change `analyze()` to call `semantic(&library, &mut context)` without `?`
- [ ] Return `Ok(context)` after validation (context now carries diagnostics)
- [ ] Keep `Err` path only for `resolve_types()` failure and empty sources

### 2.5 Update Analyzer Tests

- [ ] Update `stages.rs` tests: `analyze_when_first_steps_semantic_error_then_result_is_err` → check `Ok` with diagnostics
- [ ] Update each `rule_*.rs` test module: check diagnostics on context instead of `Err` return
- [ ] Verify existing passing tests remain green

**Phase 2 Milestone**: Validation diagnostics flow through `SemanticContext` instead of `Result::Err`.

---

## Phase 3: Update LSP Integration

**Goal**: The LSP always has access to `SemanticContext` when type resolution succeeds.

### 3.1 Update FileBackedProject

- [ ] Change `FileBackedProject::semantic()` to always cache context from `Ok(context)`
- [ ] Extract diagnostics from `context.diagnostics()` and include in the returned `Err` when diagnostics are present
- [ ] Preserve existing `Err` path for `analyze()` returning `Err` (type resolution failure)

### 3.2 Update Project Trait (if needed)

- [ ] Evaluate whether `Project::semantic()` return type needs to change
- [ ] If unchanged, verify the contract still makes sense (diagnostics in `Err`, context always cached on successful resolution)

### 3.3 Verify LspProject

- [ ] Verify `LspProject::semantic()` correctly reports diagnostics (no functional change expected)
- [ ] Verify `LspProject::document_symbols()` works when file has semantic errors (now returns symbols instead of empty)

### 3.4 Add Integration Tests

- [ ] Add test: `semantic_when_validation_error_then_context_cached` — semantic analysis with errors still populates `semantic_context`
- [ ] Add test: `document_symbols_when_semantic_errors_then_returns_symbols` — document symbols available even with semantic errors
- [ ] Update test: `analyze_when_not_valid_then_err` in `project.rs` — verify context is cached despite diagnostics

**Phase 3 Milestone**: LSP features work even when the file has semantic validation errors.

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
| `src/semantic_context.rs` | Modify | Add `diagnostics` field and methods |
| `src/result.rs` | Modify/Remove | Remove or repurpose `SemanticResult` type alias |
| `src/stages.rs` | Modify | Change `semantic()` and `analyze()` to use context for diagnostics |
| `src/rule_decl_struct_element_unique_names.rs` | Modify | Write diagnostics to context |
| `src/rule_decl_subrange_limits.rs` | Modify | Write diagnostics to context |
| `src/rule_enumeration_values_unique.rs` | Modify | Write diagnostics to context |
| `src/rule_function_block_invocation.rs` | Modify | Write diagnostics to context |
| `src/rule_function_call_declared.rs` | Modify | Write diagnostics to context |
| `src/rule_program_task_definition_exists.rs` | Modify | Write diagnostics to context |
| `src/rule_stdlib_type_redefinition.rs` | Modify | Write diagnostics to context |
| `src/rule_use_declared_enumerated_value.rs` | Modify | Write diagnostics to context |
| `src/rule_use_declared_symbolic_var.rs` | Modify | Write diagnostics to context |
| `src/rule_unsupported_stdlib_type.rs` | Modify | Write diagnostics to context |
| `src/rule_var_decl_const_initialized.rs` | Modify | Write diagnostics to context |
| `src/rule_var_decl_const_not_fb.rs` | Modify | Write diagnostics to context |
| `src/rule_var_decl_global_const_requires_external_const.rs` | Modify | Write diagnostics to context |
| `src/rule_pou_hierarchy.rs` | Modify | Write diagnostics to context |

### Project/LSP Crate (`compiler/plc2x/`)

| File | Action | Description |
|------|--------|-------------|
| `src/project.rs` | Modify | Always cache context from `Ok`; extract diagnostics from context |
| `src/lsp_project.rs` | Verify | No functional change expected |

## Dependencies

- No new crate dependencies required
- No new problem codes required

## Scope Exclusions

The following are explicitly **out of scope** for this plan:

1. **Making `resolve_types()` resilient** — When type resolution fails, the context is still discarded. Making `resolve_types()` accumulate errors and return a partial context is a separate, larger effort.
2. **Diagnostic severity levels** — All diagnostics are currently treated as errors. Adding warning/info severity is a separate concern.
3. **Incremental analysis** — Re-analyzing only changed files is out of scope.

## When Does analyze() Return Err?

After this change, `Err` only occurs when:

1. **No sources**: `sources.is_empty()` — the caller provided nothing to analyze.
2. **Type resolution failure**: One of the `xform_*` transforms in `resolve_types()` fails:
   - Circular type dependencies (`xform_toposort_declarations`)
   - Unresolvable type declarations (`xform_resolve_type_decl_environment`)
   - Unresolvable late-bound expressions (`xform_resolve_late_bound_expr_kind`)
   - Unresolvable type initializers (`xform_resolve_late_bound_type_initializer`)
   - Unresolvable symbols/functions (`xform_resolve_symbol_and_function_environment`)
   - Unresolvable type aliases (`xform_resolve_type_aliases`)

These are cases where the program structure is fundamentally broken and the environments cannot be reliably built.

## Success Criteria

1. `analyze()` returns `Ok(context)` even when validation rules find errors — diagnostics are in `context.diagnostics()`
2. `FileBackedProject` always caches the `SemanticContext` when type resolution succeeds
3. Document symbols are available in the LSP even when the file has semantic errors
4. Diagnostics (error squiggles) still appear correctly in the editor
5. All existing tests pass (with updated assertions)
6. CI pipeline passes (`cd compiler && just`)

## Summary

| Phase | Tasks | Goal |
|-------|-------|------|
| 1 | 10 | Add diagnostics to SemanticContext |
| 2 | 20 | Validation rules write into context |
| 3 | 8 | LSP always has context |
| 4 | 3 | End-to-end verification |
| **Total** | **41** | Diagnostics in context, LSP always works |
