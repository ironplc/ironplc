# Expression Type Annotation via Wrapper Struct

status: proposed
date: 2026-03-02

## Context and Problem Statement

The codegen phase needs to know the resolved type of each expression to select the correct operation width (i32 vs i64) and the correct builtin function variant (e.g., ROL_U8 vs ROL_I32 for narrow rotate). Currently, the AST (`ExprKind`) carries no type information. Codegen re-derives types by walking expression trees to find variable references and string-matching their declared type names against a hardcoded list of 15 elementary types (`resolve_type_name()` in compile.rs).

This approach fails for type aliases. If a user writes `TYPE MyByte : BYTE; END_TYPE` and declares `x : MyByte`, codegen sees the type name `"MyByte"` and cannot resolve it to `BYTE` because it has no access to the `TypeEnvironment` that the analyzer builds. The analyzer already resolves aliases, but this information never reaches codegen — `compile()` receives only `&Library` (the raw AST), not `SemanticContext`.

How should the compiler attach resolved type information to expressions?

## Decision Drivers

* **Type alias support** — IEC 61131-3 allows user-defined type aliases; codegen must resolve them to their base types to select correct opcodes
* **Separation of concerns** — the DSL crate (AST definitions) should not depend on analyzer internals (`IntermediateType`, `TypeEnvironment`)
* **Expression-level validation** — the analyzer needs expression types to validate type compatibility (e.g., rejecting `SHL` on a non-bit-string type)
* **Existing infrastructure** — the analyzer already has `TypeEnvironment` for alias resolution and the `Fold` trait for AST transformations
* **Minimal churn** — `ExprKind` is referenced 190 times across 11 files; the solution should keep the migration manageable

## Considered Options

* Wrapper struct: `Expr { kind: ExprKind, resolved_type: Option<TypeName> }`
* External side table: `HashMap<SourceSpan, ResolvedType>` passed alongside the AST
* Annotate only variable declarations (`VarDecl.resolved_type`)

## Decision Outcome

Chosen option: "Wrapper struct", because it co-locates type information with the expression it describes, enables both codegen and validation to read types directly, and uses the existing `Fold` mechanism to populate the annotation.

The new struct:

```rust
// In compiler/dsl/src/textual.rs
pub struct Expr {
    pub kind: ExprKind,
    pub resolved_type: Option<TypeName>,
}
```

The type uses `TypeName` (a string-based type reference already defined in the DSL crate), not `IntermediateType` (defined in the analyzer crate). This preserves the separation of concerns: the DSL crate defines the data structure, the analyzer crate populates it.

### Data flow

1. **Parser** produces `Expr { kind: ..., resolved_type: None }` for every expression
2. **Analyzer** runs a `Fold` pass that walks the AST, resolves type aliases via `TypeEnvironment`, and rebuilds each `Expr` with `resolved_type: Some(base_type_name)`
3. **Codegen** reads `expr.resolved_type` directly to select operation widths and builtin variants

### Consequences

* Good, because type information is always co-located with the expression — no lookup indirection
* Good, because the `Fold` pass uses existing infrastructure (`TypeEnvironment`, `Fold` trait) with no new mechanisms
* Good, because `TypeName` keeps the DSL crate independent of analyzer types
* Good, because expression-level type annotations enable future validation rules (type checking operators, function arguments)
* Good, because codegen's `infer_op_type` / `infer_storage_bits` workarounds can be eliminated in favor of reading `resolved_type`
* Bad, because introducing the `Expr` wrapper requires updating ~190 references across 11 files — this is a mechanical but large change
* Neutral, because `Option<TypeName>` adds one pointer-sized field per expression node; PLC programs are small enough that this is negligible

### Confirmation

Verify by:
1. Compiling a program with a type alias (`TYPE MyByte : BYTE; END_TYPE; VAR x : MyByte; END_VAR`) and checking that `resolved_type` is `Some(TypeName("BYTE"))` after the Fold pass
2. Confirming codegen produces correct opcodes for aliased types without using `infer_op_type`
3. All existing tests continue to pass after the `Expr` wrapper migration

## Pros and Cons of the Options

### Wrapper Struct (chosen)

A new `Expr` struct wraps `ExprKind` with an optional `resolved_type` field. A `Fold` pass populates it.

* Good, because type info travels with the expression — available to every consumer without lookup
* Good, because the `Fold` pattern already exists for AST transformations (e.g., `xform_resolve_late_bound_expr_kind`)
* Good, because `TypeName` is already in the DSL crate — no new dependencies
* Bad, because ~190 references to `ExprKind` across 11 files must be updated to use `Expr`

### External Side Table

A `HashMap<SourceSpan, ResolvedType>` is built during analysis and passed to codegen alongside `&Library`.

* Good, because zero AST changes are needed
* Bad, because `SourceSpan` is not unique per expression — compiler-generated nodes may share spans or use `SourceSpan::default()`, causing collisions
* Bad, because the map must be kept in sync if any pass transforms the AST (Fold passes may change spans)
* Bad, because every type lookup requires a hash lookup instead of reading a field

### Annotate Only VarDecl

Add `resolved_type: Option<TypeName>` to `VarDecl` only.

* Good, because the change surface is small — one struct
* Good, because it solves the type alias problem for variable declarations
* Bad, because it does not provide expression-level types — codegen still needs `infer_op_type` to propagate types through binary/unary operators
* Bad, because it does not enable expression-level validation (e.g., checking that `SHL` operands are bit-string types)

## More Information

### Implementation plan

The migration is split into four PRs to keep each change reviewable:

| PR | Scope |
|----|-------|
| 1 | ADR and design document (this document) |
| 2 | Introduce `Expr` wrapper struct, update all call sites (mechanical, `resolved_type: None` everywhere) |
| 3 | Add Fold pass in analyzer that populates `resolved_type` using `TypeEnvironment` |
| 4 | Update codegen to read `resolved_type` and remove `infer_op_type` / `infer_storage_bits` |

### Interaction with existing Fold passes

The late-bound expression resolution pass (`xform_resolve_late_bound_expr_kind`) already transforms `ExprKind` variants using Fold. The new type resolution Fold pass runs after late-bound resolution (which must complete first so all expressions have their final `ExprKind` variant). The type resolution pass does not change the `kind` — it only fills in `resolved_type`.

### Why not `IntermediateType`?

`IntermediateType` is defined in the analyzer crate and represents fully resolved types (including structure layouts, array dimensions, etc.). Storing it on AST nodes would require the DSL crate to depend on the analyzer crate, creating a circular dependency. `TypeName` is a lightweight string-based reference already defined in the DSL crate. Codegen maps `TypeName` to operation widths using its existing `resolve_type_name()` function — but with aliases resolved, the string matching works correctly.
