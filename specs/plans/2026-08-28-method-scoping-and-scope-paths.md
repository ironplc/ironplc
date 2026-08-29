# Plan: Traversal-driven scopes and scope paths

Fixes [#1439](https://github.com/ironplc/ironplc/issues/1439).

Delivered as four pull requests. Each builds, passes
`cd compiler && just`, and is independently releasable — none leaves the
compiler in a state where `check` and `compile` disagree. PR 1 is a
prefactor with no behaviour change; PR 2 closes #1439; PR 3 fixes the
type-checking half; PR 4 is the symbol-environment half and can be
deferred without reopening the issue.

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

## Prefactoring

The prefactoring is PR 1, and it is the whole shape of this change: move
scope entry out of the passes and into the traversal, with no behaviour
change, so that making a `METHOD` a scope is an attribute and a match arm
rather than an edit in five places.

It was landed as its own pull request ([#1454](https://github.com/ironplc/ironplc/pull/1454))
before any behaviour change, and the existing suite passed unchanged.
The signal it answers is the first one in
[Signals that a change needs prefactoring](../steering/development-standards.md#signals-that-a-change-needs-prefactoring):
the new behaviour needed a new arm in more than one place, so the
distinction wanted to be a type rather than repeated branching.

What good looked like, measured after the fact: PR 2 adds one enum
variant, flips one attribute, and adds two match arms — and the compiler
named both arms itself, because adding `ScopeNode::Method` made every
pass that discriminates fail to compile until it was handled.

PR 3's prefactoring is *not* the `ScopedTable` conversion — that is the
change itself. It is making `ScopedTable::find` take `&self`.
`ExprTypeResolver` reads its maps from `&self` helpers, one of which
(`resolve_parent_struct_type`) returns a reference whose lifetime is tied
to that borrow, and `find` took `&mut self` although it bottoms out in
`HashMap::get`. Without that one-word change the conversion would have had
to widen unrelated signatures to `&mut`.

PR 4 carries two prefactoring commits before its behaviour change.
First, removing the unused `SymbolEnvironment` API — thirteen items
reachable only from that module's own tests, seven of them typed in terms
of `ScopeKind`, so the path change has seven fewer signatures to carry.
Second, giving `ScopeKind` a path while every scope is still one segment
deep, so `find` resolves exactly what it did before.

No `ScopeKind::parent()`, which earlier drafts of this plan named: `find`
walks the path by slicing it and nothing else needs one, so building it
would be the speculative generality the standard warns against.

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

Source that compiles today starts erroring: the sibling-method leak and
the method-local constant leak (PR 2), and type errors inside method
bodies that were previously invisible (PR 3). That is
the point of the change. No `.st` corpus file uses `METHOD`, and the
existing METHOD tests are parser- and plc2plc-level, so the blast radius
is small.

## Pull requests

Four pull requests. The first is a prefactor that promises nothing
changed; the second makes the change the prefactor made easy.

| # | Pull request | Crates | Behaviour |
|---|---|---|---|
| 1 | Prefactor: existing scopes move onto the traversal hooks | `dsl`, `dsl_macro_derive`, `analyzer` | **none** |
| 2 | `METHOD` is a scope | `dsl`, `analyzer`, `codegen`, docs | **closes #1439** |
| 3 | `xform_resolve_expr_types` on `ScopedTable` | `analyzer` | method bodies get type-checked |
| 4 | Scope paths in `SymbolEnvironment` | `analyzer`, `mcp` | method symbols scoped for LSP/MCP |

---

### PR 1 — Prefactor: existing scopes move onto the traversal hooks

*No behaviour change. No new tests. The existing suite is the proof.*

`ScopeNode` ships with **three** variants — `Function`, `FunctionBlock`,
`Program` — and `MethodDeclaration` is deliberately not a scope yet. That
is what keeps this pull request honest, and it sets up the next one: PR 2
adds the fourth variant, and every exhaustive match fails to compile
until it is handled. The enforcement mechanism gets exercised on its
first real use rather than sitting untested until the next contributor
needs it.

- `compiler/dsl/src/scope.rs` (new) — `ScopeNode` (three variants), the
  `ScopeBearing` trait, three impls.
- `compiler/dsl/src/visitor.rs`, `compiler/dsl/src/fold.rs` —
  `enter_scope`/`exit_scope` with no-op defaults on both traits.
- `compiler/dsl_macro_derive/src/lib.rs` — parse `scope` and `no_scope`
  in the `recurse` attribute; in `expand_struct_recurse_visit` and
  `expand_struct_recurse_fold`, move the existing body into a private
  `*_inner` and emit the enter/exit wrapper when `scope` is set. Add the
  `variables: Vec<VarDecl>` guard described above.
- `compiler/dsl/src/common.rs` — `#[recurse(scope)]` on
  `FunctionDeclaration`, `FunctionBlockDeclaration` and
  `ProgramDeclaration`; `#[recurse(no_scope)]` on `MethodDeclaration`,
  with a comment citing #1439. The guard forces the annotation, so the
  marker records today's (incorrect) behaviour explicitly rather than
  leaving it implied by an omission — and PR 2's diff becomes one word
  plus one match arm.
- `compiler/analyzer/src/rule_use_declared_symbolic_var.rs` — three
  `visit_*_declaration` overrides become one `enter_scope`/`exit_scope`
  pair. A pure move: the same names are seeded into the same table at the
  same points.
- `compiler/analyzer/src/xform_fold_initializer_expressions.rs` — the
  three enter/exit pairs at `:292`, `:303`, `:314` become the trait pair.
  Also a pure move.

The only new test is the compile-fail case for the derive guard, which is
the guarantee itself and cannot be deferred without weakening it. The
generated wrapper needs no synthetic test: both migrated passes run under
the existing suite, which is the point of migrating them in the same pull
request.

`xform_resolve_expr_types` is *not* migrated here — see PR 3.

### PR 2 — `METHOD` is a scope

*Closes #1439 defects 1, 2 and 5.*

- `compiler/dsl/src/scope.rs` — add `ScopeNode::Method` and the
  `ScopeBearing` impl. Both migrated passes now fail to compile until
  their match handles it, which is the mechanism doing its job.
- `compiler/dsl/src/common.rs` — `no_scope` becomes `scope` on
  `MethodDeclaration`.
- `rule_use_declared_symbolic_var` — the `Method` arm seeds the method
  name **only when `return_type` is `Some`**. Fixes defects 1 and 2.
- `xform_fold_initializer_expressions` — the `Method` arm enters a scope
  and seeds nothing. Fixes defect 5.
- `compiler/codegen/src/compile_method.rs` — save and restore
  `ctx.variables` and `ctx.var_types` around each method, as
  `compile_user_function` does at `compile_fn.rs:78`/`:467`; insert
  `return_id → return_var_index` before `emit_function_local_prologue`,
  mirroring `compile_fn.rs:258`. Fixes defect 4, and must land no later
  than this pull request: `ctx.var_index` (`compile.rs:1230`) reports
  `Problem::VariableUndefined`, so an analyzer that accepts
  `GetSpeed := speed` while codegen cannot resolve the name would ship a
  release where `check` passes and `compile` fails with the same P4007
  the analyzer just stopped reporting. It is ~40 lines and independently
  justifiable as a latent-bug fix, so it can be split off to land first
  if this pull request feels large.
- `compiler/analyzer/src/scoped_table.rs` — debug assertion that the
  stack is back to depth 1 when a walk ends, so a leaked scope fails the
  suite rather than surfacing later as a mis-resolution.
- `docs/reference/language/object-orientation/method.rst:104-115` — the
  limitation currently covers both halves in one sentence. Narrow it: a
  method body assigns its own name to set the result value; `x :=
  instance.Method()` is still a syntax error pending #1421. The
  `--allow-fb-inheritance` note at `:33-34` points at the same anchor and
  needs the same narrowing.

Tests in `rule_use_declared_symbolic_var`:

- `apply_when_method_assigns_own_name_then_ok`
- `apply_when_method_without_return_type_assigns_own_name_then_error`
- `apply_when_method_references_sibling_method_local_then_error`
- `apply_when_method_references_function_block_field_then_ok`
- `apply_when_method_references_inherited_field_then_ok`
- `apply_when_two_methods_declare_same_local_name_then_ok`

In `xform_fold_initializer_expressions`:
`apply_when_method_local_constant_not_visible_in_sibling_method_then_error`,
the sibling of the existing
`apply_when_fb_local_constant_not_visible_in_other_fb_then_error`.

In codegen: two methods on one function block each declaring a local of
the same name compile to *distinct* slots. That source is legal both
before and after this change, so the test does not depend on the leak it
is proving absent.

Plus, per the syntax-support guide, a `plc2plc` round-trip in
`compiler/plc2plc/src/tests/methods.rs` and an execution test in
`compiler/codegen/tests/it/end_to_end_methods.rs` for a method that
computes its result via `MethodName := …`. The execution test exercises
*production* only: consuming a method result in an expression stays
blocked on #1421, so the value is observed through a field the method
writes rather than through `v := m.GetSpeed()`.

### PR 3 — `xform_resolve_expr_types` on `ScopedTable`

*Fixes defect 3.*

Held out of PR 1 because it is not a call-site move. `ExprTypeResolver`
keeps flat `var_types` / `array_element_types` maps that it `clear()`s at
each POU boundary (`:526`, `:543`, `:555`); clearing at a method boundary
would wipe the enclosing function block's fields, so the maps themselves
have to become scoped. More importantly, this is the one pass where a
mistake is silent — an unresolved name does not error, it skips the type
check, which is exactly how defect 3 stayed hidden — so "existing tests
pass" is a weaker promise here than elsewhere and deserves its own
review.

One deliberate behaviour delta: every POU fold inserts locals and then
calls `seed_implicit_globals()` (`:513-514`), so a global currently
overwrites a same-named local; under a scope stack the local shadows the
global, which is correct. It is close to unreachable today — top-level
`VAR_GLOBAL` never reaches `global_var_types` (verified: `y := i` with a
`BOOL` global and an `INT` local exits 0), leaving only the two
`__SYSTEM_UP_*` names — but it is a real change and is the reason this
pull request cannot claim behaviour neutrality.

Order within the pull request, so review can follow it:

1. Convert both maps to `ScopedTable`, keeping function, function block
   and program behaviour identical. Move globals into the base frame in
   `fold_library`, which lets `seed_implicit_globals` and its per-POU
   re-seeding go away. Existing tests must pass unchanged.
2. The `ScopeNode::Method` arm then falls out of the machinery, which is
   what fixes defect 3.
3. Seed the method's own name at its resolved return type, mirroring the
   function case at `:520-525`, so the assignment PR 2 made legal is also
   type-checked.

Tests:

- `apply_when_method_local_then_resolves_type` — the resolution defect 3
  currently skips, and with it the P4035 that depended on it
- `apply_when_method_local_shadows_field_then_resolves_local_type`
- `apply_when_two_methods_declare_same_name_then_each_resolves_own_type`
- `apply_when_method_reads_own_name_then_resolves_return_type`
- `apply_when_method_has_no_return_type_then_own_name_unresolved`
- `apply_when_function_block_body_references_method_local_then_unresolved`

Not `apply_when_method_assigns_own_name_wrong_type_then_error`, which the
plan previously listed: assigning the *result variable* a wrong-typed
value is unchecked for a `FUNCTION` too, so there is no behaviour for a
method to match. See *Out of scope*.

### PR 4 — Scope paths in `SymbolEnvironment`

*Method symbols become correctly scoped for LSP and MCP consumers.*

- `compiler/analyzer/src/symbol_environment.rs` — `ScopePath`, the new
  `ScopeKind`, `parent()`, and the chain-walking `find`. Delete
  `resolution_cache` (constructed and cleared, never read) and
  `find_in_scope_hierarchy` (`#[allow(dead_code)]`, subsumed by the new
  `find`).
- `compiler/analyzer/src/xform_resolve_symbol_and_function_environment.rs`
  — `scope: Option<Id>` becomes a `Vec<Id>` stack driven by
  `enter_scope`/`exit_scope`; `current_scope()` reads it.
- Four production call sites construct `ScopeKind::Named` and each names
  a POU scope at depth 1, so each becomes a one-segment path:
  `extractors.rs:220` and `:238`, `mcp/src/tools/project_io.rs:100`,
  `mcp/src/runner.rs:74`. Six further sites are in tests.
- Tests: `find` reaches an outer scope through two levels of nesting; an
  inner declaration shadows an outer one of the same name; a method
  parameter resolves at `⟨FB, Method⟩` and not at `⟨FB⟩`; two methods
  with the same local name keep distinct entries; a function block field
  is still found from inside a method by the parent walk.
- Check whether `specs/design/expression-type-resolution.md` needs a
  requirement for method-scoped resolution.

---

## Out of scope

- **#1421, the consumption half.** A method call is reachable only from
  statement position, so `v := m.GetSpeed()` remains a syntax error.
  Both must land before `METHOD … : T` is usable end to end.
- **`CONFIGURATION` and `RESOURCE` scopes.** Both implement
  `HasVariables` (`configuration.rs:41`, `:77`) over differently named
  fields, so neither is caught by the PR 1 guard. Under IEC 61131-3
  §2.7.1 a resource's `VAR_GLOBAL` is visible to the programs on it, so
  they are arguably scopes; `EnvironmentResolver` does not handle them
  today either, and nothing in #1439 depends on it. Recorded here so the
  omission is deliberate rather than rediscovered.
- **`rule_function_call_type_check`'s own scope tracking.** Found while
  implementing PR 3: it is a *sixth* pass with its own flat
  `var_types: HashMap<Id, TypeName>`, `clear()`ed at each of the three POU
  boundaries (`:350`, `:358`, `:366`) and populated from `visit_var_decl`,
  with no method handling and no result variable. Two consequences: a
  method's locals are visible to its siblings for type-check purposes, and
  assigning a wrong-typed value to a result variable is never diagnosed —
  for a `FUNCTION` as much as for a `METHOD`, so it is not a regression
  this change introduces. It is the same defect class and migrating it is
  mechanical (three `clear()` overrides become one enter/exit pair), but it
  *adds* diagnostics, so it belongs in its own pull request rather than
  riding along in PR 3.

  Worth noting for the design: the derive guard cannot catch this site.
  The guard forces an *AST node* holding `variables: Vec<VarDecl>` to
  declare whether it scopes them; it cannot force a *pass* to use the
  hooks. Passes that keep their own flat map remain findable only by
  reading them.

- **Node ids and a resolution map.** See the scope-path rationale.
- **`ScopeId` interning.** Only if profiling asks for it.

No new problem codes. P4007 and P4035 already exist; this change makes
them fire correctly.

## Tasks

- [x] Commit this plan
- [x] PR 1 — prefactor: hooks, derive guard, migrate the two pure-move passes (#1454)
- [x] PR 2 — `METHOD` is a scope: dsl, analyzer, codegen, docs, tests (#1463)
- [x] Confirm reproductions 1, 2, 4 and 5 in *Problem* now behave correctly
- [x] PR 3 — `xform_resolve_expr_types` on `ScopedTable`
- [x] Confirm reproduction 3 in *Problem* now behaves correctly
- [x] PR 4 — scope paths in `SymbolEnvironment`

## Verification

`cd compiler && just` on every pull request.
