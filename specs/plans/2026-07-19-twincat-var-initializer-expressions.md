# Plan: Constant Expressions in VAR Initializers

## Revision: un-stacked from the PI-constant PR

garretfick (maintainer) asked that PRs not be stacked on each other, and
separately wants a different approach for the "library support" PRs
(PI, LTRUNC/LMOD, MODABS). This branch no longer includes or depends on
the PI-constant feature at all — every example below and in the tests
uses a user-declared `CONSTANT` (e.g. `SCALE`) instead of the built-in
`PI` this plan was originally motivated by and tested against. The
narrative below is left as-is since it's an accurate account of how the
feature was discovered and designed (via real `PI`-using TwinCAT code),
but "PI" in code examples elsewhere in this branch's diff refers to a
plain user-declared constant, not the separate, still-unmerged
`--allow-math-constants` feature.

**Status: implemented and landed on this branch.** `scaled : LREAL :=
SCALE*4.0;` now compiles and runs correctly under
`--allow-constant-initializer-expressions`. The core design below (new
`SimpleExpr` placeholder variant, normalize-away-early via a dedicated fold
pass) was implemented largely as planned, with two significant discoveries
made during implementation that the plan did not anticipate — see
"Implementation Notes" at the end of this file:

1. The planned grammar approach ("try `constant()` first, fall back to
   `expression()`") does not work — `constant()` greedily matches a
   *prefix* of the input and succeeds without consuming trailing operators,
   so PEG's ordered choice never reaches the fallback for inputs like
   `3.14/180.0`. Fixed by using `expression()` unconditionally and
   dispatching on the *shape* of the parsed result instead.
2. That fix initially broke every negative-literal initializer in the
   codebase (e.g. `x : INT := -123;`) because `expression()` routes a
   leading `-` through its own unary-operator handling rather than
   `constant()`'s built-in signed-literal parsing, changing the AST shape
   from `Const` to `UnaryOp(Neg, Const)`. Fixed by collapsing that specific
   shape back to `Const` (unconditionally, not gated by the new flag, since
   it isn't new capability) directly in the parser.

Both fixes, the reasoning behind them, and the enum-disambiguation subtlety
in `simple_or_enumerated_or_subrange_ambiguous_struct_spec_init()` are
detailed in "Implementation Notes".

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
- [x] Extract shared fold logic from `xform_fold_constant_expressions.rs`
- [x] New `InitialValueAssignmentKind::SimpleExpr` variant + dispatch boilerplate
- [x] Grammar: `expression()`-based initializer parsing with shape-based
      dispatch (see Implementation Notes for why "constant() then fall
      back" doesn't work)
- [x] New `xform_fold_initializer_expressions.rs` pass
- [x] New `allow_constant_initializer_expressions` flag
- [x] New problem codes (P4036, P4037) + doc pages
- [x] Wire into `stages.rs`, `lsp.rs`
- [x] All tests from Testing Strategy, plus regression tests for the two
      discoveries (negative-literal collapse, enum-initializer
      disambiguation)
- [x] Un-stacked from PI (`2026-07-19-twincat-pi-constant.md`, still a
      separate open PR) per maintainer request -- tests use a
      user-declared `CONSTANT` instead of the built-in `PI` global
- [x] Update docs
- [x] Run full CI pipeline (`cd compiler && just`)
- [x] Push branch to fork (no PR against `ironplc/ironplc` without explicit
      go-ahead, per standing instruction)

## Implementation Notes

- **"`constant()` first, `expression()` fallback" doesn't work — discovered
  via a real end-to-end test, not by inspection.** The plan's original
  grammar design assumed ordered choice would naturally try `constant()`
  first and fall back to `expression()` on failure. This is wrong: for
  input like `3.14/180.0`, `c:constant()` doesn't fail — it *succeeds*,
  matching just `3.14` and stopping (constant() has no way to know an
  operator follows). Since the alternative as a whole "succeeded" by PEG's
  rules, the parser never backtracks to try the `expression()` fallback;
  instead the surrounding rule (expecting `;` next) fails on the
  leftover `/180.0`. Fixed by dropping the `constant()`-based alternative
  entirely and always parsing via `expression()` (which includes `constant()`
  as one of its own sub-alternatives, so bare literals parse identically),
  then dispatching on the *resulting AST shape*: `ExprKind::Const(c)` →
  `Simple`, anything else → `SimpleExpr`. This is a more general instance of
  a rule worth remembering: in `peg`/PEG generally, "try A, then B" only
  backtracks to B if A *fails outright* — it never backtracks because A
  succeeded but left something unparsed that a surrounding rule later
  rejects.
- **Negative-literal initializers broke everywhere, caught by the full
  workspace test suite, not by targeted testing.** Switching to
  `expression()` changed how `x : INT := -123;` parses: `expression()`'s
  `unary_expression()` rule greedily consumes a leading `-` as a unary
  operator *before* trying to parse the operand as a `constant()`, so the
  literal itself parses as non-negative and gets wrapped in
  `ExprKind::UnaryOp(Neg, Const(123))` — a different shape than plain
  `constant()` parsing (`Const(-123)` directly, since `constant()`'s own
  `integer_literal()`/`real_literal()`/`duration()` productions each handle
  a leading sign internally). Every negative-literal `VAR` initializer in
  the entire codebase (4 codegen tests: `INT_TO_STRING`, `DINT_TO_STRING`,
  `SINT_TO_STRING`, `REAL_TO_STRING` with negative inputs) silently started
  requiring `--allow-constant-initializer-expressions` and producing `0`
  instead of the intended value when the flag was off. Fixed with a
  targeted `negate_literal_constant()` helper in `parser.rs` that collapses
  `UnaryOp(Neg, Const(c))` back to a directly-negated `Const` for the 3
  literal kinds `constant()` itself supports with a sign
  (`IntegerLiteral`/`RealLiteral`/`Duration`), applied unconditionally (not
  gated by the new flag, since this isn't new capability — it's the same
  literal syntax reached via a different code path). This ran clean on the
  *targeted* new tests but broke 4 *unrelated, pre-existing* tests — a
  reminder that a grammar change touching a shared production needs the
  full workspace suite run, not just the tests for the new feature.
- **Enum-initializer disambiguation required a fallible (`{? }`) grammar
  action, not a plain ordered-choice fallback.**
  `simple_or_enumerated_or_subrange_ambiguous_struct_spec_init()` already
  had a documented ambiguity: `identifier := identifier` could mean "simple
  type with a value" or "enum type with an enum default", resolved by
  trying the simple/constant interpretation first and falling through to
  the enum interpretation when `constant()` fails on the bare identifier.
  Switching to `expression()` breaks this the same way as the two points
  above: `expression()` *does* successfully parse a bare identifier (as
  `ExprKind::Variable`/`LateBound`), so naively taking "not a `Const`" as
  "must be `SimpleExpr`" would swallow every enum-default declaration in
  the codebase. Fixed by making the grammar action fallible (`{? ... }`,
  returning `Result<T, &str>`) and explicitly rejecting (`Err(...)`) when
  the parsed expression is a bare `Variable`/`LateBound` with no operators
  — this makes PEG backtrack fully (undoing the `simple_specification()`
  match too) and try the `enumerated_specification()` alternative next,
  exactly reproducing the pre-existing disambiguation. `simple_spec_init()`
  (used for `AT`-located variables, which has no sibling enum alternative
  to defer to) does not need this guard and uses a plain infallible
  dispatch.
- **The "21-file blast radius" from the plan turned out to be 6 files, not
  21.** The grep-based estimate counted every file referencing
  `SimpleInitializer`/`Simple(...)` at all; `cargo build` after adding the
  enum variant showed only 6 actual non-exhaustive-match compile errors
  (`rule_var_decl_const_initialized.rs`, `xform_resolve_expr_types.rs`,
  `xform_resolve_late_bound_expr_kind.rs`,
  `xform_resolve_type_decl_environment.rs`, `xform_toposort_declarations.rs`,
  `intermediates/structure.rs`) — most of the 21 files use `if let`/match
  guards on `Simple` specifically rather than exhaustively matching the
  whole enum, so they were unaffected by the new variant. Reconfirms the
  existing lesson (also noted in `twincat-status.md`) that `cargo build`
  is the authoritative way to find the real blast radius, not grepping.
- **`substitute_and_fold` handles both `Variable` and `LateBound` node
  shapes**, not just one. The pass is wired to run after
  `xform_resolve_late_bound_expr_kind` in the normal pipeline (so
  references are already `Variable` by the time it runs), but the unit
  tests in this module call `apply()` directly without running that
  earlier pass first, leaving references as `LateBound` — handling both
  shapes makes the pass correct regardless of invocation context, at
  negligible cost.
- **`InitialValueAssignmentKind::SimpleExpr` reaches struct field
  initializers too**, not just `VAR` declarations — both
  `var1_init_decl__with_ambiguous_struct()` (used by `VAR`/`VAR_INPUT`/etc.)
  and `structure_element_declaration()` (`STRUCT ... field := expr; END_STRUCT`)
  route through the same `simple_or_enumerated_or_subrange_ambiguous_struct_spec_init()`
  production, so the constant-expression capability (and the
  `intermediates/structure.rs` fix for `field_has_default()`) apply to
  struct fields for free. Not separately tested (out of scope per the
  plan's non-goals) but worth knowing if a bug report mentions struct
  fields specifically.
