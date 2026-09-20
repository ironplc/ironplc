# Reject Duplicate Variable Names in a Scope (P4014)

Fixes [#1525](https://github.com/ironplc/ironplc/issues/1525): a user
`VAR_GLOBAL` that redeclares `__SYSTEM_UP_TIME` is accepted, shadows the
compiler-provided global and reads zero forever. The issue's root cause is
broader than the reserved names: nothing detects a variable declared twice
in the same scope, and the later declaration silently wins the variable
slot.

## Goal

A variable name declared more than once in the same scope is
`P4014 SymbolDeclDuplicated`, pointing at the second declaration and at the
first. The code already exists, documented for exactly this case, and is
emitted nowhere. The scopes are:

- the global scope: every `VAR_GLOBAL` in the merged library (configuration,
  resource, top-level, activated library), plus the compiler-provided
  `__SYSTEM_UP_TIME` / `__SYSTEM_UP_LTIME` when `allow_system_uptime_global`
  is on, which is the case the issue reports;
- each `PROGRAM`, `FUNCTION`, `FUNCTION_BLOCK` and `METHOD`, across all of
  its `VAR*` blocks, edge variables included.

Cross-scope hiding is unchanged (ADR-0051). A `VAR_EXTERNAL` names a global
from another scope, so it is not a duplicate of that global; two
`VAR_EXTERNAL` of one name in one unit are.

## Architecture

The symbol environment does the check, as it does for a repeated global
declaration: `SymbolEnvironment::insert` and `insert_variable` share one
insertion path that returns `P4014` when a variable kind repeats a variable
kind in the same scope and keeps the first declaration. The transform that
feeds the environment already collects such diagnostics best-effort.

The compiler-provided uptime globals are seeded into the environment before
the library is walked, so a user declaration of one collides on insert with
no special code. The seeded symbols are marked compiler-provided so the
diagnostic can say the name is reserved instead of pointing at a source
location that does not exist.

The initializer-folding pass keeps its own table of global constants and
reported `P4011` when one repeated. It now keeps the first constant
silently, so a repeated `VAR_GLOBAL CONSTANT` is one `P4014` rather than
two diagnostics for one line.

## Prefactoring

None. `insert` and `insert_variable` each build a `SymbolInfo` and put it
in the scope's map; the shared insertion path this change introduces is
where the check lives, so it arrives with the check rather than before it.

## Design doc reference

None.

## File map

- `compiler/analyzer/src/symbol_environment.rs` — shared insertion path,
  the `P4014` check, compiler-provided symbols
- `compiler/analyzer/src/stages.rs` — seed the uptime globals as
  compiler-provided
- `compiler/analyzer/src/xform_fold_initializer_expressions.rs` — keep the
  first constant silently
- `compiler/analyzer/src/xform_resolve_symbol_and_function_environment.rs`
  — pipeline tests for every scope
- `compiler/analyzer/src/spec_conformance.rs` — a user global repeating a
  library global under `REQ-CL-analyzer-007`
- `docs/reference/compiler/problems/P4014.rst` — problem page, expanded
- `docs/extensions/thin_problem_pages_allowlist.txt` — P4014 is no longer
  thin
- `docs/reference/language/variables/scope.rst` — one sentence

## Tasks

- [ ] Commit this plan
- [ ] Shared insertion path with the check; compiler-provided seeding
- [ ] Initializer-folding pass keeps the first constant
- [ ] Tests: same block, across blocks, across `VAR_GLOBAL` blocks, edge
      variable, case difference, reserved name with the flag on and off,
      method vs function block field not reported, `VAR_EXTERNAL` not
      reported, library global repeated
- [ ] Docs page; scope page sentence
- [ ] `cd compiler && just`, `cd docs && just compile`, `cd specs && just`
- [ ] `git rm` this plan
