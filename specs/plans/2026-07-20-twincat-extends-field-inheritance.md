# Plan: `EXTENDS` Field Inheritance for Function Blocks

## Goal

`FUNCTION_BLOCK FB_Derived EXTENDS FB_Base` already parses (and is
recognized as a vendor extension), but any *unqualified* reference to a
field declared only on `FB_Base`, from within `FB_Derived`'s own body,
fails with `P4007 Undefined variable`. Real files in the survey that
motivated this bucket (25 files) only need this: ordinary field access
through single inheritance, no method dispatch. This plan implements
just that — field visibility through the `EXTENDS` chain — and leaves
method dispatch, access modifiers, and `THIS^`/`SUPER^` for the separate
"Full OOP" item already tracked in `twincat-status.md`.

```
FUNCTION_BLOCK FB_Base
VAR
    bEnabled : BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
VAR
    bRunning : BOOL;
END_VAR
bRunning := bEnabled;  // currently P4007: bEnabled is "undefined"
END_FUNCTION_BLOCK
```

## Verification against real code

Traced the exact failure to `rule_use_declared_symbolic_var.rs`: its
`visit_function_block_declaration` opens a fresh scope
(`ScopedTable::enter()`) and populates it only from
`node.recurse_visit(self)`, which walks *this* FB's own `variables` —
never the base class's. A second, independent gap exists in
`xform_resolve_expr_types.rs`'s `fold_function_block_declaration`: it
builds a per-FB `var_types: HashMap<Id, TypeName>` the same way, only
from `node.variables`, so even if the scoping gap were fixed, an
expression using an inherited field (e.g. `bEnabled AND bRunning`)
still wouldn't type-check correctly.

Also checked `xform_toposort_declarations.rs`: `visit_interface_declaration`
already adds a toposort edge for its own `extends` (interface-to-interface),
but `visit_function_block_declaration` adds **no edge at all** for its own
`extends` — meaning today, an `EXTENDS` cycle between function blocks
(`FB_A EXTENDS FB_B EXTENDS FB_A`) is not caught by anything, and there's
no ordering guarantee that the base class is processed before the
derived one.

## Design

### 1. Toposort: add the missing `FUNCTION_BLOCK` `EXTENDS` edge

Parallel to the existing interface case:

```rust
fn visit_function_block_declaration(
    &mut self,
    node: &FunctionBlockDeclaration,
) -> Result<Self::Value, Diagnostic> {
    self.current_from = Some(node.name.name.clone());
    let this = self.declarations.add_node(&node.name.name);
    if let Some(parent) = &node.extends {
        let depends_on = self.declarations.add_node(&parent.name);
        self.declarations.graph.add_edge(depends_on, this, ());
    }
    let res = node.recurse_visit(self);
    self.current_from = None;
    res
}
```

This gives two things for free: correct base-before-derived ordering,
and cycle detection via the existing `RecursiveCycle` diagnostic — no
new diagnostic type needed. Since toposort is a hard failure early in
`resolve_types` (`let (mut library, reachable) =
xform_toposort_declarations::apply(library)?;`), every later pass can
assume the `EXTENDS` graph is acyclic.

### 2. New shared helper: `collect_inherited_fields()`

A pure function, not a pipeline stage — both consumers below run at
different points in the pipeline (one in stage 2's fold passes, one in
stage 3's semantic rules) and each already has its own `&Library` in
hand, so a stateless helper avoids threading a new field through
`SemanticContext` or the pipeline signature.

```rust
// compiler/analyzer/src/intermediates/inherited_fields.rs

/// For every FUNCTION_BLOCK with an EXTENDS clause, returns the
/// transitive list of fields inherited from its ancestor chain (not
/// including its own fields). Base-to-derived order, so a caller that
/// applies its own fields last gets correct shadowing (a redeclared
/// field on a derived FB wins over the ancestor's).
///
/// Assumes the EXTENDS graph is acyclic (enforced by
/// xform_toposort_declarations's RecursiveCycle check, which runs
/// earlier in the pipeline and is a hard failure). Includes a
/// currently-resolving guard purely as defense in depth -- a helper
/// that could infinite-loop on malformed input is a footgun regardless
/// of upstream guarantees.
///
/// A dangling EXTENDS target (no FunctionBlockDeclaration with that
/// name in the library) silently contributes no inherited fields --
/// existing behavior for referencing an undeclared base is unchanged
/// by this function.
pub fn collect_inherited_fields(lib: &Library) -> HashMap<TypeName, Vec<VarDecl>>
```

Internally: build a `TypeName -> &FunctionBlockDeclaration` map from the
library's `FunctionBlockDeclaration` elements, then for each FB with
`extends`, recursively resolve the parent's own-plus-inherited fields
(memoizing per name to avoid recomputation across a shared ancestor).

### 3. Consumer: `rule_use_declared_symbolic_var.rs`

In `visit_function_block_declaration`, after `self.enter()` and before
recursing, add the inherited field names (computed once outside the
visitor and passed in, or computed lazily per FB — whichever is simpler
given the visitor's existing shape):

```rust
self.enter();
self.add(&node.name.name, DummyNode {});
if let Some(fields) = inherited_fields.get(&node.name) {
    for f in fields {
        self.add_if(f.identifier.symbolic_id(), DummyNode {});
    }
}
let ret = node.recurse_visit(self);
```

### 4. Consumer: `xform_resolve_expr_types.rs`

In `fold_function_block_declaration`, insert inherited fields' types
into `var_types` *before* the FB's own fields, so `self.insert(v)` for
own fields naturally overrides on a name collision (last insert wins in
a `HashMap`):

```rust
fn fold_function_block_declaration(
    &mut self,
    node: FunctionBlockDeclaration,
) -> Result<FunctionBlockDeclaration, Diagnostic> {
    if let Some(fields) = self.inherited_fields.get(&node.name) {
        fields.clone().iter().for_each(|v| self.insert(v));
    }
    node.variables.iter().for_each(|v| self.insert(v));
    ...
```

`inherited_fields` becomes a new field on `ExprTypeResolver`, computed
once in `xform_resolve_expr_types::apply` from the incoming `Library`
before folding starts (same lifetime pattern as `type_environment`/
`function_environment`, already borrowed for the whole pass).

### 5. `P9004` gating change (per explicit decision)

`rule_unsupported_extension.rs`'s `visit_function_block_declaration`
currently flags on `extends.is_some() || !implements.is_empty() ||
is_abstract`. Once field inheritance resolves correctly, a plain
`EXTENDS` with no `IMPLEMENTS` and not `ABSTRACT` has nothing left
unsupported for the shape these 25 files actually use — drop `extends`
from the condition:

```rust
if !node.implements.is_empty() || node.is_abstract {
    self.flag(node);
}
```

`IMPLEMENTS` (interface dispatch still unimplemented) and `ABSTRACT`
(instantiation-legality still unenforced) keep flagging. A pure
`EXTENDS`-only function block no longer produces `P9004` at all.

## Non-goals

- Method dispatch, `METHOD`/`PROPERTY` bodies, access modifiers,
  `THIS^`/`SUPER^` — tracked separately as "Full OOP" in
  `twincat-status.md`; `METHOD`/`PROPERTY` still aren't parsed as part
  of a function block body at all (rejected outright), unaffected by
  this change.
- Qualified external field access on FB instances (`instance.field`
  reading a *non-inherited*, ordinary field from outside) —
  `xform_resolve_expr_types.rs`'s `resolve_struct_type` only recognizes
  `IntermediateType::Structure`, not `IntermediateType::FunctionBlock`,
  so this doesn't work today even with zero inheritance involved. A
  pre-existing, unrelated gap; out of scope here.
- Validating that an `EXTENDS` target actually resolves to a declared
  function block — a dangling reference silently contributes no
  inherited fields (existing "undeclared type" behavvior for the
  `extends` target itself is untouched).
- Field shadowing rules beyond "derived wins by name" — no real file in
  the survey redeclares an inherited field, so this is the obvious
  default, not a deeply validated policy.
- Multiple inheritance for function blocks — the grammar only allows a
  single `EXTENDS` target for `FUNCTION_BLOCK` (`type_name()`, not a
  list), unlike `INTERFACE EXTENDS` which already supports a list; no
  diamond-inheritance question arises here.

## File Map

| File | Change |
|------|--------|
| `compiler/analyzer/src/xform_toposort_declarations.rs` | Add the missing `FUNCTION_BLOCK` `extends` edge |
| `compiler/analyzer/src/intermediates/inherited_fields.rs` | New: `collect_inherited_fields()` |
| `compiler/analyzer/src/rule_use_declared_symbolic_var.rs` | Seed inherited field names into scope |
| `compiler/analyzer/src/xform_resolve_expr_types.rs` | Seed inherited fields' types into `var_types` |
| `compiler/analyzer/src/rule_unsupported_extension.rs` | Drop `extends` from the `P9004` flagging condition |

## Testing Strategy

- `collect_inherited_fields()` unit tests: single-level inheritance,
  multi-level (transitive) inheritance, shadowing (derived field wins),
  no-`extends` FB (empty result), dangling `extends` target (empty
  result, no panic).
- `xform_toposort_declarations.rs`: new regression test for an
  `EXTENDS` cycle between function blocks producing `RecursiveCycle`
  (parallel to the existing interface-cycle test, if one exists —
  confirm during implementation); a forward-reference test (derived FB
  declared textually before its base) to confirm ordering now works.
- `rule_use_declared_symbolic_var.rs`: unqualified access to an
  inherited field now resolves (`Ok`); an actually-undeclared field
  (typo, not on any ancestor) still produces `P4007`.
- `xform_resolve_expr_types.rs`: an expression combining an inherited
  field with an own field (e.g. `bEnabled AND bRunning`) type-checks
  correctly end-to-end via `analyze()`.
- `rule_unsupported_extension.rs`: plain `EXTENDS` (no `IMPLEMENTS`, not
  `ABSTRACT`) no longer produces `P9004`; `IMPLEMENTS` and `ABSTRACT`
  still do, alone or combined with `EXTENDS`.
- End-to-end: verify via the CLI that the original repro (`bRunning :=
  bEnabled;` referencing a base-class field) now passes clean under
  `--dialect=codesys`.

## Tasks

- [x] Write plan (this document)
- [x] Toposort: add the `FUNCTION_BLOCK` `extends` edge
- [x] `collect_inherited_fields()` + unit tests
- [x] Wire into `rule_use_declared_symbolic_var.rs`
- [x] Wire into `xform_resolve_expr_types.rs`
- [x] `rule_unsupported_extension.rs`: drop `extends` from the `P9004` condition
- [x] Tests from Testing Strategy
- [x] Verify end-to-end via CLI
- [x] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork
- [ ] Merge into `twincat-dev`, update `twincat-status.md`, push

## Implementation Notes

- `rule_use_declared_symbolic_var.rs` previously implemented `Visitor`
  directly on the generic `ScopedTable<'_, Id, DummyNode>` type, which
  can't carry the new `inherited_fields` map (can't add a field to a
  generic type defined in another module). Introduced a thin
  `SymbolScopeChecker` wrapper struct (`table` + `inherited_fields`) and
  moved the `Visitor` impl onto it instead, delegating every `ScopedTable`
  call through `self.table`.
- `collect_inherited_fields()` needed two passes internally, not one: a
  top-level function that returns *only* the inherited set (excluding a
  function block's own fields, since those belong to the caller to
  insert separately for shadowing), backed by a memoized recursive helper
  that computes *own-plus-inherited* per name (needed internally so a
  shared ancestor reached through multiple descendants isn't
  recomputed). Conflating the two initially caused every "inherited
  fields" test to also include the FB's own fields — caught immediately
  by the unit tests, not downstream.
- Verified end-to-end via the CLI: the original repro
  (`FB_Derived EXTENDS FB_Base` with `bRunning := bEnabled;` referencing
  the base's field) now parses and analyzes clean under
  `--dialect=codesys`, and a plain `IMPLEMENTS`-only function block still
  correctly produces `P9004`.
- Full workspace test suite (652+ tests) passed on the first run after
  wiring all four consumers — no unrelated regressions surfaced.
