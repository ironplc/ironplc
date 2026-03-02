# Design: Expression Type Resolution

## Overview

Add resolved type information to every expression in the AST so that codegen can select correct opcodes without re-deriving types from raw strings, and the analyzer can validate type compatibility at expression level.

### Building On

- **[ADR-0013: Expression Type Annotation via Wrapper Struct](../adrs/0013-expression-type-annotation-via-wrapper-struct.md)** — the decision to use an `Expr` wrapper with `Option<TypeName>`
- **[ADR-0001: Bytecode Integer Arithmetic Type Strategy](../adrs/0001-bytecode-integer-arithmetic-type-strategy.md)** — the promote-operate-truncate model that codegen implements

## Problem

Codegen determines operation widths by walking expression trees to find variable references and string-matching their declared type names (`infer_op_type`, `infer_storage_bits` in compile.rs). This fails for type aliases because the string `"MyByte"` doesn't match the hardcoded list of elementary types. The analyzer already resolves aliases via `TypeEnvironment`, but this information is discarded before reaching codegen.

## Architecture

```
Source text
    |
    v
Parser ──> Library (AST with Expr { kind, resolved_type: None })
    |
    v
Analyzer
    ├── resolve_types() ──> TypeEnvironment
    ├── xform_resolve_late_bound() ──> resolves LateBound variants
    ├── xform_resolve_expr_types() ──> fills in resolved_type   <── NEW
    └── semantic validation rules
    |
    v
Library (AST with Expr { kind, resolved_type: Some(...) })
    |
    v
Codegen ──> reads expr.resolved_type ──> selects opcodes
```

## The Expr Struct

```rust
// compiler/dsl/src/textual.rs
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct Expr {
    pub kind: ExprKind,
    #[recurse(ignore)]
    pub resolved_type: Option<TypeName>,
}
```

- `resolved_type` is `#[recurse(ignore)]` because it is metadata, not a child node to visit/fold
- `Option` because the parser produces `None`; the Fold pass fills it in
- `TypeName` (not `IntermediateType`) to keep the DSL crate independent of analyzer types

### Constructor helpers

```rust
impl Expr {
    pub fn new(kind: ExprKind) -> Self {
        Self { kind, resolved_type: None }
    }

    pub fn with_type(kind: ExprKind, resolved_type: TypeName) -> Self {
        Self { kind, resolved_type: Some(resolved_type) }
    }
}
```

## Type Resolution Fold Pass

A new Fold pass in the analyzer (`xform_resolve_expr_types.rs`) walks the AST after late-bound resolution and fills in `resolved_type` for each expression.

### Resolution rules

| ExprKind variant | Resolution strategy |
|---|---|
| `Const(Integer(_))` | `ANY_INT` or inferred from context |
| `Const(RealLiteral(_))` | `REAL` or `LREAL` based on literal |
| `Const(BitStringLiteral(_))` | Type from the literal prefix (e.g., `BYTE#...` → `BYTE`) |
| `Const(Boolean(_))` | `BOOL` |
| `Variable(v)` | Look up variable's declared type in scope, resolve alias via `TypeEnvironment` |
| `BinaryOp(op)` | Result type from operand types (both operands should have same type after resolution) |
| `UnaryOp(op)` | Same type as operand |
| `Compare(cmp)` | `BOOL` (comparisons always produce BOOL) |
| `Function(f)` | Return type from `FunctionEnvironment` signature, specialized to match argument type |
| `EnumeratedValue(e)` | The enumeration type name |
| `LateBound(_)` | Should not exist after late-bound resolution; error if encountered |

### Alias resolution

When a variable is declared as `x : MyByte` where `TYPE MyByte : BYTE; END_TYPE`:

1. The Fold pass looks up `x` in the symbol scope to get declared type `MyByte`
2. It queries `TypeEnvironment` to resolve `MyByte` → `BYTE`
3. It sets `resolved_type = Some(TypeName::from("BYTE"))`

Codegen then sees `"BYTE"` and correctly maps it to 8-bit width.

## Codegen Changes (PR 4)

After the Fold pass populates `resolved_type`, codegen can read it directly:

```rust
// Before (infer_op_type walks expression trees):
let op_type = infer_op_type(ctx, expr);

// After (read resolved type directly):
let op_type = match &expr.resolved_type {
    Some(type_name) => resolve_type_name(type_name),
    None => DEFAULT_OP_TYPE,
};
```

This eliminates `infer_op_type`, `infer_storage_bits`, and the redundant `resolve_type_name` string matching for aliased types.

## Implementation Plan

### PR 2: Introduce Expr wrapper

Mechanical migration — no behavior change.

**Files changed:**

| File | Change |
|---|---|
| `compiler/dsl/src/textual.rs` | Add `Expr` struct, update `ExprKind` usage in other structs (e.g., `BinaryExpr.left`/`.right` become `Expr`) |
| `compiler/dsl/src/fold.rs` | Add `fold_expr` method, update `fold_expr_kind` callers |
| `compiler/dsl/src/visitor.rs` | Add `visit_expr` method |
| `compiler/parser/src/parser.rs` | Wrap every `ExprKind` construction in `Expr::new(...)` |
| `compiler/parser/src/tests.rs` | Update test assertions |
| `compiler/codegen/src/compile.rs` | Access `.kind` when matching, pass `Expr` through |
| `compiler/analyzer/src/xform_resolve_late_bound_expr_kind.rs` | Update Fold to handle `Expr` wrapper |
| `compiler/plc2plc/src/renderer.rs` | Access `.kind` when rendering |
| `compiler/sources/src/xml/transform.rs` | Wrap constructions in `Expr::new(...)` |
| `compiler/dsl/src/sfc.rs` | Update `ExprKind` references |

### PR 3: Type resolution Fold pass

**New file:** `compiler/analyzer/src/xform_resolve_expr_types.rs`

- Implements `Fold` trait
- Maintains scope context (current POU's variable declarations)
- Receives `&TypeEnvironment` for alias resolution
- Called from `stages.rs` after late-bound resolution, before semantic validation

**Modified:** `compiler/analyzer/src/stages.rs` — add the new Fold pass to the pipeline

### PR 4: Update codegen

**Modified:** `compiler/codegen/src/compile.rs`

- Replace `infer_op_type` calls with `expr.resolved_type` reads
- Replace `infer_storage_bits` calls with `expr.resolved_type` reads
- Remove `infer_op_type` and `infer_storage_bits` functions
- Simplify `resolve_type_name` since aliases are pre-resolved

## Testing

### PR 2 tests

All existing tests pass unchanged (behavior is identical; `resolved_type` is `None` everywhere).

### PR 3 tests

- Unit tests in `xform_resolve_expr_types.rs`:
  - Variable reference resolves to declared type
  - Type alias resolves to base type
  - Binary operation inherits operand type
  - Comparison resolves to BOOL
  - Function call resolves to return type
  - Nested expressions resolve correctly

### PR 4 tests

- Existing codegen end-to-end tests pass (same opcodes produced)
- New test: program with type alias produces correct opcodes
