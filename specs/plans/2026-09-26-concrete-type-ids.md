# Plan: Concrete Type IDs for Expressions

## Goal

Every expression that denotes a value of a concrete type carries a precise,
numeric `TypeId` for that type — including types that have no name — so the
analyzer can reject a whole array, structure or enumeration used where a scalar
is expected. Fixes #1761.

## Context

`Expr.resolved_type` is an `Option<TypeName>`. A type is known only if it can
be *spelled*, and three consequences follow:

1. **Anonymous types have no type.** `a : ARRAY[1..2] OF DINT` and
   `e : (X, Y)` never get a `resolved_type`, so `rule_function_call_type_check`
   skips `LEN(a)`, `ABS(a)` and `ABS(e)` and codegen later answers with the
   first element (#1761) or reports P9998.
2. **Named arrays lose their name.** Late-bound resolution replaces
   `na : Arr` with a copy of `Arr`'s specification
   (`xform_resolve_late_bound_type_initializer.rs`), so `ABS(na)`, `F(na)` and
   `LEN(na)` are accepted too.
3. **Names are flattened to compare them.** `xform_resolve_expr_types` rewrites
   `MyByte` to `BYTE` so that string comparison works, and codegen
   re-derives everything from strings: `resolve_type_name`,
   `resolve_iec_type_tag` (`compile_setup.rs`) and the `op_type` fallback
   that treats *any* unknown name as an enumeration (so an array name reaching
   codegen would silently become a DINT).

Separately, `check_assignment_type` only checks when both sides are
elementary, so `n := a`, `n := na`, `n := r` (structure), `n := e` and
`n := ne` (enumerations) are all accepted. Enumeration-to-integer assignment
was never intended; it is a defect fixed here too.

Verified with `ironplcc check`:

| Expression | Declared as | Today |
|---|---|---|
| `ABS(a)` / `F(a)` / `LEN(a)` | `ARRAY[1..2] OF DINT` | accepted |
| `ABS(na)` / `F(na)` / `LEN(na)` | `TYPE Arr : ARRAY[1..2] OF DINT` | accepted |
| `ABS(e)` | `(X, Y)` | accepted |
| `ABS(ne)`, `ABS(r)` | named enum / named struct | P4026 (correct) |
| `n := a` / `na` / `e` / `ne` / `r` | any of the above | accepted |
| `x := a.70` (bit access on a whole array) | `ARRAY[1..2] OF DINT` | accepted |

## Architecture

### `TypeId`

A numeric identity for a **concrete** type, allocated by the
`TypeEnvironment`, and the same numbering the debug section already uses:

- **Elementary types** take their `iec_type_tag` value (ADR-0019,
  `container/src/debug_section.rs`): `BOOL` = 0 … `LDT` = 24. An elementary
  type therefore has one number in the analyzer, codegen and the debugger.
  Spelling aliases (`TOD`/`TIME_OF_DAY`, `DT`/`DATE_AND_TIME`, …) share an ID.
- **IDs 25–255 are reserved** and never allocated, so a `TypeId` can never be
  confused with an aggregate tag (`STRUCT` = 25, `ARRAY` = 26,
  `FB_INSTANCE` = 27) or `OTHER` = 255.
- **Every other type** — named declarations, anonymous declarations, sized
  strings (`STRING[8]`), stdlib and user function blocks — gets an ID ≥ 256
  allocated per compilation. Identity is **nominal**: a named type is one ID
  however it is spelled; an anonymous type is one ID per declaration site, so
  two `(X, Y)` enumerations stay distinct.
- **Generic categories never get a `TypeId`.** `ANY_INT` is a set of types,
  not a type. See *Untyped literals* below.

`TypeId` is an opaque `u32` newtype defined in `ironplc-dsl` (so `Expr` can
hold it without depending on the analyzer). Only the `TypeEnvironment` mints
one, so holding a `TypeId` guarantees the table has an entry for it — a lookup
cannot fail.

The `TypeEnvironment` becomes an indexed table: `Vec<TypeEntry>` by `TypeId`
plus `HashMap<TypeName, TypeId>` for names. A `TypeEntry` holds the
`IntermediateType`, the optional name, and the `TypeId`s of its components
(array element, structure fields, reference target) so that `a[1]`, `s.f` and
`r^` resolve to a precise ID even when the component is a named structure,
which `IntermediateType` alone cannot say.

### Untyped literals

An expression's type becomes:

```rust
pub enum ExprType {
    /// A value of exactly this type.
    Concrete(TypeId),
    /// An untyped literal (or a generic function applied only to one), whose
    /// type is fixed by the context it is used in. ADR-0028 / ADR-0031 decide
    /// which concrete types it may take.
    Literal(GenericTypeName),
}
```

The checks keep using the existing literal rules in `type_compat.rs`;
codegen resolves a `Literal` from its context exactly as it resolves
`ANY_INT` today. Pinning every literal to a concrete type before codegen is a
possible follow-up, not part of this work.

### Debug tag

Codegen derives `iec_type_tag` from the `TypeId`: the ID itself for an
elementary type, else the entry's category (`STRUCT` / `ARRAY` /
`FB_INSTANCE` / `OTHER`). The container format does not change. A debug-section
type table keyed by the same IDs (so the debugger can show
`ARRAY[1..2] OF DINT` instead of today's lossy `ARRAY OF DINT`) is a follow-up
issue.

## Prefactoring

Three reshapes, each behaviour-preserving, before any new behaviour:

1. **One "declaration → type" routine.** Four places answer "what type does
   this variable have", each differently:
   - `xform_resolve_expr_types` — `insert`, `insert_array_element_type`,
     `resolve_variable_type`, `resolve_parent_struct_type`,
     `resolve_struct_field_array_element_type`, plus a separate
     `array_element_types` table
   - `variable_type::resolve_initializer` / `of` — drops an inline array's
     dimensions (`dimensions: vec![]`)
   - `rule_assignment_aggregate_type_compat::declared_type`
   - `rule_function_call_type_check`'s own `var_types` table

   All three passes store declarations in `variable_type::Declarations` and
   derive what they need at lookup. `Declared` gains a `Typed` form for result
   variables and system globals, and the named/inline classification moves onto
   the initializer.

   Two things stay for PR 4, because doing them here changes behaviour:
   - Building inline arrays with their real dimensions. Today
     `variable_type::resolve_initializer` builds them with no dimensions. Adding
     them would start range-checking `a.70` on a whole array while `a.3` stays
     accepted. Bit access on a whole array is part of the #1761 defect.
   - Aligning the two type-name projections. The resolver and
     `rule_function_call_type_check` disagree on inline subranges and
     references. Aligning them would add diagnostics, for example `r := NULL`
     for `r : REF_TO INT` would become a P4035. `TypeId` replaces both.
2. **Explicit enumeration fallback in codegen.** `op_type` treats any
   unrecognised name as an enumeration. Make it ask the type environment
   whether the type *is* an enumeration and report P9999 otherwise.

   This turned out to be a fix, not a prefactor. Named subranges also reach
   the fallback, so a `ULINT` subrange was operated on as a `DINT`
   (`IF x > 4000000000` was rejected with P2026). Codegen now builds
   `named_types` from the type environment: enumerations map to `DINT`,
   subranges to their base type, and anything else is P9999.

   Also found: `ABS(x)` for a named `ULINT` subrange is rejected with P4026,
   because the analyzer compares type names as strings. PR 4 covers it.
3. **`TypeId` in the `TypeEnvironment`.** Introduce the indexed table and
   `TypeEntry`; elementary IDs equal `iec_type_tag`. Existing name lookups go
   name → ID → entry. Codegen's `resolve_iec_type_tag` string match is replaced
   by the ID lookup. Same tags are emitted; no program changes behaviour.

## Design doc reference

- `specs/design/expression-type-resolution.md` — updated in step 4
- ADR-0013 (expression type annotation) — amended by a new ADR in step 3
- ADR-0019 (debug type tags) — referenced, unchanged

## PRs

| # | Kind | Content |
|---|------|---------|
| 1 | Prefactor | Prefactor 1 (one declaration → type routine) |
| 2 | Prefactor | Prefactor 2 (explicit enumeration fallback) |
| 3 | Prefactor | Prefactor 3 (`TypeId` table, debug tag from ID) + ADR |
| 4 | Core | `Expr` carries `ExprType`; anonymous types and named aliases get IDs; inline arrays keep their dimensions; late-bound resolution keeps alias names; argument, assignment and bit-access checks compare IDs and reject aggregates/enumerations where a scalar is expected. Fixes #1761, enum → integer assignment, and bit access on a whole array |
| 5 | Core | Codegen selects opcodes from `TypeId` via the table; remove `resolved_type: Option<TypeName>`, `resolve_type_name` and the string-matching helpers |

A tracking issue records PRs 1–5 before the first core PR.

## File map

| File | Change |
|------|--------|
| `compiler/analyzer/src/variable_type.rs` | Single declaration → type routine; keep inline dimensions |
| `compiler/analyzer/src/xform_resolve_expr_types.rs` | Use `variable_type`; drop `array_element_types`; later produce `ExprType` |
| `compiler/analyzer/src/rule_assignment_aggregate_type_compat.rs` | Use `variable_type` |
| `compiler/analyzer/src/rule_function_call_type_check.rs` | Use `variable_type`; later compare `ExprType` and reject aggregates / enums |
| `compiler/analyzer/src/type_environment.rs` | Indexed `TypeEntry` table, `TypeId` allocation |
| `compiler/analyzer/src/type_compat.rs` | Compatibility over `ExprType` |
| `compiler/analyzer/src/xform_resolve_late_bound_type_initializer.rs` | Keep the alias name for named arrays |
| `compiler/dsl/src/textual.rs` | `TypeId`, `ExprType`, `Expr` field |
| `compiler/codegen/src/compile_expr.rs` | Explicit enum fallback; later opcode selection from `TypeId` |
| `compiler/codegen/src/compile_setup.rs` | `iec_type_tag` from `TypeId` |
| `compiler/codegen/src/string_width.rs` | Shape from `TypeId` rather than the access root |
| `specs/adrs/00NN-concrete-type-ids.md` | New ADR, amends ADR-0013 |
| `specs/design/expression-type-resolution.md` | Describe `ExprType` / `TypeId` |
| `docs/compiler/problems/P4026.rst`, `P4035.rst` | Examples for aggregates and enumerations |

## Tasks

- [ ] Open tracking issue for PRs 1–5 and a follow-up issue for the debug-section type table
- [x] PR 1: one `Declarations` table for the resolver and the argument/assignment rule (tests unchanged): ironplc/ironplc#1802
- [x] PR 2: codegen takes a named type's operand type from the type environment: ironplc/ironplc#1810
- [ ] PR 3: `TypeId` table in `TypeEnvironment`; elementary IDs = `iec_type_tag`; debug tag from ID; ADR
- [ ] PR 4: `ExprType` on `Expr`; IDs for anonymous types and aliases; checks reject aggregates/enums where a scalar is expected; tests for every row of the table above
- [ ] PR 5: codegen on `TypeId`; remove string-based type resolution; update design doc
- [ ] Close #1761 and the tracking issue
