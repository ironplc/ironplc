# Plan: Constant Expressions in VAR Initializers

**Status: plan only, implementation deliberately deferred** (per user
decision — this is bigger than the previous three PRs and deserves its own
session rather than being rushed alongside them).

## Goal

Allow a variable initializer to be a constant *expression* (arithmetic
between literals and/or references to declared `CONSTANT`s), not just a
single bare literal as today:

```
VAR
    d2r : LREAL := PI/180.0;        // currently a syntax error
    asec2r : LREAL := PI/(180.0*3600.0);  // currently a syntax error
END_VAR
```

This is the actual dominant blocker behind the "`PI` used as a bare
identifier" survey item (see
[2026-07-19-twincat-pi-constant.md](2026-07-19-twincat-pi-constant.md),
paused pending this work). Of ~18 real files using `PI`, only ~2 use it in
*statement* context (already works); the rest use it as a `VAR` initializer,
which fails today regardless of whether `PI` itself is a known symbol.

## How this was discovered

While implementing the `PI`-constant plan, testing against the real usage
pattern surfaced that `d2r : LREAL := PI/180.0;` is a **parse error today**,
independent of `PI`. Isolated exactly which forms fail via direct parser
tests:

| Initializer | Parses? |
|---|---|
| `x : LREAL := 3.14;` | ✅ (bare literal) |
| `x : LREAL := 3.14 / 180.0;` | ❌ (arithmetic) |
| `x : LREAL := SOME_CONST;` | ❌ (bare identifier reference) |
| `x : LREAL := SOME_CONST/180.0;` | ❌ (identifier + arithmetic) |
| `x := SOME_CONST/180.0;` (statement, not initializer) | ✅ |

Traced the cause to `compiler/parser/src/parser.rs`: the initializer
grammar rules (`simple_spec_init()`, `simple_or_enumerated_or_subrange_ambiguous_struct_spec_init()`,
and siblings feeding `var1_init_decl`/`var2_init_decl`) all call `constant()`
— which is IEC 61131-3's own B.1.2 grammar production, and is *itself*
literal-only per the standard. So this isn't an IronPLC bug relative to the
standard; it's that real CODESYS/TwinCAT compilers accept a broader
"constant expression" than the strict standard grammar, the same relationship
`allow_constant_type_params` already has for `STRING[MY_CONST]`/array bounds
(that flag's own docstring: "the IEC 61131-3 standard requires integer
literals in these positions... vendor extension").

## Why this needs care (and isn't "just relax the grammar")

Checked [ADR-0024](../adrs/0024-function-local-reinit-via-init-template.md)
("Function Local Re-initialization via Init Template"): IronPLC deliberately
pre-computes every variable's initial value at compile time into a static
template blob, memcpy'd at runtime — and explicitly rejected the alternative
of emitting runtime initialization bytecode, for embedded/no_std
performance and simplicity. **This means an initializer expression must
fully constant-fold at compile time or be a compile error — it can never be
deferred to runtime.** This is architecturally consistent (not a fight
against the existing design), but it does mean the fix isn't merely a
grammar relaxation — it requires an actual compile-time evaluator.

Checked how big the naive version of this change would be: grepped for
`SimpleInitializer`/`InitialValueAssignmentKind::Simple` outside tests —
**21 files** reference it (every semantic rule from `rule_var_decl_const_initialized.rs`
to `rule_ref_to.rs`, codegen's `compile_setup.rs`/`compile_fn.rs`, the
plc2plc renderer, MCP tools, XML transform). Changing
`SimpleInitializer.initial_value`'s type directly (`Option<ConstantKind>` →
something expression-shaped) would force changes across most of those 21
files.

## Design: normalize away early, don't change the shared type

The codebase already has a precedent for exactly this shape of problem:
`IntegerRef::Constant(Id)` / `LateBound` — a "parsed but not yet resolved"
placeholder variant that a dedicated `xform_resolve_*` pass later replaces
with the fully-resolved form, so everything downstream never has to know
the placeholder existed. Apply the same pattern here, scoped to keep the
21-file blast radius at effectively zero:

1. **New AST variant**, not a changed type:

   ```rust
   // compiler/dsl/src/common.rs
   pub enum InitialValueAssignmentKind {
       // ...existing variants unchanged...
       /// A constant-expression initializer not yet folded to a literal
       /// (vendor extension — see allow_constant_initializer_expressions).
       /// Always normalized to `Simple` by
       /// `xform_fold_initializer_expressions` before any other pass runs;
       /// no other code should ever match on this variant.
       SimpleExpr(SimpleExprInitializer),
   }

   pub struct SimpleExprInitializer {
       pub type_name: TypeName,
       pub initial_value: Expr,
   }
   ```

2. **Grammar**: in each initializer production (`simple_spec_init()` and
   siblings in `parser.rs`), try `constant()` first (ordered choice — matches
   today's behavior byte-for-byte for the common bare-literal case,
   zero risk of regression), and only on failure fall back to the general
   `expression()` rule, producing `SimpleExpr` instead of `Simple`. The
   grammar itself is **not** gated by `allow_constant_initializer_expressions`
   — it always accepts the broader form (matches the project's existing
   "Validation Rule Pattern": lex/parse unconditionally, reject later when
   the flag is off — see `syntax-support-guide.md`).

3. **New early semantic pass**, `xform_fold_initializer_expressions`,
   runs *before* every existing pass that touches `SimpleInitializer` (right
   alongside `xform_resolve_constant_expressions` in
   `analyzer::stages::resolve_types`):
   - Collect all known `CONSTANT`-qualified declarations and their literal
     values (parallels `xform_resolve_constant_expressions.rs`'s
     `collect_constants`, but generalized to `ConstantKind` values, not just
     `u128` — this is exactly the data the paused `PI` plan's injected
     `VarDecl` would already satisfy, since it's `qualifier: Constant` with a
     literal `RealLiteral` initializer).
   - Walk each `SimpleExpr`'s `Expr` tree, substitute any `Variable`/
     `LateBound` node referencing a known constant with `ExprKind::Const(...)`.
   - Fold the resulting expression using the **existing**
     `fold_real_binary`/`fold_integer_binary` logic already in
     `xform_fold_constant_expressions.rs` (confirmed these already handle
     `+`/`-`/`*`/`/` for both integer and real literals, including nested
     binary expressions — this part needs no new arithmetic logic, only
     reuse/extraction into a shared function both passes can call).
   - If fully folded to `ExprKind::Const(c)`: replace with
     `InitialValueAssignmentKind::Simple(SimpleInitializer { type_name, initial_value: Some(c) })`
     — the pre-existing shape, so all 21 downstream consumers see exactly
     what they see today and need zero changes.
   - If **not** fully folded (references a non-constant variable, or
     `allow_constant_initializer_expressions` is off and any `SimpleExpr`
     survived at all): emit a new problem code (P-code TBD — e.g.
     "InitializerNotConstantExpression").

4. **Dialect flag**: `allow_constant_initializer_expressions`, `[Rusty, Codesys]`
   — same placement/reasoning as `allow_constant_type_params` (this is the
   initializer-position sibling of that exact same vendor behavior).

## Non-goals

- No runtime-evaluated initializers (rejected by ADR-0024 above; would need
  a different ADR-level decision to revisit).
- No function calls in constant expressions (e.g. `ATAN(x) + PI` is real
  usage, but only ever seen in *statement* context in the survey, never as
  a `VAR` initializer — no evidence this is needed there; scope to
  literal/named-constant arithmetic only, matching what's actually blocked).
- No changes to the `SimpleInitializer`/`ConstantKind` type itself — the
  whole point of the design is to avoid that ripple.
- Does not itself register `PI` — that's the companion, still-paused
  `2026-07-19-twincat-pi-constant.md` plan. The two are independent pieces
  that combine to fully unblock the real-world pattern; either can land
  first, but neither alone fixes `d2r : LREAL := PI/180.0;`.

## File Map

| File | Change |
|------|--------|
| `compiler/dsl/src/common.rs` | New `InitialValueAssignmentKind::SimpleExpr` variant + `SimpleExprInitializer` struct |
| `compiler/dsl/src/fold.rs`, `compiler/dsl/src/visitor.rs` | Dispatch boilerplate for the new variant (same pattern as `InterfaceDeclaration`) |
| `compiler/parser/src/parser.rs` | Fall back to `expression()` after `constant()` fails, in each initializer production |
| `compiler/analyzer/src/xform_fold_constant_expressions.rs` | Extract `fold_real_binary`/`fold_integer_binary` (and the fold-driving logic) into a reusable form the new pass can call |
| `compiler/analyzer/src/xform_fold_initializer_expressions.rs` (new) | Collect constants, substitute references, fold, normalize back to `Simple`, or diagnose |
| `compiler/analyzer/src/stages.rs` | Wire the new pass into `resolve_types`, before anything else touches initializers |
| `compiler/parser/src/options.rs` | New `allow_constant_initializer_expressions` flag |
| `compiler/problems/resources/problem-codes.csv` + new `docs/reference/compiler/problems/P####.rst` | New problem code for "initializer must fold to a constant" |
| `compiler/ironplc-cli/src/lsp.rs` | LSP flag wiring |
| Docs (3 files, same as every prior PR) | Document the new flag |

## Testing Strategy

- Parser tests: each of the 5 probe cases above, both with and without the
  flag context mattering (grammar itself is unconditional; only the fold
  pass's diagnostic depends on the flag).
- Fold-pass unit tests: named-constant substitution + arithmetic folding for
  `PI/180.0`-shaped expressions (integer and real), nested expressions,
  and the negative case (reference to a non-constant variable → diagnostic).
- Regression: every existing `SimpleInitializer`-consuming test (21 files'
  worth) must be completely unaffected — the normalization means they should
  need zero changes; if any do need changes, that's a signal the "normalize
  away early" design isn't fully closing the gap.
- End-to-end: compile-and-run a program using `d2r : LREAL := PI/180.0;`
  (depends on the `PI` plan landing too) and verify the computed value.

## Tasks

- [x] Write plan (this document)
- [ ] Extract shared fold logic from `xform_fold_constant_expressions.rs`
- [ ] New `InitialValueAssignmentKind::SimpleExpr` variant + dispatch boilerplate
- [ ] Grammar: `constant()` then fall back to `expression()` in initializer productions
- [ ] New `xform_fold_initializer_expressions.rs` pass
- [ ] New `allow_constant_initializer_expressions` flag
- [ ] New problem code + doc page
- [ ] Wire into `stages.rs`, `lsp.rs`
- [ ] All tests from Testing Strategy
- [ ] Revisit the paused `PI` plan together with this one (they combine to
      fully unblock the real-world pattern)
- [ ] Update docs
- [ ] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork (no PR against `ironplc/ironplc` without explicit
      go-ahead, per standing instruction)
