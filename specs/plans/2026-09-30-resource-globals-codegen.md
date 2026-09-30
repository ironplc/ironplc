# Allocate RESOURCE VAR_GLOBAL variables in codegen

Issue: #1930

## Goal

A `VAR_GLOBAL` declared inside a `RESOURCE` passes `check` today, but
`compile` rejects every read or write of it with P4007, because codegen only
allocates configuration globals and top-level globals. After this change the
globals of the resource that instantiates the program are allocated like
configuration globals: they take their initial values, the program reads and
writes them through `VAR_EXTERNAL`, and their values persist across scans.

## Architecture

`compile()` in `compiler/codegen/src/compile.rs` builds the list of global
declarations (`synthetic_globals`) that `compile_program_with_functions`
allocates at variable indices `0..G`. `VAR_EXTERNAL` in the program resolves
by name against those indices, so no change is needed downstream: adding the
resource's declarations to that list is the whole fix.

Which resource: IEC 61131-3 (2.7.1) makes a resource's globals visible to the
programs of that resource only (#1489 records that the analyzer does not yet
enforce this). Codegen therefore appends the globals of the one resource that
instantiates the compiled program, not every resource's globals. On `main`
the grammar allows one resource per configuration and that resource must
instantiate a program, so this is the same set; with several resources
(#1902) it stays correct and avoids allocating another resource's globals
(which may legally reuse a name).

Order in the variable table: system uptime globals, top-level globals,
configuration globals, then the program's resource globals. Names cannot
collide across these blocks: the analyzer reports P4014 for a repeated name
across `VAR_GLOBAL` blocks.

## Prefactoring

1. Move the construction of the global declaration list out of `compile()`
   into a new module `compiler/codegen/src/program_globals.rs`
   (`program_globals(library, config, program_name, options)`), unchanged in
   behaviour. `compile.rs` is already over the module size limit, so the new
   logic should not grow it.
2. Extract `find_program_resource(config, program_name)` from
   `find_bound_task`, which already walks the resources looking for the one
   that instantiates the program. `find_bound_task` then uses it, and the
   new resource-globals lookup reuses it rather than repeating the walk.

## Design doc reference

None; no new design decision beyond the IEC visibility rule above (#1489 owns
the analyzer-side scoping question).

## File map

- `specs/plans/2026-09-30-resource-globals-codegen.md` (this plan, removed
  before merge)
- `compiler/codegen/src/program_globals.rs` (new)
- `compiler/codegen/src/compile.rs` (call the new module; `find_bound_task`
  uses `find_program_resource`)
- `compiler/codegen/src/lib.rs` (module declaration)
- `compiler/codegen/tests/it/end_to_end_resource_globals.rs` (new) and
  `compiler/codegen/tests/it/main.rs`
- `docs/reference/language/pous/resource.rst`,
  `docs/reference/language/variables/scope.rst`

## Tasks

- [ ] Prefactor: move global list construction to `program_globals.rs`,
      extract `find_program_resource`; tests stay green
- [ ] Failing end-to-end tests: resource global scalar read/write through
      `VAR_EXTERNAL`, initial value, persistence across scans, `STRING`,
      array, structure, coexistence with configuration globals
- [ ] Append the program's resource globals in `program_globals`
- [ ] Unit tests for `find_program_resource` / `program_globals`
- [ ] Docs: resource page and scope page mention resource globals
- [ ] `git rm` this plan; `cd compiler && just`; `cd specs && just`; docs build

## Out of scope

- Located resource globals (`AT %QW0`): they do not parse on `main` (#1932).
- Calling a global function block instance through `VAR_EXTERNAL` (#1903).
- Enforcing resource visibility in the analyzer (#1489).
