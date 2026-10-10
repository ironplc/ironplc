# Variable Binding: The Analyzer Decides Which Declaration a Name Means

## Goal

Make one analyzer function the only place that decides which declaration a
variable reference names, and state its answer in the language's own terms: the
scope the declaration is in, and its name. The semantic rules, today's code
generation and lowering all ask that function, so they cannot disagree.

The AST is not changed. It keeps the program as written, plus decisions recorded
in the language's vocabulary (`VarDecl::initial_value`, `ImplicitConversion`).
An identity that exists only to key storage is a lowering concern: the lowered
program's `VarId` (`specs/design/lowered-program.md` §3.1, REQ-LOW-lowering-020
and -022).

### Why

Today four places answer the question, and they disagree:

| Where | What it does |
|---|---|
| `SymbolEnvironment::find` | Enclosing scopes innermost first, then the `EXTENDS` chain, then the global scope. Returns a symbol of any kind. |
| `rule_use_declared_symbolic_var` | Calls `find`, then refuses a global that the POU does not reach through `VAR_EXTERNAL` (`is_usable_variable`). This is the only place the refusal is made. |
| `write_collector` | Maps a write through a `VAR_EXTERNAL` to `(Global, name)`. This is the only place an external is tied to its global, and it is tied by name. |
| Codegen `scope.rs` | Name-keyed maps, one per kind of variable. Each function and function block body gets a copy of the globals. A local only replaces the map of its own kind. |

The last row miscompiles programs that analysis accepts (on `main` at
`4480575`):

| Program | Result | Expected |
|---|---|---|
| Global `x : INT`; function local `x : BIG` (`DINT(0..100000)`); `x := 70000; F := x` | 4464 | 70000 |
| Program `t : TON := (PT := T#5s)`; function local `t : REC`; `F := t.PT` | `T#5000ms` | `T#0ms` |
| Global `p : POINT`; function local `p : DINT` | P9998 | 3 |
| Global array read in a function block through `VAR_EXTERNAL` | P9999 | the global's element |

Nothing checks that a `VAR_EXTERNAL` names a global, or that its type matches the
global's. Structure elements share the global symbol map with global variables.
When a global variable and a structure field have the same name, the later
insertion silently replaces the earlier one.

[#2175](https://github.com/ironplc/ironplc/pull/2175) tried to fix this by
putting a `DeclId` on every declaration and every reference in the AST, and keying
codegen storage by it. It was closed: the id has no meaning in the source, and
lowering already owns variable identity.

## Architecture

### The declaration, named in language terms

```rust
// compiler/analyzer/src/symbol_environment.rs
/// A variable declaration, named by the scope it is declared in and its name.
pub struct DeclRef {
    pub scope: ScopeKind,
    pub name: Id,
}
```

A `DeclRef` is not an id the analyzer allocates. It is how the program itself
names a declaration. It is unique because a name is declared at most once in a
scope (P4014), and `CleanAnalysis` ensures no repeat reaches codegen or lowering.
It can name any variable:

- a global (`Global`, including the compiler-provided uptime globals);
- a POU variable of any section;
- a function's or method's result variable (`ResultVariable`, keyed by the
  POU's own name);
- a function block field;
- a field inherited through `EXTENDS`, which names the base block's scope.

### One resolution function

```rust
impl SymbolEnvironment {
    /// The declaration `name` refers to from `from`, by the language's scope rules.
    pub fn resolve_variable(&self, name: &Id, from: &ScopeKind) -> Resolution<'_>;
}

pub enum Resolution<'a> {
    /// A variable the reference may use.
    Variable { decl: DeclRef, info: &'a SymbolInfo },
    /// A global the scope does not reach: it declares no `VAR_EXTERNAL` for it
    /// (P4007 today).
    GlobalNotVisible { decl: DeclRef, info: &'a SymbolInfo },
    /// The name is declared, but not as a variable (a type, a POU, ...).
    NotAVariable,
    /// Nothing visible declares the name.
    Undeclared,
}
```

The rules move into this one function, out of the places in the table above:

1. **Lookup order.** The enclosing scopes innermost first, then the `EXTENDS`
   chain, then the global scope, exactly as `find` does today.
2. **Global visibility.** A global is `Variable` only if it is compiler-provided
   or `allow_top_level_var_global` is set. Otherwise it is `GlobalNotVisible`.
   This is the rule's `is_usable_variable`, moved.
3. **`VAR_EXTERNAL` aliases its global.** A hit on an external resolves to the
   global's `DeclRef`, not to the external's. This is the decision
   REQ-LOW-lowering-022 relies on: lowering then gives the external no variable
   of its own.

Every caller that asks which variable a name means calls `resolve_variable`.
Code that asks about other kinds of symbol keeps calling `find`.

### Who uses it

- **The semantic rules:**
  - `rule_use_declared_symbolic_var` (P4007, and its "did you mean"
    suggestions);
  - `variable_type::declared` and `variable_type::of`;
  - `xform_resolve_expr_types`;
  - `write_collector`, whose external→global mapping is replaced by the
    function's;
  - `rule_ref_to`;
  - `rule_function_call_in_out_argument`;
  - `rule_function_call_type_check`;
  - `rule_constant_not_written`.
- **Lowering**, when it is built: it allocates one `VarId` per `DeclRef`. Add a
  row to the analyzer-decisions table of `lowered-program.md` §4: "Declaration a
  name refers to | `SymbolEnvironment::resolve_variable` | `VarId` per `DeclRef`".
  The function's result is recorded in the `SemanticContext` that analysis
  returns, which satisfies ADR-0058 ("the analyzer decides and lowering
  translates") without an AST field.
- **Today's codegen**, only if the stopgap below is approved.

### Out of scope, recorded for later

- **Passes that run before the symbol environment exists.** They keep name tables
  of their own: `xform_resolve_constant_expressions`,
  `xform_resolve_late_bound_expr_kind` (whose table also never enters a
  method's variables) and `xform_fold_initializer_expressions`. Moving them onto
  the environment means building it earlier in `stages::resolve_types`. That is
  its own plan; open an issue.
- **Field identity.** Field identity (`FieldIdx`) is lowering's, per the design.
- **Function block member visibility.** These are analyzer rules that need only
  the type environment, independent of this plan:
  - Reading or writing an internal `VAR` or `VAR_TEMP` from outside the block.
  - Writing a `VAR_OUTPUT` from outside the block. Edition 3 Figure 13 does not
    permit an output assignment from outside, as vendor compliance statements
    cite it; this needs confirming against the standard text.

  Open an issue.

## Prefactoring

**P1. Move resolution into `SymbolEnvironment::resolve_variable`.**
- Signals:
  - The same question ("which variable is this name") is answered with a
    different filter at each `find` call site.
  - The global-visibility rule lives inside one semantic rule.
- Change:
  - Add `DeclRef`, `Resolution` and `resolve_variable`, implementing exactly
    today's semantics: lookup order and global visibility.
  - Externals still resolve to the external's own declaration in this step.
  - Switch `rule_use_declared_symbolic_var` to it.
- Behaviour-preserving: the analyzer tests pass unchanged.

**P2. Route the other variable lookups through it.**
- Change: `variable_type::declared` and `of`, `xform_resolve_expr_types`,
  `rule_ref_to`, the two function-call rules and `rule_constant_not_written` call
  `resolve_variable`.
- Each of them types or checks a global whether or not it is visible today, and
  continues to: it accepts both `Variable` and `GlobalNotVisible`.
- Behaviour-preserving.

## Core Changes

**C1. `VAR_EXTERNAL` aliasing, and checking it.** These are decisions that can make
a program invalid, so they are the analyzer's (ADR-0058).
- `resolve_variable` resolves an external to its global's `DeclRef`.
- `write_collector` drops its own mapping.
- A new rule reports an external that names no global, or whose type differs from
  the global's, as a new problem code. It needs a
  `docs/reference/compiler/problems/P####.rst` page.
- Structure elements move out of the global symbol map into their structure
  type's scope, so a structure field cannot replace a global variable of the same
  name.
  - Check first which rules read structure elements from the global map, and move
    those reads.
  - If that turns out not to be mechanical, split this into its own PR.

**C2 (open question, see below). Codegen stopgap.**
- `scope.rs` keys its maps by `DeclRef` instead of `Id`.
- A body looks each reference up through `resolve_variable` from the body's scope
  (program, function, function block or method).
- The program's globals are no longer copied into bodies by name.
- Fixes rows 1, 3 and 4. Row 2 becomes a not-implemented error: a function-local
  structure has no storage yet.
- Same container bytes for every program that compiles today. Verify by dumping
  the containers of the codegen suite and the repository's `.st` files on both
  sides, as #2175 did.
- End-to-end tests for each row.

## Design Doc Reference

- **New `specs/design/variable-binding.md`** (`status: approved` until C1 lands).
  `REQ-VB-analyzer-NNN` requirements for:
  - lookup order;
  - global visibility;
  - external aliasing;
  - the external checks;
  - result variables;
  - inherited fields;
  - the structure-element fix.

  Plus `REQ-VB-codegen-NNN` for C2 if it is approved.
- **New ADR-0065**, "Name resolution is an analyzer function, not an AST
  annotation":
  - Records `DeclRef` and `resolve_variable`.
  - Records the rejected options: an AST `DeclId` (#2175), a side table keyed by
    scope and name, and codegen resolving names itself.
- **ADR-0051.** Add a dated postscript: its "function local hides global: Correct"
  row is not true in codegen today.
  - With C2, it becomes true.
  - Without C2, it stays untrue until lowering.
- **`lowered-program.md` §4.** Add the decisions-table row above, and in §8 note
  that `SymbolEnvironment::resolve_variable` is what `VarId`s are allocated from.

## File Map

| File | Change | PR |
|---|---|---|
| `compiler/analyzer/src/symbol_environment.rs` | `DeclRef`, `Resolution`, `resolve_variable` | P1, C1 |
| `compiler/analyzer/src/symbol_environment/tests.rs` | Resolution tests | P1, C1 |
| `compiler/analyzer/src/rule_use_declared_symbolic_var.rs` | Use `resolve_variable`; drop `is_usable_variable` | P1 |
| `compiler/analyzer/src/variable_type.rs` | Use `resolve_variable` | P2 |
| `compiler/analyzer/src/xform_resolve_expr_types.rs` | Use `resolve_variable` | P2 |
| `compiler/analyzer/src/rule_ref_to.rs`, `rule_function_call_in_out_argument.rs`, `rule_function_call_type_check.rs`, `rule_constant_not_written.rs` | Use `resolve_variable` | P2 |
| `compiler/analyzer/src/write_collector.rs` | Drop the external→global mapping | C1 |
| `compiler/analyzer/src/xform_resolve_symbol_and_function_environment.rs` | Structure elements in their type's scope | C1 |
| `compiler/analyzer/src/rule_var_external_matches_global.rs` (new) | External must name a global of its type | C1 |
| `compiler/problems/resources/problem-codes.csv`, `docs/reference/compiler/problems/P####.rst` | New problem code | C1 |
| `compiler/codegen/src/scope.rs`, `compile.rs`, `compile_fn.rs`, `compile_method.rs`, `compile_setup.rs` | Maps keyed by `DeclRef` | C2 |
| `compiler/codegen/tests/it/end_to_end_variable_binding.rs` (new) | The table's rows | C2 |
| `specs/design/variable-binding.md` (new) | Requirements | C1 |
| `specs/adrs/0065-name-resolution-is-an-analyzer-function.md` (new) | Decision | C1 |
| `specs/adrs/0051-...md`, `specs/design/lowered-program.md` | Postscript; table row | C1 |

## Tasks

### P1: `resolve_variable`
- [ ] Add `DeclRef`, `Resolution`, `resolve_variable` with today's lookup order and global visibility.
- [ ] Tests: local before global; method local before field; field from a method; inherited field names the base block; result variable; undeclared; not a variable; global not visible; compiler-provided global visible; `allow_top_level_var_global`.
- [ ] Switch `rule_use_declared_symbolic_var`; its tests pass unchanged.
- [ ] `cd compiler && just`.

### P2: route the variable lookups
- [ ] Switch `variable_type`, `xform_resolve_expr_types`, `rule_ref_to`, the two function-call rules, `rule_constant_not_written`.
- [ ] Analyzer tests pass unchanged; `cd compiler && just`.

### C1: externals and the global map
- [ ] Design doc and ADR-0065; ADR-0051 postscript; `lowered-program.md` row.
- [ ] `resolve_variable` resolves an external to its global; `write_collector` uses it.
- [ ] New rule and problem code for an external without a matching global; docs page.
- [ ] Structure elements out of the global symbol map; test that a global and a structure field of one name both resolve.
- [ ] Conformance tests for every `REQ-VB-analyzer-*`.
- [ ] `cd compiler && just`, `cd specs && just`, `cd docs && just`.

### C2 (if approved): codegen stopgap
- [ ] Key `scope.rs` maps by `DeclRef`; delete `globals_of`, `for_function_body`, `for_fb_body`.
- [ ] End-to-end tests for the four rows; container bytes unchanged for the codegen suite and the repository's `.st` files.
- [ ] `cd compiler && just`.

### Cleanup
- [ ] Open the tracking issue for the core PRs once this plan is approved.
- [ ] Open issues for the passes that run before the symbol environment, and for function block member visibility.
- [ ] Close this plan PR unmerged.

## Open Questions

1. **Codegen stopgap (C2), or leave the miscompiles to lowering?**
   - For C2: rows 1, 3 and 4 compile correctly now.
   - Against C2: the code is scheduled for deletion construct by construct.
2. **The external check's strictness.** Should a type mismatch between an external
   and its global be an error on every dialect, or should CODESYS and TwinCAT
   relax it?

## Coordination

- **Call binding.** Its parameter identity uses `DeclRef`, or a `VarId` once
  lowering exists, rather than an id of its own.
- **Execution model** (#2155). Its globals list names each global by `DeclRef`,
  including the uptime globals.
- **Initial values** (#2163). It touches `compile_setup.rs`. C2 rebases on
  whichever lands first.
