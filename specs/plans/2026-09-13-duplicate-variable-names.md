# Reject Duplicate Variable Names in a Scope (P4051)

Fixes [#1525](https://github.com/ironplc/ironplc/issues/1525): a user
`VAR_GLOBAL` that redeclares `__SYSTEM_UP_TIME` is accepted, shadows the
compiler-provided global and reads zero forever. The issue's root cause is
broader than the reserved names: nothing in the analyzer detects a variable
declared twice in the same scope. `SymbolEnvironment::insert` carries a
`TODO` for it and silently lets the later declaration overwrite the earlier
one; codegen then does the same with slot indices.

## Goal

A variable name declared more than once in the same scope is a diagnostic,
`P4051 VariableNameDuplicated`, pointing at the second declaration and at
the first. The scopes are:

- the global scope: every `VAR_GLOBAL` in the merged library (configuration,
  resource, top-level, activated library), plus the compiler-provided
  `__SYSTEM_UP_TIME` / `__SYSTEM_UP_LTIME` when `allow_system_uptime_global`
  is on, which is the case the issue reports;
- each `PROGRAM`, `FUNCTION`, `FUNCTION_BLOCK` and `METHOD`, across all of
  its `VAR*` blocks, edge variables included.

Out of scope, deliberately:

- Cross-scope hiding (a method local named like a function block field, a
  function block field named like a global). ADR-0051 decided which of
  those pairs are rejected; this change does not revisit it.
- Duplicate POU or type names. Those are a separate class with their own
  codes (`P2007`) and are not what the issue reports.
- Codegen dedup. Once the analyzer rejects the program, the overwrite in
  `assign_variables` is unreachable for duplicates.

## Architecture

A new semantic rule, `rule_var_decl_names_unique`, in the shape of
`rule_program_var_hides_global`: an AST-level `DiagnosticVisitor` run by
`run_rule`, so every duplicate in the library is reported (ADR-0048). It
is a rule rather than a check inside `SymbolEnvironment::insert` because
the symbol environment is built inside a reverting transform; an `Err`
there would discard the whole library for one duplicate.

For the global scope the rule reuses `collect_global_var_decls` and seeds
the seen-set with the reserved uptime names when the flag is on. A user
declaration of one of those names is reported with the same code and a
label saying the name is compiler-provided, since there is no first
declaration span to point at.

**Library shadowing.** `REQ-CL-analyzer-004` says a user declaration
shadows an activated library declaration of the same name, and
`Tc2_System` declares `VAR_GLOBAL CONSTANT PI`. Today only shadowed library
*functions* are dropped before the merge; a user global `PI` would now be
reported as a duplicate of the library's. `remove_shadowed_functions`
becomes `remove_shadowed_declarations` and also drops a library top-level
global whose name a user global declares, so the merged library carries
exactly one, as it already does for functions. This matches what the flat
slot table did by accident (the later, user, declaration won).

## Prefactoring

Two reshapes, each behaviour-preserving, in one commit before the rule:

1. **One definition of the reserved uptime globals.** The names and types
   of `__SYSTEM_UP_TIME` / `__SYSTEM_UP_LTIME` are spelled as literals in
   `stages.rs`, `rule_use_declared_symbolic_var.rs`,
   `xform_resolve_expr_types.rs` and `codegen/src/compile.rs`. The new rule
   would be a fifth copy, which is the "new branch in more than one place"
   signal. A `system_globals` module in the analyzer holds the one table;
   the four sites iterate it.
2. **Delete the no-op duplicate checks in `SymbolEnvironment::insert`.**
   Two `if let Some(_existing) = ... { /* TODO */ }` blocks do nothing. They
   go, replaced by a doc comment saying where duplicates are diagnosed, so
   the `TODO` the issue cites is no longer stale.

## Design doc reference

None. ADR-0051 covers cross-scope redeclaration and is unaffected;
`specs/design/compatibility-libraries.md` (`REQ-CL-analyzer-004`) is the
requirement the shadowing change honours.

## File map

- `compiler/analyzer/src/system_globals.rs` — new: the reserved uptime
  globals table
- `compiler/analyzer/src/stages.rs`,
  `compiler/analyzer/src/rule_use_declared_symbolic_var.rs`,
  `compiler/analyzer/src/xform_resolve_expr_types.rs`,
  `compiler/codegen/src/compile.rs` — use the table
- `compiler/analyzer/src/symbol_environment.rs` — remove the no-op checks
- `compiler/sources/src/libraries/mod.rs`,
  `compiler/project/src/project.rs`,
  `compiler/codegen/tests/it/end_to_end_tc2_math.rs`,
  `compiler/codegen/tests/it/end_to_end_tc2_utilities.rs` — shadowed
  library globals dropped alongside functions
- `compiler/problems/resources/problem-codes.csv` — `P4051`
- `compiler/analyzer/src/rule_var_decl_names_unique.rs` — new rule
- `compiler/analyzer/src/lib.rs`, `compiler/analyzer/src/stages.rs` —
  register it
- `docs/reference/compiler/problems/P4051.rst` — problem page
- `docs/reference/language/variables/scope.rst` — one sentence on
  same-scope duplicates next to the existing `P4050` note

## Tasks

- [ ] Commit this plan
- [ ] Prefactor: `system_globals` table; delete no-op checks
- [ ] Drop shadowed library globals in `remove_shadowed_declarations`
- [ ] Add `P4051` to the CSV and write the rule with tests: same block,
      across blocks, across `VAR_GLOBAL` blocks, edge variable, case
      difference, reserved name with the flag on and off, method vs
      function block field not reported, `VAR_EXTERNAL` not reported
- [ ] Register the rule; docs page; scope page sentence
- [ ] `cd compiler && just`, `cd docs && just compile`
- [ ] `git rm` this plan
