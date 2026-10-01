# Name a RESOURCE global `<resource>.<variable>` in the MCP server

Plan for #1934.

## Goal

The MCP server names a global declared in a `RESOURCE` `VAR_GLOBAL` block
`<resource>.<variable>` (for example `resource1.level`), as REQ-ARC-mcp-020
requires, in both places that produce variable names:

- the `inputs` and `outputs` of the `project_io` tool;
- the symbol map that `compile` stores with a container, which the `run` tool
  uses to resolve `variables` and to name the `trace` and `final_values`
  entries.

A bare name (`level`) still resolves in `run` when the container has exactly
one resource, which is the rule REQ-ARC-mcp-020 states. Configuration and
top-level globals keep their bare names.

## Architecture

### Where the resource is recorded

`SymbolEnvironment` keeps configuration, resource and top-level globals in one
global scope, and nothing records which resource declared a global. The fix
adds that fact to the symbol, not to the scope:

- `SymbolInfo` gains `resource: Option<Id>`, the name of the `RESOURCE` whose
  `VAR_GLOBAL` block declares the variable. It is `None` for every other
  symbol.
- `EnvironmentResolver` (`xform_resolve_symbol_and_function_environment.rs`)
  remembers the resource it is inside (`visit_resource_declaration` sets it,
  and clears it after the recursion), and `visit_var_decl` passes it on.

The global scope and its lookups do not change, so `VAR_EXTERNAL` resolution,
the P4014 repeated-name check and every analyzer rule behave as today.

This keeps #1489 open rather than answering it. #1489 asks whether
`CONFIGURATION` and `RESOURCE` should become scopes; this change only records
provenance, which a later scoping change can build on or replace. Recording
the resource in the scope key instead (a `ScopeKind` for resources) would
answer #1489 implicitly and would break the flat global lookup that
`VAR_EXTERNAL` relies on, so it is not proposed here.

Considered and rejected: having the MCP server walk the AST
(`ConfigurationDeclaration::resource_decl[].global_vars`) to find resource
globals. It works without touching the analyzer, but it duplicates
knowledge the symbol table is meant to own: `project_io` and the runner
already take every variable from the symbol table, and the resource that
declares a global is a fact the analyzer will need anyway once #1489 decides
how resource globals are scoped.

### How the MCP server names variables

Today `project_io::collect_io` and `runner::build_symbol_map` each walk the
programs and the globals and build the qualified name themselves, so the new
rule would be a new branch in both. A prefactor moves that walk into one
function, `variable_names::qualified_variables`, which returns, for every
variable the MCP server exposes, its canonical name, its declaring program
(if any) and its `SymbolInfo`. Both callers use it; the core change then
adds the `<resource>.<variable>` case in one place.

`ResolvedVar` (in `cache.rs`) gains `resource: Option<String>`.
`run::resolve_name` applies the REQ-ARC-mcp-020 order for a bare name:

1. exactly one configuration or top-level global with that name;
2. otherwise, when the container has exactly one resource, exactly one
   resource global with that name;
3. otherwise the existing rule (a single candidate resolves, several are
   ambiguous).

A dotted name keeps its exact lookup, so `resource1.level` resolves by its
canonical name. `is_observable_output` keeps treating a resource global as a
global (`program.is_none()`), so `trace_outputs` includes it.

Today a configuration global and a resource global cannot share a name
(P4014, one flat scope), and `compile` accepts one program instance, hence
one resource, so step 2 never meets two resources. The rule is still written
as the requirement states it, so it stays correct if #1489 or #1613 relax
those limits.

### Relation to open PRs

- #1938 (fix for #1930) makes `compile` allocate a resource's globals, after
  which they appear in the container's debug section by their bare name.
  Until it lands, a resource global does not compile, so `run` cannot be
  exercised end to end; the runner change is tested with a hand-built
  container (`ContainerBuilder::add_var_name`). If #1938 lands first, the
  core PR also adds the end-to-end scenario in `mcp/tests/scenarios.rs`:
  `project_io` lists `resource1.level`, `run` traces it, and the bare name
  resolves too. #1938 changes codegen only; no file overlaps.
- #1932 (located configuration and resource globals) changes the parser and
  one analyzer rule. A located resource global gets the same
  `<resource>.<variable>` name with its address; no file overlaps.
- #1902 (several resources) leaves resource globals in the flat scope (P4014
  on a reused name). The naming here works per symbol, so it is correct with
  several resources too.

## Prefactoring

Two behaviour-preserving prefactors, each its own PR from `main`:

1. **Analyzer: `insert_variable` takes the variable's `SymbolInfo`.** It has
   seven inputs today (`self`, name, kind, scope, variable type, qualifier,
   address); an eighth for the resource would exceed clippy's
   `too_many_arguments` limit of seven, and suppressing the lint is not an
   option. A new `SymbolInfo::variable(kind, scope, span,
   variable_type, qualifier)` constructor builds the symbol (and sets
   `is_external` for `VAR_EXTERNAL`, as `insert_variable` does now), the
   caller adds `with_address` when there is one, and
   `insert_variable(name, info)` inserts it. The existing tests change only
   in how they call it.
2. **MCP: one function names the exposed variables.** Add
   `compiler/mcp/src/variable_names.rs` with `qualified_variables`, and make
   `project_io::collect_io` and `runner::build_symbol_map` use it. The
   `IoScope` enum in `project_io.rs` becomes a test of whether the variable
   has a declaring program. The existing `project_io`, runner and scenario
   tests pass unchanged.

## Design doc reference

`specs/design/mcp-server.md`, Variable Naming (REQ-ARC-mcp-020,
REQ-ARC-mcp-021), `project_io` (REQ-TOL-mcp-212) and `run` (REQ-TOL-mcp-041).
The requirements already specify the behaviour; no requirement text changes.
The core PR replaces the ignored placeholder tests for REQ-ARC-mcp-020 and
REQ-ARC-mcp-021 in `spec_conformance.rs` (already over the module size limit)
with real `#[spec_test]` tests in a new module.

## File map

| File | Change | PR |
|------|--------|----|
| `compiler/analyzer/src/symbol_environment.rs` | `SymbolInfo::variable`; `insert_variable(name, info)` | prefactor 1 |
| `compiler/analyzer/src/symbol_environment/tests.rs` | Call sites | prefactor 1 |
| `compiler/analyzer/src/xform_resolve_symbol_and_function_environment.rs` | Call sites | prefactor 1 |
| `compiler/mcp/src/variable_names.rs` (new) | `qualified_variables` | prefactor 2 |
| `compiler/mcp/src/tools/project_io.rs` | Use `qualified_variables` | prefactor 2 |
| `compiler/mcp/src/runner.rs` | Use `qualified_variables` | prefactor 2 |
| `compiler/mcp/src/lib.rs` | Module declaration | prefactor 2 |
| `compiler/analyzer/src/symbol_environment.rs` | `SymbolInfo::resource`, `with_resource` | core |
| `compiler/analyzer/src/xform_resolve_symbol_and_function_environment.rs` | Track the current resource; record it | core |
| `compiler/mcp/src/variable_names.rs` | `<resource>.<variable>` | core |
| `compiler/mcp/src/cache.rs` | `ResolvedVar::resource` | core |
| `compiler/mcp/src/runner.rs` | Fill `ResolvedVar::resource` | core |
| `compiler/mcp/src/tools/run.rs` | Bare-name order of REQ-ARC-mcp-020 | core |
| `compiler/mcp/src/spec_conformance.rs` | Remove the two ignored placeholders | core |
| `compiler/mcp/src/spec_conformance_variable_naming.rs` (new) | `#[spec_test(REQ_ARC_mcp_020)]`, `#[spec_test(REQ_ARC_mcp_021)]` | core |
| `compiler/mcp/tests/scenarios.rs` | End-to-end scenario, if #1938 has landed | core |
| `docs/reference/mcp/tools.rst` | Say how a resource global is named (`project_io`, `run`) | core |

## Tasks

### Prefactor 1: `insert_variable` takes a `SymbolInfo` (analyzer)

- [ ] Add `SymbolInfo::variable`, change `insert_variable` to take the info
- [ ] Update the resolver and the tests' call sites
- [ ] `cd compiler && just`

### Prefactor 2: one function names the MCP variables

- [ ] Add `variable_names.rs` with `qualified_variables`, unit tests for the
      program and global cases as they behave today
- [ ] Use it in `project_io::collect_io` and `runner::build_symbol_map`
- [ ] `cd compiler && just`

### Core: name resource globals `<resource>.<variable>`

- [ ] Analyzer test (seen failing first): a `RESOURCE` `VAR_GLOBAL` symbol
      records its resource; a configuration global, a top-level global and a
      program variable record none
- [ ] Add `SymbolInfo::resource` and record it in `EnvironmentResolver`
- [ ] `project_io` test (seen failing first): the source from #1934 lists
      `resource1.level` in `inputs` and `outputs`; a configuration global in
      the same source keeps its bare name
- [ ] Name resource globals in `qualified_variables`
- [ ] Runner test with a hand-built container: the symbol map holds
      `resource1.level` with `resource` set
- [ ] `run` tests: `resource1.level` and the bare `level` both resolve to the
      resource global; an unknown `resource2.level` does not resolve
- [ ] Replace the ignored REQ-ARC-mcp-020 / REQ-ARC-mcp-021 placeholders with
      `#[spec_test]` tests in `spec_conformance_variable_naming.rs`
- [ ] End-to-end scenario in `mcp/tests/scenarios.rs` if #1938 has landed;
      otherwise open a follow-up issue for it
- [ ] Docs: `docs/reference/mcp/tools.rst`
- [ ] `cd compiler && just`, `cd specs && just`, docs build with `-W -n`
