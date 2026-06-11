# Layer 1 Debug Info: Function-Local Variable Entries

## Goal

Emit `VarNameEntry` debug records (debug section Tag 2, VAR_NAME) for the
parameters and local variables of user-defined `FUNCTION` and
`FUNCTION_BLOCK` bodies. Today these entries are emitted **only** for
program/global-scope variables; every parameter and local inside a user
function or FB body is invisible to the debug section. Each emitted entry
carries the owning `function_id`, the real IEC `var_section`
(VAR_INPUT / VAR_OUTPUT / VAR_IN_OUT / VAR / VAR_TEMP), the `iec_type_tag`,
the variable name, and the source-level `type_name`.

This is the one substantial remaining gap in Layer 1 of the debugger
design (`specs/design/debugger-support.md`). It closes Gap #2 (variable
scope info) for the non-global case, which is exactly what the DAP
`scopes` / `variables` requests need to filter the Variables pane to the
current stack frame.

## Why now

The call-depth arc (#1094, #1100, #1101) finished the VM prerequisite for
the debugger: iterative dispatch with an authoritative, header-sized frame
stack. Layer 1 debug info is the next foundation — every DAP request reads
from the debug section, and a paused frame's locals cannot be shown
without per-function `VarNameEntry`s.

Most of Layer 1 is already done:

- Tag 3 FUNC_NAME — emitted for INIT, SCAN, user functions, user FB bodies
  (`compile.rs:707`).
- Tag 1 LINE_MAP + Tag 6 SOURCE_FILE — wired end-to-end with BLAKE3 file
  hashes.
- Tag 2 VAR_NAME — emitted for **globals only** (`compile_setup.rs:283`).
- Tags 4/9 STRING_LAYOUT / ENUM_DEF — done.

The only meaningful hole is function-local `VarNameEntry`s. It is
self-contained, codegen-only (the container format already has the
`function_id` field and serializes it), and unblocks per-frame variable
inspection in the DAP work.

## Scope

In:

- `compiler/codegen/src/compile_fn.rs`: collect a `VarNameEntry` for each
  parameter, local, and the return variable as slots are assigned in
  `compile_user_function` / `compile_user_function_block`, tagged with the
  function's `FunctionId`.
- `compiler/codegen/src/compile_setup.rs`: lift `map_var_section` and
  `resolve_iec_type_tag` to `pub(crate)` (or move to a shared helper
  module) so `compile_fn.rs` can reuse them. No behavior change to the
  global path.
- Tests: codegen end-to-end coverage asserting that a program with a user
  function and a user FB produces per-function `VarNameEntry`s with the
  correct `function_id` and `var_section`.

Out (deferred):

- Tags 4/5 FB_TYPE_NAME / FB_FIELD_NAME (design Gaps #5/#6). Phase 5.
- Any DAP server, stepping, breakpoints, or VS Code work. Later phases.
- Container format / wire-encoding changes. The `function_id` and
  `var_section` fields already exist and round-trip
  (`debug_section.rs` unit tests cover it).
- Fine-grained per-expression line maps (design Gap #4).
- Rich type resolution for composite locals beyond what the global path
  already does. Function-local entries reuse the same best-effort
  resolution; unresolved composites fall back to `iec_type_tag::OTHER`
  with a best-effort `type_name`, matching the existing global behavior.

## Architecture

### Where slots are assigned today

`compile_fn.rs` assigns variable-table slots for a user function / FB body
in three groups:

1. Input-compatible parameters (VAR_INPUT, VAR_IN_OUT) — first, for CALL
   arg passing (`compile_fn.rs:97-160`).
2. Locals (VAR, VAR_TEMP) (`compile_fn.rs:163-225`).
3. The return variable, named after the function
   (`compile_fn.rs:227-230`).

Each group already has `decl` (with `decl.var_type` and
`decl.initializer`) and the assigned `current_index` in hand. This is the
natural collection point — the same place `compile_setup.rs:283` collects
the global entries.

### What to add

At each slot assignment, push a `VarNameEntry` into the existing
`ctx.debug_var_names` vector (the same vector the global path appends to;
it persists across the per-function `ctx.variables` save/restore, so
appends accumulate correctly):

```rust
ctx.debug_var_names.push(VarNameEntry {
    var_index: current_index,
    function_id: FunctionId::new(function_id), // the function being compiled
    var_section: map_var_section(&decl.var_type),
    iec_type_tag: tag,
    name: id.to_string(),
    type_name: type_name_str,
});
```

- `function_id` is the `function_id: u16` parameter already threaded into
  `compile_user_function` / `compile_user_function_block` — **not**
  `GLOBAL_SCOPE`. This is the field the DAP server uses to filter a
  frame's locals.
- `var_section` comes from the existing `map_var_section` helper.
- `iec_type_tag` / `type_name` are resolved from `decl.initializer` using
  the same logic the global path uses. To avoid duplicating the
  resolution match, factor the global path's "initializer → (tag,
  type_name)" computation (`compile_setup.rs:~230-281`) into a
  `pub(crate)` helper, e.g. `resolve_debug_type(initializer, types) ->
  (u8, String)`, and call it from both sites. If factoring proves noisy,
  the fallback is to reuse `resolve_iec_type_tag` for the `Simple` case
  and `OTHER` + best-effort name otherwise — identical to globals.

### Return variable section

The function result (named after the function) is not a normal IEC
section. Map it to `var_section::VAR_OUTPUT` — the result is conceptually
the function's output, and the design's section→DAP-scope table
(`debugger-support.md`) maps VAR_OUTPUT to the "Outputs" scope, which is
the right place for a function's return value. Document this choice in a
code comment so it's a deliberate decision, not an accident. FB bodies
have no return variable, so this only applies to functions.

### Ordering / invariant

`ContainerBuilder::build` does not require `var_names` to be sorted (only
the line map is sorted). Append order is fine. Entries for the same
`var_index` can legitimately appear under different `function_id`s because
the variable table is partitioned per function — the `(function_id,
var_index)` pair is what disambiguates, exactly as the design's
"Why function_id on variables" note describes.

## File map

Modified:

- `compiler/codegen/src/compile_fn.rs` — push `VarNameEntry` at the three
  slot-assignment sites (params, locals, return var); import the entry
  type and the section/type helpers.
- `compiler/codegen/src/compile_setup.rs` — make `map_var_section` /
  `resolve_iec_type_tag` (and, if extracted, `resolve_debug_type`)
  `pub(crate)`; no behavior change to the global emission.
- `compiler/codegen/tests/it/` — new end-to-end test module (see Tests).

Not modified:

- `compiler/container/src/debug_section.rs` — format already supports it;
  existing round-trip unit tests cover `function_id` / `var_section`.
- Builder API — `add_var_name` already exists and is already used by the
  global path.
- VM, DAP, VS Code — untouched.

## Migration

Single implementation commit (after the plan commit):

1. Extract / expose the section + type-resolution helpers in
   `compile_setup.rs`.
2. Add the three `VarNameEntry` pushes in `compile_fn.rs`.
3. Add tests.
4. `cd compiler && just` → must exit 0 (compile, coverage ≥ 85%, clippy,
   fmt).

If an existing codegen test that snapshots the debug section breaks, that
is the signal to update its expectation to include the new function-local
entries (the additions are purely additive — no existing entry changes).

## Tests

Codegen end-to-end (`compiler/codegen/tests/it/`), following the pattern
in `end_to_end_debug_line_map.rs` and `end_to_end_enum.rs`:

- `var_names_when_user_function_has_params_then_entries_have_function_id_and_sections`
  — a program with a `FUNCTION FOO : DINT` taking `VAR_INPUT a : DINT;`
  and a local `VAR t : BOOL;`. Assert the debug section contains entries
  for `a` (var_section = VAR_INPUT, function_id = FOO's id), `t`
  (var_section = VAR, same function_id), and the return var `FOO`
  (var_section = VAR_OUTPUT), each with the right `iec_type_tag` /
  `type_name`.
- `var_names_when_user_function_block_has_locals_then_entries_have_fb_function_id`
  — a `FUNCTION_BLOCK` with input/output/local vars; assert each emitted
  entry carries the FB body's `function_id` and correct sections.
- `var_names_when_global_and_function_locals_then_globals_keep_global_scope`
  — a program with both globals and a user function; assert globals still
  carry `GLOBAL_SCOPE` and function locals carry the function's id (no
  regression to the existing global path, no `var_index` collisions
  surfacing as wrong-scope lookups).
- `var_names_when_param_and_local_share_index_across_functions_then_disambiguated_by_function_id`
  — two user functions whose local partitions reuse the same `var_index`
  values; assert the `(function_id, var_index)` pairs are distinct.

These run through the real `compile()` path (not hand-built containers),
so they also exercise the container round-trip via `build()`.

## Out of scope

- DAP server, breakpoints, stepping, pause/resume, VS Code adapter.
- FB type/field name tables (Tags 4/5 per design). Phase 5.
- Showing FB *instance* fields (a global FB variable's sub-fields) in the
  Variables pane — that needs the FB field-name table and is a separate
  Phase 5 item; this PR covers variables that occupy variable-table slots
  during a function / FB body's own execution.
- Column-precise line maps and per-expression mapping (design Gap #4).
- Any change to how values are *formatted* — `debug_format.rs` already
  renders by `iec_type_tag`.
