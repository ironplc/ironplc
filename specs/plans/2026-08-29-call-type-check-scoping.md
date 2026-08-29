# Plan: Scope `rule_function_call_type_check`'s variable types

Follow-on from
[2026-08-28-method-scoping-and-scope-paths](2026-08-28-method-scoping-and-scope-paths.md),
which recorded this pass in its *Out of scope* section as the sixth place
tracking scope by hand.

## Problem

`rule_function_call_type_check` — which raises P4035 (assignment type
mismatch) and P4027 (function call return type mismatch) — keeps its own
`var_types: HashMap<Id, TypeName>`, `clear()`ed at each of the three POU
boundaries (`:350`, `:358`, `:366`) and filled from `visit_var_decl`. It
has no notion of a method scope and no result variable. Two defects
follow.

Verified against `3b2289c` with `ironplcc check --dialect iec61131-3-ed3`:

1. **A method local leaks its type to the next method, hiding a real
   mismatch.** Every method's declarations land in the enclosing function
   block's map, so a method local overwrites a field of the same name for
   every method compiled after it:

   ```st
   FUNCTION_BLOCK FB_Motor
   VAR
       v : INT;
   END_VAR
   METHOD A
   VAR
       v : REAL;
   END_VAR
       v := 1.5;
   END_METHOD
   METHOD B
       v := 2.5;
   END_METHOD
   END_FUNCTION_BLOCK
   ```

   In `B`, `v` is the function block's `INT` field, so `v := 2.5` is an
   `INT := REAL` mismatch. It exits 0: `A`'s local left `v` recorded as
   `REAL`.

2. **Assigning the result variable is never type-checked.** The pass
   never records a declaration's own name, so the target type lookup
   misses and the check returns early. `FUNCTION GetFlag : BOOL` whose
   body says `GetFlag := n` with `n : INT` exits 0. This affects a
   `FUNCTION` exactly as much as a `METHOD`, so it is not a regression
   from method scoping — it has always been unchecked.

## Prefactoring

None needed, and that is the point. The shape this change requires —
scope entry driven by the traversal, and a `ScopedTable` whose `find`
borrows immutably — was built by
[#1454](https://github.com/ironplc/ironplc/pull/1454),
[#1463](https://github.com/ironplc/ironplc/pull/1463) and
[#1466](https://github.com/ironplc/ironplc/pull/1466). Migrating a sixth
pass onto it is now a drop-in: delete three `clear()` overrides, add one
`enter_scope`/`exit_scope` pair.

## Design

Replace the flat map with `ScopedTable<'static, Id, TypeName>` and the
three `clear()` overrides with the scope hooks, exactly as
`xform_resolve_expr_types` now does. `enter_scope` pushes a frame and
records the declaration's own name as its result variable; `exit_scope`
pops, dropping only what that declaration added.

The result variable is seeded for a `FUNCTION` unconditionally and for a
`METHOD` **only when it declares a return type** — a method without one
has no result to assign, and `rule_use_declared_symbolic_var` already
rejects the assignment outright.

`visit_var_decl` keeps filling the table; it simply fills the current
frame rather than one flat map.

### Behaviour change

Both defects are silent acceptance, so fixing them turns previously
accepted source into errors. Measured against the full suite: **zero
existing tests change outcome**, for either half. Nothing in the corpus
assigns a mismatched type to a result variable or depends on the leak.

## Implementation

### Commit 1 — scope the variable types

`compiler/analyzer/src/rule_function_call_type_check.rs`

- `var_types` becomes `ScopedTable<'static, Id, TypeName>`.
- The three `visit_*_declaration` overrides are deleted; `enter_scope`
  pushes a frame and `exit_scope` pops one. The `ScopeNode` match is
  exhaustive, as in the other migrated passes.
- `self.var_types.get(..)` becomes `find(..)`; `insert` becomes `add`.

Fixes defect 1.

### Commit 2 — record the result variable

- `enter_scope` seeds the declaration's own name at its return type:
  unconditionally for `Function`, and for `Method` when `return_type` is
  `Some`. `Program` and `FunctionBlock` have no result.

Fixes defect 2.

## Tests

Commit 1:

- `apply_when_method_local_shadows_field_then_sibling_method_uses_field_type`
  — the reproduction above, asserting P4035.
- `apply_when_method_local_assigned_wrong_type_then_error` — a method
  local is type-checked against its own declared type.

Commit 2:

- `apply_when_function_result_assigned_wrong_type_then_error`
- `apply_when_function_result_assigned_correct_type_then_ok`
- `apply_when_method_result_assigned_wrong_type_then_error`
- `apply_when_method_without_return_type_then_name_not_a_target`

Each must fail without the commit that introduces it; verify by
neutering the relevant arm before relying on it.

## Out of scope

- **`P4027`'s own target lookup.** `check_return_type` reads the same
  `var_types`, so it inherits the fix for free. No separate work, but
  also no new tests for it beyond what exists.
- **The remaining `#[allow(dead_code)]` surface in
  `symbol_environment.rs`.** Unrelated to this pass.

## Tasks

- [x] Commit this plan
- [x] Commit 1 — scope the variable types, with tests
- [x] Commit 2 — record the result variable, with tests
- [x] Confirm both reproductions in *Problem* now report P4035
- [ ] `cd compiler && just`

## Verification

`cd compiler && just`
