# Remove DAP Plan Citations

**Goal:** Remove the 8 `specs/plans/` citations in the DAP and debug-info
cluster, repointing each at `specs/design/debugger-support.md` or deleting it
where the surrounding comment already stands alone.

**Architecture:** `specs/design/debugger-support.md` already owns every fact
these citations reach for — the v1 scope cuts, the single-threaded loop, the
launch preconditions, the debug section tag registry, and the codegen source
position tracking. `compiler/vm-cli/src/dap/launch.rs` already cites it
alongside the plan, which is the pattern the rest should follow.

**Issue:** #1464 (Phase 1)

**Design doc reference:** `specs/design/debugger-support.md`

---

## Scope

8 citation sites across three plans:

| Plan | Sites |
|---|---|
| `2026-06-25-dap-server-scaffold.md` | 5 (`dap/mod.rs`, `dap/state.rs`, `dap/types.rs`, `dap/launch.rs`, `dap_main.rs`) |
| `2026-05-22-debug-source-file-table.md` | 2 (`vm-cli/tests/cli.rs`) |
| `2026-04-07-debug-source-map-and-hook.md` | 1 (`codegen/src/emit.rs`) |

## Prefactoring

None needed. Unlike the TwinCAT cluster, `debugger-support.md` is accurate and
already carries the relevant sections:

- §"v1 Scope Decisions" and §"Single-threaded DAP loop (v1)" — the state
  machine and `requestNotApplicable` behaviour
- §"Multi-instance: not supported in v1" — the second launch precondition
- §"Tag Registry" and §"Sub-table Payload Formats" — SOURCE_FILE_TABLE (tag 6)
- §"Source Position Tracking" and §"Emitter API Additions" — the codegen line
  map

Its six plan citations were already removed in #1456.

## One stale comment to correct

`compiler/codegen/src/emit.rs:166` says the line-map APIs are "scaffolding for
the source-map work" and that "the consumer in compile_stmt / compile_fn lands
in a follow-up". That follow-up shipped:

- `compile_stmt.rs:91` calls `set_source_position` for each statement
- `compile.rs` carries entries through `take_line_map` → `remap_line_map` →
  `add_line_map_entries` into the container builder
- `codegen/tests/it/end_to_end_debug_line_map.rs` asserts LINE_MAP and
  SOURCE_FILE_TABLE contents end to end against real source

The comment describes the feature as unfinished when it is wired and tested.
Removing the citation without correcting that framing would leave a false
statement behind, so the comment is rewritten to describe what the APIs do.

No deferred work in this cluster needs an issue: the one comment claiming a
deferral was stale, and the remaining "later commit" notes in `dap/mod.rs` and
`dap/state.rs` describe DAP phases already tracked in `debugger-support.md`.

## Triage

- **Repoint** — comments describing v1 scope, the DAP phase split, launch
  preconditions, or debug-section layout.
- **Delete** — comments that already state their fact in full.
- **Rewrite** — `emit.rs`, per above.

## File map

- `compiler/vm-cli/src/dap/mod.rs`, `dap/state.rs`, `dap/types.rs`,
  `dap/launch.rs`, `dap_main.rs`
- `compiler/vm-cli/tests/cli.rs` (2)
- `compiler/codegen/src/emit.rs`

## Tasks

- [ ] Repoint or delete the 5 `dap-server-scaffold` citations
- [ ] Repoint or delete the 2 `debug-source-file-table` citations
- [ ] Rewrite the `emit.rs` comment to drop the stale "follow-up" framing
- [ ] Confirm no citation in this cluster remains
- [ ] `cd compiler && just` passes
- [ ] Delete this plan file before merge

## Note

Per the process introduced in #1456, this file is deleted before its own PR
merges. Its content is reviewable in the commit that adds it.
