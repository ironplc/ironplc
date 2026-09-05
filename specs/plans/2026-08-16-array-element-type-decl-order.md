# Plan: Fix Declaration-Order Sensitivity for Array-Typed Struct Fields

## Context

A structure field declared as an array of a user-defined type — for example
`Items : ARRAY[1..6] OF Item;` — is rejected with a spurious
P2013 `ArrayElementTypeNotDeclared` depending on where the declarations
appear in the file. The behaviour is deterministic, not flaky:

| File layout (identical declarations)               | `check`        |
| -------------------------------------------------- | -------------- |
| `PROGRAM` … then `TYPE Item`, `TYPE Holder`         | passes         |
| `TYPE Item`, `TYPE Holder` … then `PROGRAM`         | **fails P2013** |

Reported from the playground in
[#1376](https://github.com/ironplc/ironplc/issues/1376), where the reporter's
source only escaped the error because its `TYPE` blocks happened to sit after
`END_PROGRAM`.

### Root cause

`xform_toposort_declarations` orders declarations so that a type is always
emitted before anything that references it. Struct fields contribute their
dependency edges in `visit_initial_value_assignment_kind`, which handles
`Structure`, `FunctionBlock`, `FunctionBlockCall` and `LateResolvedType` —
but the `Array` arm is empty:

```rust
InitialValueAssignmentKind::Array(_) => {}
```

So an array-typed struct field records no dependency on its element type. The
toposort is then free to order the containing struct before the element type,
and `intermediates::array::try_from` looks the element type up in a
`TypeEnvironment` that does not yet contain it, producing P2013.

`visit_array_declaration` already adds exactly this edge for a *top-level*
`TYPE MyArr : ARRAY[1..6] OF Item; END_TYPE`, which is why only the
struct-field form is affected.

## Approach

Populate the `Array` arm with the same edge `visit_array_declaration` adds,
using the referenced-type-before-container direction (`add_edge(to, from)`)
that the neighbouring arms in this function already use.

Both `SpecificationKind` variants carry an element type name:

- `Named(type_name)` — field declared via a named array type
- `Inline(subranges)` — `subranges.type_name.to_type_name()`

`ArrayElementType::to_type_name()` also yields a name for `STRING`/`WSTRING`
elements; adding a node for an elementary type name is already what
`visit_array_declaration` does, and unresolvable nodes are dropped by the
`filter_map` in `apply()`, so no special-casing is needed.

### Cycle safety

Adding edges can turn a previously unordered graph into a cyclic one. A cycle
here requires a genuinely recursive declaration (`TYPE A : STRUCT x : ARRAY[1..2]
OF A; END_STRUCT`), which is infinite-sized and must be rejected. Confirm the
existing P2011 `RecursiveCycle` path reports it rather than hanging.

## Changes

1. **`xform_toposort_declarations.rs`** — replace the empty
   `InitialValueAssignmentKind::Array(_)` arm with element-type edge
   construction for both `SpecificationKind` variants.

2. **Unit tests** (same file) — declaration-order independence for an
   array-of-struct field, plus recursive-array-field cycle detection.

3. **Integration test** — an end-to-end `check` over the ordering that
   previously failed, guarding the analyzer pipeline as a whole.

## Scope

This fixes only the analyzer ordering bug. Codegen support for arrays whose
element type is a struct remains unimplemented (`compile_struct.rs` L308 and
`compile_array.rs` "Unsupported array element type"); that is the separate,
larger piece of work tracked by #1376 itself. After this change the reported
source reaches codegen and fails there instead of in `check`.

## Files

| File                                                        | Change                     |
| ----------------------------------------------------------- | -------------------------- |
| `compiler/analyzer/src/xform_toposort_declarations.rs`       | Edge construction + tests  |
| `compiler/analyzer/src/lib.rs` (or integration test module)   | Pipeline-level order test  |
