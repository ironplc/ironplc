# Scoped Variable Dump for `ironplcvm`

## Goal

Add an opt-in `--group-by-scope` flag to `ironplcvm run` that renders the
`--dump-vars` output grouped by owning POU (frame) and annotated with each
variable's IEC section, using the `function_id` + `var_section` metadata
now present in the debug section (shipped in #1106). The default
`--dump-vars` output is unchanged — the flat, spec-locked format stays as
is.

Today's flat dump:

```
acc: 0
counter: 3
result: 6
n: 3
bump: 11
add_offset: 14
step: 3
total: 6
```

With `--group-by-scope`:

```
[Globals]
  counter : DINT = 3
  result : DINT = 6
  acc : ACCUMULATOR = 0
[add_offset]
  n : DINT = 3  (VAR_INPUT)
  bump : DINT = 11  (VAR)
  add_offset : DINT = 14  (VAR_OUTPUT)
[accumulator]
  step : DINT = 3  (VAR_INPUT)
  total : DINT = 6  (VAR_OUTPUT)
```

## Why now

The user wants to *try out* the debug-info work. #1106 added per-function
`VarNameEntry`s (owner `function_id` + `var_section`), but the only tool
that reads variables — `ironplcvm run --dump-vars` — still prints a flat,
index-ordered list that ignores both fields. Surfacing the scope metadata
in the dump makes the merged work directly visible and gives a concrete
artifact to exercise, without waiting for the DAP/VS Code stack.

It is also a natural rehearsal for the DAP `scopes` request, which groups
variables by the same `var_section` → scope mapping
(`specs/design/debugger-support.md`).

## Constraints (must not break)

- **REQ-VC-005 / REQ-VC-008 / REQ-VC-009** in `specs/design/vm-cli.md`
  pin the *flat* dump format: one variable per line, ascending by index,
  `<name>: <value>` (or `var[<index>]: <raw_i32>`). These have `spec_test`
  conformance tests. The grouped output is therefore a **new, opt-in
  mode**, not a change to the default. The default path stays byte-for-byte
  identical.
- `VarDebugInfo` / `build_var_debug_map` (in `compiler/container/src/
  debug_format.rs`) are shared by `vm-cli`, `ironplc-cli` (lsp_runner),
  and `playground`, and are constructed in their tests. To avoid a
  cross-crate blast radius, the scoped renderer reads
  `container.debug_section` (`var_names`, `func_names`) **directly** in
  `vm-cli` rather than extending the shared struct.

## Scope

In:

- `compiler/vm-cli/src/main.rs`: add `--group-by-scope` bool to the `Run`
  subcommand; thread it into `cli::run`.
- `compiler/vm-cli/src/cli.rs`: thread the flag through
  `dump_variables_stopped` / `dump_variables_faulted`; add a
  `dump_variables_scoped` renderer and supporting helpers
  (`var_section_name`, group building).
- `specs/design/vm-cli.md`: document `--group-by-scope` and add new REQ
  IDs for the grouped format (the flat REQs are untouched).
- Tests: unit tests for the scoped renderer + grouping/ordering.

Out (deferred):

- Changing or replacing the default flat dump.
- Showing FB *instance* sub-fields (needs FB field-name tables, Phase 5).
- Live per-frame inspection (that is the DAP debugger, later phases).
- Reading the *call stack* at the dump point — the dump reflects the
  variable table after the VM stops at a scan boundary, where no frames
  are live. Grouping is by *declaring* POU, not a runtime frame snapshot.

## Architecture

### Flag plumbing

`Action::Run` gains `#[arg(long)] group_by_scope: bool`. `main` passes it
to `cli::run(path, dump_vars, scans, group_by_scope)`. `run` forwards it to
the two dump functions. When `false`, behavior is exactly as today.

### Grouping

A scoped group is `(function_id, label, Vec<VarRow>)` where `VarRow` holds
`var_index`, `name`, `type_name`, `iec_type_tag`, `var_section`, and the
raw value read from the stopped/faulted VM.

Build order:

1. **Globals** first: all `var_names` whose `function_id ==
   FunctionId::GLOBAL_SCOPE`, labeled `[Globals]`.
2. Then each distinct non-global `function_id` that appears in
   `var_names`, in ascending id order, labeled with its `func_names` entry
   (fallback `[function <id>]` if absent).

Within a group, rows are ordered by `var_index` (which matches
declaration order: params, locals, return).

Each row: `  <name> : <type_name> = <value>` with the value formatted via
the existing `format_variable_value(raw, iec_type_tag)`. For non-global
sections, append `  (<SECTION>)`; globals omit the annotation (it is
implied by the group). The section name comes from a small
`var_section_name(u8) -> &str` map (VAR, VAR_TEMP, VAR_INPUT, …).

### Fallback when no debug info

If the container has no debug section, or `var_names` is empty,
`--group-by-scope` falls back to the existing flat renderer (so the flag
is always safe to pass). A variable index present in the VM but absent
from `var_names` is not expected in a debug build; if it occurs, it is
appended under an `[Unmapped]` group using the flat `var[i]` form so no
value is silently dropped.

### Value reads

Unchanged: `stopped.read_variable_raw(VarIndex)` /
`faulted.read_variable_raw(...)`. The scoped renderer reads the same
slots; it only changes layout, not which bytes are read.

## File map

Modified:

- `compiler/vm-cli/src/main.rs` — `--group-by-scope` arg + call wiring.
- `compiler/vm-cli/src/cli.rs` — flag param on `run` and the two dump
  helpers; new `dump_variables_scoped`, `build_scope_groups`,
  `var_section_name`, `write_scoped_group`.
- `specs/design/vm-cli.md` — `--group-by-scope` option row + new REQ-VC
  IDs for the grouped layout.

Not modified:

- `compiler/container/src/debug_format.rs` — shared helpers untouched.
- Flat dump path / its REQ-VC conformance tests.
- VM crate.

## Tests

`vm-cli` unit tests (alongside the existing `write_variable_line` tests):

- `var_section_name_when_known_section_then_returns_iec_name`
- `build_scope_groups_when_globals_and_functions_then_globals_first_then_by_function_id`
- `build_scope_groups_when_rows_then_ordered_by_var_index_within_group`
- `write_scoped_group_when_global_then_omits_section_annotation`
- `write_scoped_group_when_function_local_then_includes_section_annotation`
- `dump_scoped_when_no_debug_section_then_falls_back_to_flat`

Spec conformance:

- New REQ IDs (e.g. REQ-VC-014..016) for: grouped layout shape, globals
  group first, section annotation on non-globals. Bound with `spec_test`.
- The existing REQ-VC-005/008/009 flat-format tests remain and must still
  pass (default path unchanged).

Manual end-to-end (recorded in the PR description, not a test):

```
ironplcc compile demo.st --output demo.iplc
ironplcvm run demo.iplc --dump-vars - --scans 3 --group-by-scope
```

against a program with a user FUNCTION and FUNCTION_BLOCK.

## Out of scope

- A separate `inspect` subcommand for static debug-section dumps (the
  alternative considered; can come later if useful).
- Trace/step execution modes (a different debugger increment).
- Grouping by DAP scope buckets (Inputs/Outputs/Locals) rather than by
  IEC section annotation — the per-section annotation already conveys the
  same information and maps cleanly when the DAP server lands.
