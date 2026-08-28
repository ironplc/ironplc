# Plan: Traversal-driven scopes and scope paths

Fixes [#1439](https://github.com/ironplc/ironplc/issues/1439).

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

Two independent mechanisms. Phase 1 moves scope entry into the traversal
machinery and closes #1439. Phase 2 gives `SymbolEnvironment` a real
nesting model. Phase 2 can be deferred without reopening the bug.

### Phase 1: the traversal opens scopes, not the passes

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

### Phase 1: `ScopeNode` is an exhaustive enum

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
exhaustiveness and the derive below. Writing `#[recurse(scope)]` without
the impl does not compile.

### Phase 1: closing the "forgot the attribute" hole

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

### Phase 2: scope paths in `SymbolEnvironment`

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

While in the file, delete `resolution_cache` (constructed and cleared,
never read) and `find_in_scope_hierarchy` (`#[allow(dead_code)]`, and
subsumed by the new `find`).

### Behaviour change

Source that compiles today starts erroring: the sibling-method leak, and
type errors inside method bodies that were previously invisible. That is
the point of the change. No `.st` corpus file uses `METHOD`, and the
existing METHOD tests are parser- and plc2plc-level, so the blast radius
is small.

## Implementation

### Phase 1

**`compiler/dsl/src/scope.rs`** (new) — `ScopeNode`, the `ScopeBearing`
trait, and the four impls. `MethodDeclaration::as_scope_node` is where
the `return_type.is_some()` rule is documented, though the decision is
made by each consuming pass.

**`compiler/dsl/src/visitor.rs`, `compiler/dsl/src/fold.rs`** — add
`enter_scope`/`exit_scope` with no-op defaults to both traits.

**`compiler/dsl_macro_derive/src/lib.rs`** — parse `scope` and `no_scope`
in the `recurse` attribute. In `expand_struct_recurse_visit` and
`expand_struct_recurse_fold`, move the existing body into a private
`*_inner` and emit the enter/exit wrapper when `scope` is set. Add the
`variables: Vec<VarDecl>` guard, with a `syn::Error` naming the struct
and both attribute spellings.

**`compiler/dsl/src/common.rs`** — `#[recurse(scope)]` on the four POU
structs.

**`compiler/analyzer/src/rule_use_declared_symbolic_var.rs`** — replace
the three `visit_*_declaration` overrides with one `enter_scope` that
matches `ScopeNode` exhaustively (function block also seeds
`inherited_fields`; method seeds its name only when `return_type` is
`Some`) and one `exit_scope` calling `self.table.exit()`. Closes #1439
defects 1 and 2.

**`compiler/analyzer/src/xform_fold_initializer_expressions.rs`** —
replace the three enter/exit pairs at `:292`, `:303`, `:314` with the
trait pair. Closes problem 5.

**`compiler/analyzer/src/xform_resolve_expr_types.rs`** — the substantive
one. `ExprTypeResolver` keeps flat `var_types` / `array_element_types`
maps that it `clear()`s at each POU boundary (`:526`, `:543`, `:555`);
clearing at a method boundary would wipe the enclosing function block's
fields, so the existing idiom cannot be extended to methods. Convert both
maps to `ScopedTable`. `enter_scope` pushes a frame and inserts the
declaration's variables plus, for `Function` and for `Method` with a
return type, the declaration's own name at its resolved return type
(`:520-525` is the existing function case). `exit_scope` pops. Globals
move into the base frame in `fold_library`, which lets
`seed_implicit_globals` and its per-POU re-seeding go away entirely.
Closes problem 3, and gives the newly legal `GetSpeed := speed` a type.

**`compiler/codegen/src/compile_method.rs`** — save and restore
`ctx.variables` and `ctx.var_types` around each method, as
`compile_user_function` does at `compile_fn.rs:78`/`:467`; and insert
`return_id → return_var_index` before `emit_function_local_prologue`,
mirroring `compile_fn.rs:258`, so the assignment the analyzer now accepts
actually compiles. Closes problem 4. Codegen keeps its own map — it binds
names to `VarIndex` slots, not to declarations — so it shares the
discipline, not the table.

### Phase 2

**`compiler/analyzer/src/symbol_environment.rs`** — `ScopePath`, the new
`ScopeKind`, `parent()`, the chain-walking `find`; delete
`resolution_cache` and `find_in_scope_hierarchy`.

**`compiler/analyzer/src/xform_resolve_symbol_and_function_environment.rs`**
— `scope: Option<Id>` becomes a `Vec<Id>` stack driven by
`enter_scope`/`exit_scope`; `current_scope()` reads it. Method parameters
and locals then land in `⟨FB, Method⟩` rather than in the function
block's scope.

**Callers** — four production sites construct `ScopeKind::Named`:
`extractors.rs:220` and `:238`, `mcp/src/tools/project_io.rs:100`,
`mcp/src/runner.rs:74`. Each names a POU scope at depth 1 and becomes a
one-segment path. Six more sites are in tests.

## Tests

Phase 1, `compiler/analyzer/src/rule_use_declared_symbolic_var.rs`:

- `apply_when_method_assigns_own_name_then_ok`
- `apply_when_method_without_return_type_assigns_own_name_then_error`
- `apply_when_method_references_sibling_method_local_then_error`
- `apply_when_method_references_function_block_field_then_ok`
- `apply_when_method_references_inherited_field_then_ok`
- `apply_when_two_methods_declare_same_local_name_then_ok`

`compiler/analyzer/src/xform_resolve_expr_types.rs`:

- `apply_when_method_local_assigned_wrong_type_then_error` — the P4035
  that problem 3 currently swallows
- `apply_when_method_local_shadows_field_then_uses_local_type`
- `apply_when_method_assigns_own_name_then_result_type_resolved`

`compiler/analyzer/src/xform_fold_initializer_expressions.rs`:

- `apply_when_method_local_constant_not_visible_in_sibling_method_then_error`
  — the sibling of the existing
  `apply_when_fb_local_constant_not_visible_in_other_fb_then_error`

`compiler/dsl_macro_derive` — a `trybuild` (or equivalent compile-fail)
case proving a struct with `variables: Vec<VarDecl>` and no scope
attribute fails to compile, and that the message names both spellings.
This test *is* the by-design guarantee; without it the guard can be
silently weakened later.

Scope-balance assertion — `ScopedTable` gains a debug assertion that the
stack is back to depth 1 when a walk ends, so a leaked scope fails the
existing suite rather than surfacing as a mis-resolution.

`compiler/plc2plc/src/tests/methods.rs` — round-trip a method that
assigns its own name, per the syntax-support guide.

`compiler/codegen/tests/it/end_to_end_methods.rs` — compile and run a
function block whose method computes and returns a value via
`MethodName := …`, called from a program, asserting the variable value.
Note this exercises *production* only: consuming a method result in an
expression stays blocked on #1421, so the call site is a statement and
the value is observed through a `VAR_OUTPUT`-style field or a second
method that stores it.

Phase 2 — `xform_resolve_symbol_and_function_environment.rs`: a method
parameter resolves at `⟨FB, Method⟩` and not at `⟨FB⟩`; two methods with
the same local name keep distinct entries; a function block field is
still found from inside a method by the parent walk.

## Documentation

`docs/reference/language/object-orientation/method.rst:104-115` states
the current limitation as a single sentence covering both halves. After
Phase 1 only the consumption half remains: rewrite it to say that a
method body assigns its own name to set the result value, and that
`x := instance.Method()` is still a syntax error pending #1421. The
`--allow-fb-inheritance` note at `:33-34` points at the same anchor and
needs the same narrowing.

No new problem codes. P4007 and P4035 already exist; this change makes
them fire correctly.

## Out of scope

- **#1421, the consumption half.** A method call is reachable only from
  statement position, so `v := m.GetSpeed()` remains a syntax error.
  Both must land before `METHOD … : T` is usable end to end.
- **`CONFIGURATION` and `RESOURCE` scopes.** Both implement
  `HasVariables` (`configuration.rs:41`, `:77`) over differently named
  fields, so neither is caught by the derive guard. Under IEC 61131-3
  §2.7.1 a resource's `VAR_GLOBAL` is visible to the programs on it, so
  they are arguably scopes; `EnvironmentResolver` does not handle them
  today either, and nothing in #1439 depends on it. Recorded here so the
  omission is deliberate rather than rediscovered.
- **Node ids and a resolution map.** See the Phase 2 rationale.
- **`ScopeId` interning.** Only if profiling asks for it.

## Tasks

Phase 1 — closes #1439:

- [ ] Commit this plan
- [ ] `dsl/src/scope.rs`: `ScopeNode`, `ScopeBearing`, four impls
- [ ] `dsl/src/visitor.rs` + `fold.rs`: `enter_scope`/`exit_scope` defaults
- [ ] `dsl_macro_derive`: `scope`/`no_scope` attributes, `*_inner` split, enter/exit wrapper
- [ ] `dsl_macro_derive`: `variables: Vec<VarDecl>` compile-fail guard + its test
- [ ] `dsl/src/common.rs`: `#[recurse(scope)]` on the four POU structs
- [ ] `rule_use_declared_symbolic_var`: three overrides → one enter/exit pair
- [ ] `xform_fold_initializer_expressions`: three enter/exit pairs → trait pair
- [ ] `xform_resolve_expr_types`: maps → `ScopedTable`, drop `seed_implicit_globals`
- [ ] `compile_method.rs`: save/restore `ctx.variables`+`ctx.var_types`, bind `return_id`
- [ ] `ScopedTable`: debug assertion for balanced depth at end of walk
- [ ] Analyzer, plc2plc round-trip, and codegen end-to-end tests
- [ ] `method.rst`: narrow the limitation to the consumption half
- [ ] Confirm the reproductions from *Problem* now behave correctly
- [ ] `cd compiler && just`

Phase 2 — scope paths (separate PR):

- [ ] `symbol_environment.rs`: `ScopePath`, `ScopeKind::parent`, chain-walking `find`
- [ ] `symbol_environment.rs`: delete `resolution_cache`, `find_in_scope_hierarchy`
- [ ] `EnvironmentResolver`: `Option<Id>` → stack, driven by enter/exit
- [ ] Update the four production `ScopeKind::Named` call sites and the tests
- [ ] Check whether `specs/design/expression-type-resolution.md` needs a
      requirement for method-scoped resolution
- [ ] `cd compiler && just`

## Verification

`cd compiler && just`
