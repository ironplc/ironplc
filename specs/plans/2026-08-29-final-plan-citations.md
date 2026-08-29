# Remove the Remaining Plan Citations

**Goal:** Remove the last 19 `specs/plans/` citations, closing Phase 1 of
#1464. After this, nothing outside `specs/plans/` itself references a plan.

**Architecture:** Two citations repoint — one to `specs/design/mcp-server.md`,
three to the existing issue #1439. The other sixteen are deleted: each comment
already states its fact in full.

**Issue:** #1464 (Phase 1)

**Design doc reference:** `specs/design/mcp-server.md` §`run`

---

## Scope

19 sites. This branch is cut from `main`, which does not yet carry #1479, so
21 of the 37 citations visible here belong to that PR and **must not be touched**
— editing them would duplicate merged work and conflict. The overlapping files
are `analyzer/src/stages.rs`, `codegen/tests/it/end_to_end_struct.rs`,
`analyzer/src/xform_resolve_expr_types.rs` and `project/src/project.rs`, where
this slice's sites are far from #1479's.

| Plan | Sites |
|---|---|
| `2026-08-28-method-scoping-and-scope-paths` | 4 |
| `2026-08-01-fb-call-style-initializer-distinct-node` | 3 |
| `2026-08-02-partial-resolution-revert-on-unrelated-error` | 2 |
| `vscode-marketplace-extension-id` (workflows) | 2 |
| `twincat-status` (dangling) | 1 |
| `2026-08-04-compatibility-libraries` | 1 |
| `2026-08-16-array-element-type-decl-order` | 1 |
| `2026-08-16-array-of-struct-field-codegen` | 1 |
| `2026-04-25-shared-symbol-extractors` | 1 |
| `2026-04-23-mcp-run-tool` | 1 |
| `2026-06-12-opencode-integration-e2e` | 1 |
| `library-e2e-cross-platform-fix` (justfile) | 1 |

## Prefactoring

None. No design document needs correcting to make a repoint honest.

The one document that *is* wrong — `mcp-server.md` §`run`, which specifies the
Phase 11 surface as normative requirements the code rejects at runtime — is
filed as #1480 rather than fixed here. Correcting it means deciding what nine
ignored stub conformance tests should be, which is a larger question than a
citation slice should answer.

## Triage

**Repoint (4):**

- `mcp/src/tools/run.rs` → `specs/design/mcp-server.md` §`run`, plus #1480 for
  the gap between what that section specifies and what the tool implements.
- The three METHOD-scoping test-section headers already cite issue #1439
  alongside the plan; the plan line goes and the issue link stays.

**Delete (15)** — the comment is self-contained. Notable ones:

- `dsl/src/common.rs` cites `specs/plans/twincat-status.md`, which **does not
  exist**. The prose around it is the valuable part: the `UDINT` ↔ `DWORD`
  implicit conversion is a verified exception to Beckhoff's documented rule,
  confirmed against a real TcXaeShell build, and deliberately scoped to that
  one pair. That belongs exactly where it is — beside the code implementing
  it — so only the dead link goes.
- `analyzer/src/extractors.rs` explains that without a shared traversal each
  front-end re-implements the same filtering and has diverged before. Complete
  on its own.
- `mcp/tests/cli.rs` explains the OpenCode failure mode — a boolean sub-schema
  in `properties` makes that client drop the entire tool list — in full.
- The two workflow comments explain that the two marketplaces are independent
  namespaces each needing its own VSIX and extension ID.
- The `justfile` comment explains that the verification script used to exist
  twice, drifted, and is now shared.

`symbol_environment.rs` keeps its explanation of why a scope is a path of
names rather than an AST node id; only the plan link goes.

## Deferred work

One issue filed: **#1480**. `mcp/src/tools/run.rs` refuses `stimuli`,
`container_base64`, the `tasks` filter and non-default trace modes, all of
which `mcp-server.md` specifies with REQ IDs. Their conformance tests
(REQ-TOL-mcp-040 through 048) are empty bodies marked `#[ignore]`, so the
bidirectional spec check reports coverage for a surface that is not built.
Parts are already tracked by #1163 and #1167.

## File map

- `compiler/analyzer/src/` — `stages.rs` (3), `symbol_environment.rs`,
  `extractors.rs`, `xform_resolve_symbol_and_function_environment.rs`,
  `rule_use_declared_symbolic_var.rs`, `xform_resolve_expr_types.rs`,
  `xform_toposort_declarations.rs`
- `compiler/parser/src/tests/var_declarations.rs`
- `compiler/plc2plc/src/tests/declarations.rs`
- `compiler/dsl/src/common.rs`
- `compiler/project/src/project.rs`
- `compiler/codegen/tests/it/end_to_end_struct.rs`
- `compiler/mcp/src/tools/run.rs`, `compiler/mcp/tests/cli.rs`
- `.github/workflows/deployment.yaml`,
  `.github/workflows/partial_upload_release_artifacts.yaml`
- `justfile`

## Tasks

- [ ] Repoint the MCP `run` citation and the three METHOD-scoping citations
- [ ] Delete the 15 self-contained citations, including the dangling one
- [ ] Confirm no `specs/plans/` reference remains outside `specs/plans/`
- [ ] `cd compiler && just` passes
- [ ] Delete this plan file before merge

## Note

Per the process introduced in #1456, this file is deleted before its own PR
merges. Its content is reviewable in the commit that adds it.
