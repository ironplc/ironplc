# Fix: `project_io` and the MCP runner never see global variables

Issues: #1929 (globals never listed), #1713 (globals path has no tests).

## Goal

The MCP `project_io` tool lists global variables as REQ-TOL-mcp-210 and
REQ-TOL-mcp-211 require, and the `run` tool resolves and traces them by their
bare name, as REQ-ARC-mcp-020 requires.

## Architecture

`SymbolEnvironment::get_variables_in_scope` looks the scope up in
`scoped_symbols`, but `insert_symbol` keeps every `ScopeKind::Global` symbol
in `global_symbols`. The global lookup therefore always returns nothing, and
both of its callers (`project_io::collect_io` and `runner::build_symbol_map`)
silently skip every global.

Fix at the root: `get_variables_in_scope` resolves a scope to its symbol map
the same way `insert_symbol` does, so `ScopeKind::Global` reads
`global_symbols`. No caller has to change to see globals.

Once globals are listed, two kinds of global would be reported as inputs
although the caller cannot drive them:

- a `CONSTANT` global (and a program's `VAR_EXTERNAL CONSTANT` reference to
  it): its value never changes;
- a global the compiler provides (`__SYSTEM_UP_TIME`, `__SYSTEM_UP_LTIME`
  with `--allow-system-uptime-global`): the VM writes it every scan.

Both stay observable, so a non-addressed one is an output only. The symbol
table records whether a variable was declared `CONSTANT` (`SymbolInfo`
gains the declared qualifier's constness, captured before
`xform_mark_unwritten_constants` runs, so an inferred constant is still an
input). The design doc gets a requirement for this.

#1713 also asks for:

- `VAR_EXTERNAL` coverage: a program's `VAR_EXTERNAL` reference is an input.
- Scope of REQ-TOL-mcp-210: `collect_io` walks programs and globals only, so
  a `VAR_EXTERNAL` inside a function block is not listed. It references a
  global that is itself listed, so narrow the requirement to "`VAR_EXTERNAL`
  references of Programs" rather than widen the walk.
- The unused `is_hw_memory` flag: enforce the `%M*` exclusion explicitly
  instead of relying on it being incidental.

Located globals (`AT %IX0.0` in a `VAR_GLOBAL`) do not parse on `main` in any
`VAR_GLOBAL` block (#1913, fixed by open PR #1932). Their classification is
tested at the `classify` level with a `SymbolInfo` carrying an address, so
this change does not depend on #1932.

Out of scope, filed separately if not already tracked:

- #1162: `type` is always empty (the analyzer never fills
  `SymbolInfo::data_type`; the `symbols` tool has the same gap).
- REQ-ARC-mcp-020 names a `RESOURCE` global `<resource>.<variable>`, but the
  symbol table does not record which resource declared a global, so it is
  listed by its bare name.

## Prefactoring

`analyzer/src/symbol_environment.rs` is 1119 lines, over the 1000-line
limit, and the change adds tests to it. Move its test module to
`symbol_environment/tests.rs` first (the `rule_constant_range` layout),
without changing any test.

## Design doc reference

`specs/design/mcp-server.md` — `project_io` (REQ-TOL-mcp-210..212) and
Variable Naming (REQ-ARC-mcp-020).

## File map

| File | Change |
|------|--------|
| `compiler/analyzer/src/symbol_environment.rs` | `get_variables_in_scope` reads `global_symbols` for the global scope; `SymbolInfo` records `CONSTANT` |
| `compiler/analyzer/src/symbol_environment/tests.rs` | Moved tests; new global-scope and constant tests |
| `compiler/analyzer/src/xform_resolve_symbol_and_function_environment.rs` | Pass the declaration qualifier to `insert_variable` |
| `compiler/mcp/src/tools/project_io.rs` | Constant and compiler-provided globals are not inputs; explicit `%M*` exclusion; tests |
| `compiler/mcp/tests/scenarios.rs` | `project_io` → `compile` → `run` scenario over a configuration global |
| `specs/design/mcp-server.md` | Narrow REQ-TOL-mcp-210; new REQ-TOL-mcp-213 for globals that cannot be driven |

## Tasks

- [ ] Prefactor: move `symbol_environment` tests to their own file
- [ ] Failing tests: `project_io` over configuration and top-level globals, `VAR_EXTERNAL`, constants, compiler-provided globals, located globals via `classify`
- [ ] Failing test: `get_variables_in_scope(&ScopeKind::Global)` returns global variables
- [ ] Fix `get_variables_in_scope`
- [ ] Record `CONSTANT` in `SymbolInfo`; classify constant and compiler-provided globals as outputs only
- [ ] Scenario test: `project_io` names a global that `run` traces
- [ ] Update `specs/design/mcp-server.md`
- [ ] File issues for out-of-scope findings
- [ ] `git rm` this plan
- [ ] `cd compiler && just`, `cd specs && just`
