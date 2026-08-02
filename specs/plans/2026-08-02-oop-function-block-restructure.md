# Plan: Restructure `FunctionBlockDeclaration` OOP fields into `Option<FunctionBlockOop>`

## Goal

Address garretfick's PR #1287 review: replace the three always-present,
flat `extends`/`implements`/`is_abstract` fields on `FunctionBlockDeclaration`
with a single `pub oop: Option<FunctionBlockOop>` sub-node, so that:

1. An ordinary (non-OOP) function block cannot carry empty-but-present OOP
   data — `oop: None` makes that state unrepresentable rather than a
   field-contents check.
2. `VendorExtension` is implemented on `FunctionBlockOop`, not on
   `FunctionBlockDeclaration` — a plain FB no longer structurally claims to
   be a vendor extension.
3. There is a single, named home for `METHOD`/`PROPERTY` to land on in
   later PRs, and for the ADR-0041 Phase 2 `oop.is_some()` hook.

This is scoped exactly to garretfick's point 1 (the AST-shape review
comment on `compiler/dsl/src/common.rs:2792`). His point 2 (interface
registered as `IntermediateType::Structure { fields: [] }`) is a separate,
already-tracked concern and out of scope here — it doesn't share any code
paths with this restructuring. The still-open P4037.rst question was fixed
separately (already committed).

## Non-goals

- Interface intermediate-representation fix (garretfick's point 2) — separate work.
- Any new OOP semantics (dispatch, override checking, `METHOD`/`PROPERTY`) — later PRs, per ADR-0041.
- Changing `InterfaceDeclaration` — it already has its own correct shape (`extends: Vec<TypeName>`, no flat/optional confusion) and isn't touched.

## Design

```rust
pub struct FunctionBlockDeclaration {
    pub name: TypeName,
    pub variables: Vec<VarDecl>,
    pub edge_variables: Vec<EdgeVarDecl>,
    pub body: FunctionBlockBodyKind,
    pub span: SourceSpan,
    pub oop: Option<FunctionBlockOop>,
}

pub struct FunctionBlockOop {
    pub base: Option<TypeName>,       // EXTENDS (single-inheritance)
    pub implements: Vec<TypeName>,    // IMPLEMENTS (multiple)
    #[recurse(ignore)]
    pub is_abstract: bool,            // ABSTRACT
    pub span: SourceSpan,             // span of the OOP-related tokens
}

impl VendorExtension for FunctionBlockOop { ... }
```

Field renamed `extends` → `base` per garretfick's naming rationale (avoid a
shared name with `InterfaceDeclaration::extends: Vec<TypeName>`, which has
different cardinality).

`oop` is `Some` whenever any of `ABSTRACT`/`EXTENDS`/`IMPLEMENTS` was parsed
on the FB header, `None` otherwise (same "presence" test as today's
`extends.is_some() || !implements.is_empty() || is_abstract`, just
collapsed to one condition).

### Span computation

`ABSTRACT` (if present) appears *before* the FB name in the grammar;
`EXTENDS`/`IMPLEMENTS` appear *after* it. `FunctionBlockOop.span` is
computed by joining the span of the earliest present OOP token through the
end of the last present OOP token/type-name (using `SourceSpan::join`,
min-start/max-end over whichever of `ABSTRACT`, `EXTENDS <name>`,
`IMPLEMENTS <list>` are present). When `ABSTRACT` and `EXTENDS`/`IMPLEMENTS`
are both present, the computed span incidentally includes the FB name
between them — an acceptable imprecision, and still materially tighter than
today's behavior (`extension_span()` currently returns the *entire* FB
span, start-of-`FUNCTION_BLOCK` to end-of-`END_FUNCTION_BLOCK`).

### `rule_unsupported_extension.rs`

`visit_function_block_declaration` changes from inspecting
`node.implements`/`node.is_abstract` directly to:

```rust
if let Some(oop) = &node.oop {
    if !oop.implements.is_empty() || oop.is_abstract {
        self.flag(oop);
    }
}
```

`self.flag` takes `&dyn VendorExtension`, so this now passes `oop`
(`&FunctionBlockOop`) instead of `node`. No behavior change: plain `EXTENDS`
alone still doesn't flag (already-resolved field inheritance); `IMPLEMENTS`
and `ABSTRACT` still do.

## File Map

| File | Change |
|---|---|
| `compiler/dsl/src/common.rs` | Replace 3 flat fields with `oop: Option<FunctionBlockOop>`; add `FunctionBlockOop` struct; move `VendorExtension` impl onto it |
| `compiler/dsl/src/visitor.rs` | Add `dispatch!(FunctionBlockOop);` so the new struct participates in `Recurse`/`Visitor` |
| `compiler/parser/src/parser.rs` | `function_block_declaration()` rule: capture OOP tokens, build `Option<FunctionBlockOop>` with computed span |
| `compiler/analyzer/src/rule_unsupported_extension.rs` | Visit through `node.oop`, flag the `FunctionBlockOop`, not the FB |
| `compiler/analyzer/src/xform_toposort_declarations.rs` | `node.extends` → `node.oop.as_ref().and_then(\|o\| o.base.as_ref())` |
| `compiler/analyzer/src/rule_abstract_not_instantiated.rs` | `fb.is_abstract` → `fb.oop.as_ref().is_some_and(\|o\| o.is_abstract)` |
| `compiler/analyzer/src/intermediates/inherited_fields.rs` | `fb.extends`/`.extends` (x2) → `fb.oop.as_ref().and_then(\|o\| o.base.as_ref())` |
| `compiler/plc2plc/src/renderer.rs` | `visit_function_block_declaration`: render through `node.oop` (`is_abstract`/`base`/`implements`) |
| `compiler/analyzer/src/xform_resolve_late_bound_type_initializer.rs` | 4 test-fixture constructor sites: `extends`/`implements`/`is_abstract` → `oop: None` |
| `compiler/sources/src/xml/transform.rs` | 1 constructor site → `oop: None` |
| `compiler/parser/src/tests/corpus.rs` | 3 constructor sites → `oop: None` |
| `compiler/parser/src/tests/oop_extensions.rs` | Update all field assertions to go through `fb.oop` |

No changes needed in `sources/src/parsers/twincat_parser.rs` or
`plc2plc/src/tests/oop_extensions.rs` — both only touch
`InterfaceDeclaration::extends`, which is unaffected.

## Testing Strategy

- Existing `parser/src/tests/oop_extensions.rs` suite updated in place —
  same coverage (EXTENDS-only, IMPLEMENTS-only, both, multi-interface,
  ABSTRACT combinations, absence cases), just asserting through
  `fb.oop`/`fb.oop.as_ref().unwrap()` instead of flat fields.
- Existing `rule_unsupported_extension.rs` test suite covers the same
  cases (plain EXTENDS ok, IMPLEMENTS/ABSTRACT flag, one diagnostic per FB)
  — no new tests needed, behavior is unchanged, only representation.
- `cargo build` will surface any exhaustive-match / field-access site this
  plan's file map missed; treat that as the authoritative completeness
  check per this branch's established convention.
- Full CI (`cd compiler && just`) before pushing.

## Tasks

- [x] Add `FunctionBlockOop` struct + `oop` field to `FunctionBlockDeclaration`; move `VendorExtension` impl (`dsl/src/common.rs`)
- [x] `dispatch!(FunctionBlockOop);` in `dsl/src/visitor.rs` (and `dsl/src/fold.rs`, surfaced by `cargo build`)
- [x] Update `function_block_declaration()` parser rule to build `Option<FunctionBlockOop>` with computed span
- [x] Update `rule_unsupported_extension.rs`
- [x] Update `xform_toposort_declarations.rs`
- [x] Update `rule_abstract_not_instantiated.rs`
- [x] Update `intermediates/inherited_fields.rs`
- [x] Update `plc2plc/src/renderer.rs`
- [x] Update all test-fixture constructor sites (`xform_resolve_late_bound_type_initializer.rs`, `sources/src/xml/transform.rs`, `parser/src/tests/corpus.rs`)
- [x] Update `parser/src/tests/oop_extensions.rs` assertions
- [x] `cargo build` clean, fix any remaining sites it surfaces (also needed `dsl/src/fold.rs` dispatch entry, not anticipated in the file map)
- [x] Run full CI (`cd compiler && just`) — clean
- [ ] Commit, push to fork
