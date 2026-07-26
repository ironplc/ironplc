# Plan: General Expressions as Struct/FB-Instance Initializer Values

## Goal

Item 2 from `twincat-status.md`'s "Next" list. Support a general
expression -- specifically a pointer dereference plus member access -- as
the value in a structured/call-style initializer's `name := value` pairs:

```
FUNCTION_BLOCK FB_Example
VAR
    pDevice : REF_TO FB_Device;
    tonDelta : TON := (PT := pDevice^.Delta);
END_VAR
END_FUNCTION_BLOCK
```

Currently **fails to parse** — confirmed directly: `P0002` exactly at the
`^` in `pDevice^.Delta`.

## Verification against the real file

Reproduced directly against the current parser (not assumed from the
status doc's prior framing): the failing construct is a `TON` (a
function-block type) declared with a `:=`-prefixed parenthesized
initializer, `(PT := pDevice^.Delta)`. Traced the exact grammar path: this
goes through `initialized_structure__without_ambiguous()` ->
`structure_initialization()` -> `structure_element_initialization()`
(`compiler/parser/src/parser.rs`), which is used identically for plain
`STRUCT` type initializers and function-block call-style initializers --
both share `StructureInitializationDeclaration`/
`StructInitialValueAssignmentKind`. `structure_element_initialization()`'s
value position only accepts `constant()`, `enumerated_value()`,
`array_initialization()`, or `structure_initialization()` -- no general
`expression()` fallback, which is exactly what rejects `pDevice^.Delta`
(a dereference-then-member-access chain, not a bare constant).

Confirmed the fix is narrowly scoped to this one rule: ordinary `VAR`
initializers already route through `expression()` (via the
`--allow-constant-initializer-expressions` work) and already parse `^.`
fine; this gap is specific to the parenthesized structured/call-style
initializer position.

## Design

### Not the same shape as `--allow-constant-initializer-expressions`

PR #1220's `SimpleExpr`/`xform_fold_initializer_expressions` requires the
expression to fold to a compile-time constant (`P4037` if it doesn't).
`pDevice^.Delta` is fundamentally **not** a compile-time constant -- it
depends on a runtime pointer value. This is not a "constant expression"
feature; it's the same shape as the *already-supported* FB call-style
instantiation params (`comm : FB_Comm(retries := 3, THIS)` from PR #1222),
where `param_assignment()` already accepts arbitrary runtime expressions
for named/positional inputs. The `:=`-prefixed parenthesized form
(`TON := (PT := ...)`) is a second, structurally distinct spelling of a
call-style initializer that happens to share a grammar rule with plain
`STRUCT` initializers -- and that shared rule is the one that's too
narrow.

### Grammar: `expression()` as a last-resort alternative

```rust
rule structure_element_initialization() -> StructureElementInit =
  name:structure_element_name() _ tok(TokenType::Assignment) _
  init:(c:constant() { StructInitialValueAssignmentKind::Constant(c) }
      / ev:enumerated_value() { StructInitialValueAssignmentKind::EnumeratedValue(ev) }
      / ai:array_initialization() { StructInitialValueAssignmentKind::Array(ai) }
      / si:structure_initialization() { StructInitialValueAssignmentKind::Structure(si) }
      / ex:expression() { StructInitialValueAssignmentKind::Expression(Expr::new(ex)) }) {
    StructureElementInit { name, init }
  }
```

Added last in the ordered choice, so existing constant/enum/array/struct
cases are unaffected (PEG tries alternatives in order; `expression()` only
fires when the earlier four all fail) -- same permissive-superset,
zero-regression-risk pattern used for every prior grammar addition in this
series.

### DSL: `StructInitialValueAssignmentKind::Expression(Expr)`

```rust
// compiler/dsl/src/common.rs
pub enum StructInitialValueAssignmentKind {
    Constant(ConstantKind),
    EnumeratedValue(EnumeratedValue),
    Array(Vec<ArrayInitialElementKind>),
    Structure(Vec<StructureElementInit>),
    /// A general expression value (e.g. `pDevice^.Delta`) -- used for
    /// call-style FB-instance/struct initializers where the value is
    /// computed at instantiation time, not a compile-time constant. See
    /// specs/plans/2026-07-26-twincat-struct-init-expression-value.md.
    Expression(Expr),
}
```

Already `#[derive(Recurse)]` -- the new `Expr` field is automatically
walked by the existing generic visitor/fold dispatch
(`dsl/src/visitor.rs`/`fold.rs`'s `dispatch!(StructInitialValueAssignmentKind)`),
so it's automatically type-resolved by the existing
`xform_resolve_expr_types.rs` pass with no new plumbing there -- the same
"faithful representation, no persistent provenance marker" shape ADR-0040
endorses (this *is* real source content, not an inference).

### Semantic analysis: no new restriction

Unlike `SimpleExpr`, this value is **not** required to reduce to a
constant -- it's accepted as a genuine runtime expression, matching how
`param_assignment()`'s named/positional inputs already work for FB
call-style construction. No new problem code for "must be constant."

### Codegen: explicit "not implemented" rather than silently wrong/no-op

`compile_struct.rs`'s `compile_struct_field_init` currently no-ops
`Array`/`Structure` (pre-existing gap, out of scope here). For the new
`Expression` variant, return `Diagnostic::not_implemented(...)` (reusing
the existing generic `P9998`/`NotImplemented` code, same as `AND_THEN`'s
codegen stub) rather than silently emitting no bytecode for the field --
`ironplcc check` fully supports the construct (the actual motivating use
case); `ironplcc compile` fails clearly instead of producing an
uninitialized field.

## Non-goals

- Compile-time constant folding for this position -- the whole point is
  supporting genuinely runtime values; `--allow-constant-initializer-expressions`
  already covers the compile-time-foldable case for plain `VAR`
  initializers.
- Codegen/VM execution of the runtime-evaluated initializer -- explicitly
  refused with `not_implemented`, matching the `AND_THEN` precedent.
- Any change to `Array`/`Structure` struct-field-init codegen (pre-existing
  no-op, unrelated to this change).

## File Map

| File | Change |
|------|--------|
| `compiler/dsl/src/common.rs` | New `StructInitialValueAssignmentKind::Expression(Expr)` variant |
| `compiler/parser/src/parser.rs` | `expression()` fallback alternative in `structure_element_initialization()` |
| `compiler/codegen/src/compile_struct.rs` | `Expression` arm -> `not_implemented` in `compile_struct_field_init`'s match |
| `compiler/plc2plc/src/renderer.rs` | Render `Expression` (visitor recursion should cover this automatically; verify and add an explicit case only if needed) |

## Testing Strategy

- Parser test: the real motivating shape
  (`tonDelta : TON := (PT := pDevice^.Delta);`) parses, with
  `StructInitialValueAssignmentKind::Expression` holding the deref+member
  expression.
- Parser regression: existing constant/enum/array/struct-value struct
  inits still parse unchanged (ordering doesn't shadow them).
- plc2plc round-trip test for the new shape.
- Codegen test: compiling a struct/FB-instance init using the `Expression`
  variant produces `Diagnostic::not_implemented` rather than silently
  emitting no bytecode.
- End-to-end: verify via the CLI that `ironplcc check` accepts the real
  motivating shape under `--dialect=codesys`.

## Tasks

- [x] Write plan (this document)
- [x] Verify the real failing shape and trace the exact grammar rule
- [ ] DSL: `StructInitialValueAssignmentKind::Expression(Expr)`
- [ ] Grammar: `expression()` fallback in `structure_element_initialization()`
- [ ] Codegen: `not_implemented` for the new variant
- [ ] Check plc2plc renderer; add explicit case only if the generic visitor doesn't already cover it
- [ ] Tests from Testing Strategy
- [ ] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork
