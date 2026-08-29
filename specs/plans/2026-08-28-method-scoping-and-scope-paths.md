# Plan: Traversal-driven scopes and scope paths

Fixes [#1439](https://github.com/ironplc/ironplc/issues/1439).

Delivered as eight slices, one pull request each. Every slice builds,
passes `cd compiler && just`, and is independently releasable — no slice
leaves the compiler in a state where `check` and `compile` disagree.
Slices 1-6 close #1439; slices 7-8 are the symbol-environment half and
can be deferred without reopening it.

## Problem

Five passes each maintain their own idea of what is in scope, and none of
them knows that a `METHOD` opens one. `MethodDeclaration` falls through to
the `dispatch!` default in `compiler/dsl/src/visitor.rs:275`, which
recurses without entering a scope.

Verified against `634c868` with
`ironplcc check --dialect iec61131-3-ed3`:

1. **A method cannot produce its return value.** `GetSpeed := speed;`
   inside `METHOD GetSpeed : REAL` reports P4007. The identical shape in a
   `FUNCTION` body is accepted. (#1439 defect 1)

2. **Method locals leak into sibling methods.** A second method
   referencing the first method's `VAR_INPUT` parameter is accepted.
   Every method's declarations land in the function block's scope.
   (#1439 defect 2)

3. **Method bodies are not type-checked at all.** With
   `b : BOOL; i : INT`, the assignment `b := i` inside a method exits 0;
   the same two lines in the function block body report P4035.
   `ExprTypeResolver` never inserts a method's variables, so every
   method-local reference resolves to `None` and the type rules skip
   silently. Not in the issue.

4. **Codegen leaks the same way, and miscompiles rather than
   diagnosing.** `compile_user_method`
   (`compiler/codegen/src/compile_method.rs:111`, `:128`) inserts each
   method's parameters and locals into `ctx.variables` with no
   save/restore — `compile_user_function` does exactly that at
   `compile_fn.rs:78` and `:467`. Method B still sees method A's names
   bound to A's `VarIndex`, which may sit outside B's frame bounds
   window. Reachable today precisely because defect 2 lets it past the
   analyzer. Not in the issue.

5. **A method-local `VAR CONSTANT` leaks into sibling methods.**
   `xform_fold_initializer_expressions` enters a scope for function
   (`:292`), program (`:303`) and function block (`:314`), and not for a
   method. Not in the issue.

One correction to the issue's "Downstream" note: codegen no longer has to
learn the method return slot. `compile_method.rs:138-197` already
allocates it, sets `current_function_return`, and loads it before `RET`
(landed in a860646). The only missing piece is binding the *name* to that
slot — one line, mirroring `compile_fn.rs:258`.

Nothing here was implemented incorrectly. `dispatch!(MethodDeclaration)`
was added, the ~37 passes that do not care about scope kept working
correctly, and the five that do kept compiling while silently doing the
wrong thing. **The defect is omission at a call site.** Fixing five call
sites leaves the sixth to be discovered the same way, so the fix has to
make the omission impossible instead.

## Design

### The traversal opens scopes, not the passes

Today each pass overrides `visit_function_declaration`,
`visit_program_declaration` and `visit_function_block_declaration` and
does its own `enter()`/`exit()`. That is opt-in: a pass that does not
override a node kind is silently unscoped for it, which is exactly the
failure that produced this bug. Invert it. `Visitor` and `Fold` each gain

```rust
fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), E> { Ok(()) }
fn exit_scope(&mut self) {}
```

and the `Recurse` derive, for a struct carrying `#[recurse(scope)]`,
wraps the generated recursion:

```rust
pub fn recurse_visit<V: Visitor<E> + ?Sized, E>(&self, v: &mut V) -> Result<V::Value, E> {
    v.enter_scope(self.as_scope_node())?;
    let result = self.recurse_visit_inner(v);
    v.exit_scope();
    result
}
```

`recurse_fold` gets the same shape. `as_scope_node()` borrows `self`
before the fold consumes it, and the borrow ends when `enter_scope`
returns, so the by-value fold signature still works.

Three properties follow:

- A pass implements one enter/exit pair instead of one override per node
  kind. `rule_use_declared_symbolic_var`'s three near-identical overrides
  become a single pair.
- Annotating a new scope-bearing node scopes it in *every* pass at once.
- Enter and exit are generated as a matched pair around the recursion,
  including the `?` paths, so no pass can leak a scope on an error
  return. `rule_use_declared_symbolic_var` is correct today only because
  someone remembered to bind `let ret = …;` rather than writing `?`
  before `exit()`.

The ~37 passes that do not care about scope take the no-op defaults and
are untouched.

**Invariant to document:** enter/exit fire if and only if the traversal
recurses. A pass that overrides `visit_method_declaration` and returns
without calling `recurse_visit` opens no scope, which is the correct
behaviour — it visited nothing.

**Constraint on the signature:** `enter_scope` receives a borrow it may
not retain. A pass copies what it needs (the name, the declarations) into
its own table during the call. `exit_scope` takes no argument and returns
no `Result`: it runs on the error path, and a pass that needs to know
what it is closing already has its own stack.

### `ScopeNode` is an exhaustive enum

```rust
pub enum ScopeNode<'a> {
    Function(&'a FunctionDeclaration),
    FunctionBlock(&'a FunctionBlockDeclaration),
    Program(&'a ProgramDeclaration),
    Method(&'a MethodDeclaration),
}
```

Passes match on it without a wildcard arm, so adding a variant is a build
error in every pass that discriminates by kind. That matters because the
kinds genuinely differ: a function block seeds its own name plus its
`EXTENDS`-inherited fields, a function and a program seed their own name
unconditionally, and a method seeds its name **only when `return_type` is
`Some`** — a method with no return type has no result to assign, so
`MethodName := …` in that body must keep reporting P4007.

The corresponding `ScopeBearing` trait (`fn as_scope_node(&self) ->
ScopeNode<'_>`) is the content half — four hand-written impls. It is
deduplication, not enforcement; the enforcement is the enum's
exhaustiveness and the derive guard below. Writing `#[recurse(scope)]`
without the impl does not compile.

### Closing the "forgot the attribute" hole

Exactly four AST structs have a `variables: Vec<VarDecl>` field —
`FunctionDeclaration` (`common.rs:2734`), `FunctionBlockDeclaration`
(`:2755`), `MethodDeclaration` (`:2792`), `ProgramDeclaration` (`:2915`).
The `Recurse` derive already inspects every field, so it can refuse to
compile a struct that has that field and carries neither
`#[recurse(scope)]` nor an explicit `#[recurse(no_scope)]`. Adding the
next POU-like construct without deciding which it is then becomes a build
error at the declaration, three lines above the struct it governs.

This is the one hole that neither the enum nor the derive-generated calls
could close on their own, and it is worth the twenty lines in the macro.

`ResourceDeclaration.global_vars` and `ConfigurationDeclaration.global_var`
implement `HasVariables` under different field names, so the guard does
not reach them — see *Out of scope*.

### Scope paths in `SymbolEnvironment`

`ScopeKind` is `Global | Named(Id)` — one level, no nesting — and its
builder tracks `scope: Option<Id>`
(`xform_resolve_symbol_and_function_environment.rs:63`), so it cannot
represent a method scope at all. Replace the single `Id` with the chain
of declaration names from the library root:

```rust
pub struct ScopePath(Vec<Id>);        // ⟨FB_Motor, GetSpeed⟩

pub enum ScopeKind {
    Global,
    Named(ScopePath),                 // never empty
}

impl ScopeKind {
    /// The enclosing scope: pops the last segment, `Global` at depth 1.
    pub fn parent(&self) -> Option<ScopeKind>;
}
```

`find` walks the chain — method, then function block, then global —
instead of today's scope-then-global hop. `Id` already implements `Hash`
and `Eq` (case-insensitively, on `lower_case`), so `ScopePath` derives
both and remains usable as a `HashMap` key.

No interning. The parent walk allocates, but symbol lookup is not hot on
PLC-sized inputs, and a `ScopeId` interner is a mechanical follow-up if
profiling ever disagrees.

**Why a path and not a `NodeId`.** Scopes are nameable, so a path *is*
stable identity and costs no AST change. Node ids would be needed for a
`reference → declaration` resolution map — go-to-definition,
find-references, rename — which is a real feature and not this bug. The
groundwork is cheap when we want it (`SourceSpan`'s trivial `PartialEq`
at `core.rs:158` already establishes how to keep an identity field out of
AST equality, and `xform_assign_file_id` establishes the post-parse
stamping pass), so nothing here forecloses it.

### Behaviour change

Source that compiles today starts erroring: the sibling-method leak
(slice 4), the method-local constant leak (slice 5), and type errors
inside method bodies that were previously invisible (slice 6). That is
the point of the change. No `.st` corpus file uses `METHOD`, and the
existing METHOD tests are parser- and plc2plc-level, so the blast radius
is small.

## Slices

| # | Slice | Crates | Behaviour |
|---|---|---|---|
| 1 | Scope hooks in the traversal | `dsl`, `dsl_macro_derive` | none |
| 2 | Derive guard for scope-bearing structs | `dsl_macro_derive` | none |
| 3 | Codegen isolates method variables and binds the result name | `codegen` | fixes a latent miscompile |
| 4 | `rule_use_declared_symbolic_var` on the hooks | `analyzer`, docs | **closes #1439** |
| 5 | `xform_fold_initializer_expressions` on the hooks | `analyzer` | method-local constants stop leaking |
| 6 | `xform_resolve_expr_types` on `ScopedTable` | `analyzer` | method bodies get type-checked |
| 7 | `ScopePath` data model | `analyzer` | none |
| 8 | `EnvironmentResolver` pushes method scopes | `analyzer`, `mcp` | method symbols scoped for LSP/MCP |

**Why 3 precedes 4.** `ctx.var_index` (`compile.rs:1230`) reports
`Problem::VariableUndefined` for a name it cannot resolve. If slice 4
landed first, there would be a release where `check` accepts
`GetSpeed := speed` and `compile` rejects it with the same P4007 the
analyzer just stopped reporting. Binding the name in codegen first
removes that window.

Slices 5 and 6 are independent of each other and of 4; they may land in
any order after 1. Slices 7-8 depend only on 1.

---

### Slice 1 — Scope hooks in the traversal

*No behaviour change. Nothing implements the hooks yet.*

- `compiler/dsl/src/scope.rs` (new) — `ScopeNode`, the `ScopeBearing`
  trait, and the four impls. `MethodDeclaration::as_scope_node` documents
  the `return_type.is_some()` rule, though each consuming pass makes the
  decision.
- `compiler/dsl/src/visitor.rs`, `compiler/dsl/src/fold.rs` — add
  `enter_scope`/`exit_scope` with no-op defaults to both traits.
- `compiler/dsl_macro_derive/src/lib.rs` — parse `scope` in the `recurse`
  attribute. In `expand_struct_recurse_visit` and
  `expand_struct_recurse_fold`, move the existing body into a private
  `*_inner` and emit the enter/exit wrapper when `scope` is set.
- `compiler/dsl/src/common.rs` — `#[recurse(scope)]` on the four POU
  structs.

Tests, in `dsl` beside the existing `Descender` visitor
(`visitor.rs:456`): a recording visitor and a recording folder that
capture enter/exit events and assert (a) the order and nesting for a
function block containing two methods, (b) that depth returns to zero,
and (c) that an error raised inside a body still fires `exit_scope`.
These also keep the new code covered — no other slice exercises it.

### Slice 2 — Derive guard for scope-bearing structs

*No behaviour change.*

- `compiler/dsl_macro_derive/src/lib.rs` — a struct with a
  `variables: Vec<VarDecl>` field must carry `#[recurse(scope)]` or
  `#[recurse(no_scope)]`; otherwise emit a `syn::Error` naming the struct
  and both spellings.
- Compile-fail test (`trybuild` or equivalent) proving the rejection and
  the message. **This test is the by-design guarantee** — without it the
  guard can be silently weakened later.

### Slice 3 — Codegen isolates method variables and binds the result name

*Fixes problem 4. No source-visible change yet.*

- `compiler/codegen/src/compile_method.rs` — save and restore
  `ctx.variables` and `ctx.var_types` around each method, as
  `compile_user_function` does at `compile_fn.rs:78`/`:467`; and insert
  `return_id → return_var_index` before `emit_function_local_prologue`,
  mirroring `compile_fn.rs:258`.

Codegen keeps its own map — it binds names to `VarIndex` slots, not to
declarations — so it shares the discipline, not the table.

Test: two methods on one function block each declaring a local of the
same name compile to *distinct* slots. That source is legal both before
and after slice 4, so the test survives the analyzer change; a test built
on the leak itself would not.

### Slice 4 — `rule_use_declared_symbolic_var` on the hooks

*Closes #1439 defects 1 and 2.*

- `compiler/analyzer/src/rule_use_declared_symbolic_var.rs` — replace the
  three `visit_*_declaration` overrides with one `enter_scope` matching
  `ScopeNode` exhaustively (function block also seeds `inherited_fields`;
  method seeds its name only when `return_type` is `Some`) and one
  `exit_scope` calling `self.table.exit()`.
- `compiler/analyzer/src/scoped_table.rs` — debug assertion that the
  stack is back to depth 1 when a walk ends, so a leaked scope fails the
  existing suite rather than surfacing later as a mis-resolution.
- `docs/reference/language/object-orientation/method.rst:104-115` — the
  limitation currently covers both halves in one sentence. Narrow it: a
  method body assigns its own name to set the result value; `x :=
  instance.Method()` is still a syntax error pending #1421. The
  `--allow-fb-inheritance` note at `:33-34` points at the same anchor and
  needs the same narrowing.

Tests in the rule's own module:

- `apply_when_method_assigns_own_name_then_ok`
- `apply_when_method_without_return_type_assigns_own_name_then_error`
- `apply_when_method_references_sibling_method_local_then_error`
- `apply_when_method_references_function_block_field_then_ok`
- `apply_when_method_references_inherited_field_then_ok`
- `apply_when_two_methods_declare_same_local_name_then_ok`

Plus, per the syntax-support guide, a `plc2plc` round-trip in
`compiler/plc2plc/src/tests/methods.rs` and an execution test in
`compiler/codegen/tests/it/end_to_end_methods.rs` for a method that
computes its result via `MethodName := …`. The execution test exercises
*production* only: consuming a method result in an expression stays
blocked on #1421, so the value is observed through a field the method
writes rather than through `v := m.GetSpeed()`.

### Slice 5 — `xform_fold_initializer_expressions` on the hooks

*Fixes problem 5.*

- Replace the three enter/exit pairs at `:292`, `:303`, `:314` with the
  trait pair.
- Test:
  `apply_when_method_local_constant_not_visible_in_sibling_method_then_error`,
  the sibling of the existing
  `apply_when_fb_local_constant_not_visible_in_other_fb_then_error`.

### Slice 6 — `xform_resolve_expr_types` on `ScopedTable`

*Fixes problem 3. The largest analyzer change.*

`ExprTypeResolver` keeps flat `var_types` / `array_element_types` maps
that it `clear()`s at each POU boundary (`:526`, `:543`, `:555`).
Clearing at a method boundary would wipe the enclosing function block's
fields, so the existing idiom cannot be extended to methods — the maps
have to become scoped.

Order within the slice, so review can follow it:

1. Convert both maps to `ScopedTable`, keeping function, function block
   and program behaviour identical. Move globals into the base frame in
   `fold_library`, which lets `seed_implicit_globals` and its per-POU
   re-seeding go away. Existing tests must pass unchanged.
2. The `ScopeNode::Method` arm then falls out of the machinery, which is
   what fixes problem 3.
3. Seed the method's own name at its resolved return type, mirroring the
   function case at `:520-525`, so the assignment slice 4 made legal is
   also type-checked.

Tests:

- `apply_when_method_local_assigned_wrong_type_then_error` — the P4035
  that problem 3 currently swallows
- `apply_when_method_local_shadows_field_then_uses_local_type`
- `apply_when_method_assigns_own_name_then_result_type_resolved`
- `apply_when_method_assigns_own_name_wrong_type_then_error`

### Slice 7 — `ScopePath` data model

*No behaviour change: every path is still depth 1 until slice 8.*

- `compiler/analyzer/src/symbol_environment.rs` — `ScopePath`, the new
  `ScopeKind`, `parent()`, and the chain-walking `find`. Delete
  `resolution_cache` (constructed and cleared, never read) and
  `find_in_scope_hierarchy` (`#[allow(dead_code)]`, subsumed by the new
  `find`).
- Four production call sites construct `ScopeKind::Named` and each names
  a POU scope at depth 1, so each becomes a one-segment path:
  `extractors.rs:220` and `:238`,
  `mcp/src/tools/project_io.rs:100`, `mcp/src/runner.rs:74`. Six further
  sites are in tests.
- Test: `find` reaches an outer scope through two levels of nesting, and
  an inner declaration shadows an outer one of the same name.

### Slice 8 — `EnvironmentResolver` pushes method scopes

*Method symbols become correctly scoped for LSP and MCP consumers.*

- `compiler/analyzer/src/xform_resolve_symbol_and_function_environment.rs`
  — `scope: Option<Id>` becomes a `Vec<Id>` stack driven by
  `enter_scope`/`exit_scope`; `current_scope()` reads it.
- Tests: a method parameter resolves at `⟨FB, Method⟩` and not at
  `⟨FB⟩`; two methods with the same local name keep distinct entries; a
  function block field is still found from inside a method by the parent
  walk.
- Check whether `specs/design/expression-type-resolution.md` needs a
  requirement for method-scoped resolution.

---

## Out of scope

- **#1421, the consumption half.** A method call is reachable only from
  statement position, so `v := m.GetSpeed()` remains a syntax error.
  Both must land before `METHOD … : T` is usable end to end.
- **`CONFIGURATION` and `RESOURCE` scopes.** Both implement
  `HasVariables` (`configuration.rs:41`, `:77`) over differently named
  fields, so neither is caught by the slice 2 guard. Under IEC 61131-3
  §2.7.1 a resource's `VAR_GLOBAL` is visible to the programs on it, so
  they are arguably scopes; `EnvironmentResolver` does not handle them
  today either, and nothing in #1439 depends on it. Recorded here so the
  omission is deliberate rather than rediscovered.
- **Node ids and a resolution map.** See the scope-path rationale.
- **`ScopeId` interning.** Only if profiling asks for it.

No new problem codes. P4007 and P4035 already exist; this change makes
them fire correctly.

## Tasks

- [ ] Commit this plan
- [ ] Slice 1 — scope hooks in the traversal (`dsl`, `dsl_macro_derive`)
- [ ] Slice 2 — derive guard + compile-fail test
- [ ] Slice 3 — codegen method variable isolation and result-name binding
- [ ] Slice 4 — `rule_use_declared_symbolic_var`, balance assertion, docs, round-trip and execution tests
- [ ] Slice 5 — `xform_fold_initializer_expressions`
- [ ] Slice 6 — `xform_resolve_expr_types` on `ScopedTable`
- [ ] Confirm every reproduction in *Problem* now behaves correctly
- [ ] Slice 7 — `ScopePath` data model
- [ ] Slice 8 — `EnvironmentResolver` method scopes

## Verification

`cd compiler && just` on every slice.
