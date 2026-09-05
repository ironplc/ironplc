# Plan: Aggregate Variable Inspection in the Debugger

## Goal

Let a user paused in the debugger expand a `STRUCT`, `ARRAY` or
`FUNCTION_BLOCK` instance variable and see its fields and elements, by name,
with typed values, to any nesting depth. Today such a variable renders as
its type name in angle brackets and cannot be opened.

This plan covers the design PR only. The design is the deliverable; the
implementation is broken into prefactor PRs and core-change PRs that follow
once the design is approved.

## Architecture

The debug section gains type-directed layout tables: one description per
aggregate *type* (fields with byte offsets, or element type with bounds and
stride) and, on each variable's `VarNameEntry`, a reference to its type and
the static data-region offset of its contents. A reader walks the type tree
lazily and reads leaf values from the data region. Nothing changes in the
VM, the verifier, or the instruction set.

Design: [Variable Inspection Model](../design/variable-inspection-model.md).
Decision record: [ADR-0049](../adrs/0049-type-directed-debug-layout-for-aggregates.md).

This supersedes the draft in pull request #1115, which described the same
approach before the shared `VariableRenderer`, the aggregate type tags and the
DAP `variables` path landed, and whose review feedback (fold the type
reference and static offset into `VarNameEntry`, drop the `inline` flag,
partition the type-id space, add an enumeration kind, source stdlib FB fields
from the codegen field maps, reuse free tags) is applied here.

## Prefactoring

None in this PR: it is documentation only. The prefactors below are what the
design needs before the feature can drop in; each is its own PR and each is
behaviour-preserving.

1. **Split `compiler/container/src/debug_section.rs`** (1167 lines, already
   over the 1000-line limit) into one module per sub-table before two more
   sub-tables are added.
2. **One place that records a variable's debug entry.** `assign_variables`,
   `compile_fn` (parameters, locals, return variable) and
   `compile_user_function_block` each build a `VarNameEntry` and each copy the
   STRING data-region allocation verbatim. Extract one allocation-and-record
   helper so the new `type_ref` and `data_offset` fields are set in one place.
3. **Carry the declared type name on `IntermediateStructField`** in the
   analyzer. Field type names are needed for display and for enumeration
   fields; today only the resolved `IntermediateType` survives.
4. **Derive the stdlib FB field maps from the analyzer definitions.**
   `timer_fb_fields()` and friends in `compile_call.rs` restate names and
   indices that `stdlib_function_block.rs` already declares with types and
   sections. One source, and it carries the types the layout table needs.
5. **A `variablesReference` handle table in the DAP server**, replacing the
   two constants, so expanding a node is allocating a handle.

## File map

- `specs/design/variable-inspection-model.md` (new): the design
- `specs/adrs/0049-type-directed-debug-layout-for-aggregates.md` (new): the decision
- `specs/design/bytecode-container-format.md`: tag registry, `VarNameEntry`,
  array-descriptor and versioning notes point at the design
- `specs/design/debugger-support.md`: gap analysis rows 5 and 6 closed, tag
  registry corrected, FB field-name section replaced, `DebugInfo` sketch and
  formatting table updated
- `specs/design/variable-value-rendering.md`: aggregate rendering rules
- `specs/design/structure-codegen-memory-layout.md`,
  `specs/design/debug-info-in-iplc-container.md`: deferred items resolved

## Tasks

Design PR (this branch):

- [x] Write the design and the ADR
- [x] Reconcile the existing design documents
- [ ] Review; close #1115 in favour of this PR
- [ ] Open the tracking issue with the PR sequence below
- [ ] `git rm` this plan before merge

Prefactor PRs (one each, merged individually):

- [ ] P1 split `debug_section.rs` by sub-table
- [ ] P2 one allocation-and-record helper for variable declarations
- [ ] P3 declared type name on `IntermediateStructField`
- [ ] P4 stdlib FB field maps derived from analyzer definitions
- [ ] P5 DAP `variablesReference` handle table

Core-change PRs:

- [ ] C1 container: `TypeRef`, `COMPOSITE_TYPE`, `ARRAY_TYPE`, revised
      `VarNameEntry`, `STRING_LAYOUT` retired, format version 4; renderer
      reads strings and enumerations through the new fields; no aggregate
      expansion yet, every existing rendering unchanged
- [ ] C2 codegen emits the layout tables for structures, arrays, arrays of
      structures, stdlib and user FB instances, and function-local aggregates,
      with the consistency tests against `ArrayDescriptor` and the codegen
      field offsets
- [ ] C3 renderer tree API; `--dump-vars` writes one line per leaf with the
      full IEC path; end-to-end tests
- [ ] C4 DAP expandable variables with paging and `evaluateName`;
      `docs/reference/editor/debugging.rst` updated
- [ ] C5 (optional) playground and LSP run panel show the tree
