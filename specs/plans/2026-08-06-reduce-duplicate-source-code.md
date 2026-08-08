# Plan: Reduce duplicate source code in the compiler

## Context

`cargo-dupes` reports substantial structural duplication in the compiler
workspace. Running with `--exclude-tests` over `compiler/`:

```
Exact duplicates: 265 groups (946 code units), 6528 lines (13.6%)
Near duplicates:   44 groups  (92 code units), 1442 lines  (3.0%)
```

`--exclude-tests` only strips `#[test]` / `#[cfg(test)]`. Two large slices of
the reported total are still test code and are **out of scope** for this plan:

- **Integration tests** under `*/tests/it/**` and `benchmarks/benches/**` —
  separate binaries whose helpers are not marked `#[test]` (e.g.
  `codegen/tests/it/common/mod.rs`, `vm/tests/it/common/mod.rs`,
  `st_benchmark.rs`).
- **Spec-conformance suites** `*/src/spec_conformance.rs` — these use
  `#[spec_test(...)]`, which expands to `#[test]`. They are the single largest
  reported group (`enum_spec_req_en_002_ordinal_is_runtime_value`, 12 members)
  but are intentionally repetitive test fixtures.

Filtering those out leaves **~216 groups / ~2,960 duplicated lines of genuine
source**. Distribution by crate:

| Crate        | Dup lines |
|--------------|-----------|
| analyzer     | ~864 |
| codegen      | ~632 |
| dsl          | ~411 |
| ironplc-cli  | ~271 |
| vm           | ~153 |
| sources      | ~144 |
| container    | ~107 |
| (others)     | ~380 |

This plan groups that duplication into **actionable refactors** (a shared
abstraction genuinely removes the repetition) and **structural coincidences**
(same AST shape, different meaning — collapsing them would *reduce* clarity, so
we leave them alone and, where useful, add them to the `cargo-dupes` ignore
list).

## Guiding principle

Only refactor where a single abstraction removes repeated **logic**. Where many
functions share a shape only because Rust has no shorter way to write a
one-line delegation or a distinct-typed builder method, a macro trades
readability for a lower dupe score — a bad trade. Each item below states the
mechanism, the scope, the approximate dup-line reduction, and the risk.

---

## Actionable refactors (in priority order)

### 1. Analyzer rule `apply` boilerplate — trait + helper

**~130 dup lines · low risk · highest confidence**

18 of the 28 `analyzer/src/rule_*.rs` files repeat the same `apply` body: build
a visitor holding `diagnostics: Vec<Diagnostic>`, `walk(lib)`, then convert to
`SemanticResult`:

```rust
pub fn apply(lib: &Library, _context: &SemanticContext, _options: &CompilerOptions) -> SemanticResult {
    let mut visitor = RuleX { diagnostics: Vec::new() };
    visitor.walk(lib).map_err(|e| vec![e])?;
    if !visitor.diagnostics.is_empty() {
        return Err(visitor.diagnostics);
    }
    Ok(())
}
```

The rules are dispatched in `analyzer/src/stages.rs` as bare `rule_x::apply`
function pointers, so the free-function signature is the contract and must stay.

**Mechanism.** Add to a shared module (e.g. `analyzer/src/rule_support.rs`):

```rust
pub(crate) trait DiagnosticVisitor: Visitor<Diagnostic, Value = ()> {
    fn into_diagnostics(self) -> Vec<Diagnostic>;
}

pub(crate) fn run_rule<V: DiagnosticVisitor>(mut visitor: V, lib: &Library) -> SemanticResult {
    visitor.walk(lib).map_err(|e| vec![e])?;
    let diagnostics = visitor.into_diagnostics();
    if diagnostics.is_empty() { Ok(()) } else { Err(diagnostics) }
}
```

Each rule's `apply` collapses to `run_rule(RuleX::default(), lib)` (deriving
`Default` where the visitor is a plain `diagnostics` holder). Rules that name
the field `problems` (e.g. `rule_pou_hierarchy`) rename to `diagnostics` for
uniformity or implement `into_diagnostics` accordingly.

**Risk.** Low — behavior-preserving, each rule still owns its `visit_*` logic.
Covered by existing per-rule tests.

### 2. `Located::span` delegations — derive macro

**~140 dup lines · medium risk (macro authoring) · high count**

The largest genuine-source group (49 members) is the three-line `Located::span`
impl repeated across `dsl/src/textual.rs` (18×), `dsl/src/common.rs`, and
others. Every impl is one of two shapes:

```rust
impl Located for FbCall    { fn span(&self) -> SourceSpan { self.position.clone() } }
impl Located for NamedVariable { fn span(&self) -> SourceSpan { self.name.span() } }
```

**Mechanism.** The crate already ships a proc-macro crate (`dsl_macro_derive`,
which provides `#[derive(Recurse)]`). Add `#[derive(Located)]` supporting two
field attributes:

- `#[located(position)]` on a `SourceSpan` field → `self.<field>.clone()`
- `#[located(delegate)]` on a sub-node field → `self.<field>.span()`

Default (no attribute): use a field named `position` if present. Then replace
the hand-written impls with the derive on each struct.

**Risk.** Medium — proc-macro code plus attribute plumbing. Mitigated by the
fact that the generated code is trivial and every affected type already has
span coverage through parser/analyzer tests. Convert incrementally (derive one
type, keep the rest hand-written) so the change is reviewable.

### 3. `container` id newtypes — declarative macro

**~85 dup lines · low risk**

`container/src/id_types.rs` defines 9 `u16` newtypes (`FunctionId`, `TaskId`,
`VariableId`, …) each repeating identical `new`, `raw`, `to_le_bytes`, and
`Display` bodies (two separate dupe groups, `to_le_bytes` ×10 and `Display`
×10).

**Mechanism.** A `macro_rules! u16_id_type!` that expands the struct, the four
methods, and the `Display` impl. Per-type constants (`FunctionId::INIT = 0`,
`TaskId::DEFAULT = 0`, …) are passed as an optional `const` list or kept as a
separate `impl` block next to the invocation.

```rust
u16_id_type!(FunctionId);
impl FunctionId { pub const INIT: Self = Self(0); /* … */ }
```

**Risk.** Low — pure boilerplate, no logic. `container` round-trip tests cover
encoding.

### 4. codegen `emit_<binop>` dispatch — declarative macro

**~80 dup lines · low risk**

`codegen/src/compile_expr.rs` has 14 `emit_add` / `emit_sub` / `emit_mul` /
`emit_div` / `emit_and` / … functions. Each is a `match op_type { … }` mapping
the `(OpWidth, Signedness)` to the corresponding `Emitter::emit_<op>_<ty>`
method. Two shapes exist: width-only (add/sub/mul/logical) and width+sign
(div/mod and comparisons).

**Mechanism.** Two `macro_rules!` arms — `emit_binop_width!(add)` and
`emit_binop_signed!(div)` — each generating the function from the op stem by
pasting `emit_<op>_i32` etc. Keep the functions `pub(crate)` so `compile_call`
and friends still import them unchanged.

**Risk.** Low — mechanical, and the emitter methods it references already exist.
VM end-to-end arithmetic tests exercise every branch.

### 5. `SymbolEnvironment` global+scoped iteration — helper

**~55 dup lines · low–medium risk**

`analyzer/src/symbol_environment.rs` repeats a "scan `global_symbols`, then scan
every scope in `scoped_symbols`, applying the same predicate" walk in several
methods (`get_enumeration_values_for_type`, `insert_enumeration_value`, and
neighbors — two 25–29 line near-duplicate groups).

**Mechanism.** Add a private `fn all_symbols(&self) -> impl Iterator<Item = (&Id,
&SymbolInfo)>` (chaining global with the flattened scoped map) and express the
callers as `.filter(...)` over it. Mutating callers get a small
`for_each_symbol_mut`-style helper or keep their explicit loop if borrow-checker
friction outweighs the win.

**Risk.** Low–medium — iterator lifetimes need care; behavior is covered by
enumeration-resolution tests.

### 6. codegen builtin lookup tables — optional macro

**~110 dup lines · medium risk · optional**

`compile_call.rs` reports ~18 near-identical 4-line "closures" that are really
`match` arms in `lookup_builtin`: `"SIN" => match op_width { F32 => SIN_F32,
F64 => SIN_F64, _ => None }`. These are a lookup table written longhand.

**Mechanism.** A `float_builtin!("SIN", SIN)` macro arm generating the match
arm, applied to the transcendental/float family.

**Risk / recommendation.** Medium. This is a table, not logic — the longhand is
arguably more greppable than a macro. **Recommend deferring** unless the file
approaches the 1000-line module limit; if kept as-is, add its fingerprints to
the ignore list (below) so it stops dominating the report.

---

## Structural coincidences — leave as-is (and ignore)

These groups share an AST shape but not meaning; a shared abstraction would hurt
readability. Add their `cargo-dupes` fingerprints to the ignore file so future
runs highlight *new* duplication instead of these.

- **`ContainerBuilder::add_*` methods** (`container/src/builder.rs`, 11×) — each
  pushes to a *different* typed vector and returns a distinct id. This is a
  deliberate, readable builder API; a macro obscures the per-field types. Same
  reasoning covers the `with_secondary`-shaped group.
- **Three-line delegating getters** — `UriKey::as_str` (25×),
  `LspProject::semantic_context` (part of the 49-group), and similar
  one-line accessors across `ironplc-cli` and `vm`. Semantically distinct;
  not worth a macro. (The `Located::span` subset is the exception — item 2.)
- **`xform_*` `Fold` triples** — the `fold_function_declaration` /
  `fold_function_block_declaration` / `fold_program_declaration` trios repeated
  within each transform. These mirror the three POU kinds and are constrained by
  the `Fold` trait shape; collapsing them fights the visitor design. Revisit
  only if the `Fold` trait itself is redesigned.
- **Per-rule `Visitor::visit_*` bodies** (e.g. `rule_string_encoding_compat`,
  `rule_bit_access_range`) — the diagnostic-construction shape recurs but the
  conditions differ per rule; item 1 already removes the surrounding `apply`
  boilerplate, which is the mechanical part.

## Sequencing & validation

Land as **independent PRs**, each self-contained and green before the next:

1. Item 1 (analyzer `apply`) — largest low-risk win, no new macro machinery.
2. Item 3 (id newtypes) and Item 4 (emit dispatch) — small declarative macros,
   independent crates.
3. Item 2 (`Located` derive) — proc-macro work; convert types incrementally.
4. Item 5 (symbol iteration).
5. Item 6 only if justified; otherwise ignore-list the builtin table.

Per PR:

- `cd compiler && just` must pass (compile, coverage ≥ 85%, clippy, fmt).
- Re-run `cargo-dupes stats --exclude-tests` and record the delta in the PR
  body. Target: bring exact duplication from 13.6% toward < 9% after items 1–5
  (est. ~490 source dup lines removed).
- No behavior change is intended in any item; existing unit, spec-conformance,
  and end-to-end tests are the safety net. Add tests only for the new derive
  macro (item 2) and the new declarative macros (items 3–4) covering each
  generated shape.

## Estimated impact

| Item | Mechanism | ~Dup lines removed | Risk |
|------|-----------|--------------------|------|
| 1 Analyzer `apply` | trait + `run_rule` helper | 130 | low |
| 2 `Located::span` | `#[derive(Located)]` | 140 | medium |
| 3 id newtypes | `macro_rules!` | 85 | low |
| 4 emit dispatch | `macro_rules!` | 80 | low |
| 5 symbol iteration | iterator helper | 55 | low–med |
| 6 builtin table | macro (optional) | 110 | medium |
| **Total (1–5)** | | **~490** | |
| **With 6** | | **~600** | |
