# Remove TwinCAT Non-OOP Plan Citations

**Goal:** Remove the 21 `specs/plans/` citations covering TwinCAT dialect
features outside the OOP cluster, plus the LSP multi-workspace-folder work.

**Architecture:** Four citations describe features the design document already
covers and repoint to it. The other seventeen are deleted: each comment already
states its own fact in full, and the citation recorded only where the decision
was first written down.

**Issue:** #1464 (Phase 1)

**Design doc reference:** `specs/design/beckhoff-twincat-dialect.md` §3.3, §3.4

---

## Scope

21 citation sites across nine plans:

| Plan | Sites |
|---|---|
| `twincat-case-label-bit-string-literals` | 4 |
| `twincat-var-initializer-expressions` | 4 |
| `twincat-struct-init-expression-value` | 3 |
| `twincat-lsp-multi-workspace-folder` | 3 |
| `twincat-and-then-operator` | 3 |
| `twincat-empty-case-branch` | 1 |
| `twincat-mixed-located-var-declarations` | 1 |
| `twincat-pragma-skipping` | 1 |
| `twincat-initializer-dialect-and-fold-revert-fixes` | 1 |

## Prefactoring

None. The design document is accurate for the two features being repointed —
§3.3 Pragma Attributes and §3.4 Short-Circuit Boolean Operators both describe
what shipped.

The document does **not** cover bit-string CASE labels, empty CASE branches,
mixed located variable declarations, constant-expression VAR initializers, or
struct initializer expressions. That is not a defect to fix here: it is a design
document for a body of work, not a living feature registry, and every one of
those features is gated behind an `--allow-*` flag that
`docs/explanation/enabling-dialects-and-features.rst` and the flag table in
`specs/steering/syntax-support-guide.md` already document. The comments for
those features name their flag, which is the useful pointer.

## Triage

**Repoint (4)** — comments about features the design document covers:

- `parser/src/tests/common.rs` pragma-skipping section → §3.3
- `parser/src/tests/common.rs` AND_THEN section → §3.4
- `analyzer/src/xform_resolve_expr_types.rs` AND_THEN section → §3.4
- `codegen/tests/it/compile_bool.rs` AND_THEN codegen refusal → §3.4 and #1476

**Delete (17)** — the surrounding comment is self-contained. These fall into
three shapes:

1. **Test-section headers** naming the feature under test — the citation was
   provenance, not information.
2. **Comments that fully explain their own design**, e.g. `parser.rs` on why
   the bit-string CASE alternative must be ordered after `signed_integer()`,
   and `stages.rs` on why the initializer fold must run before any pass that
   touches `SimpleExpr`.
3. **Module headers that already name the governing `--allow-*` flag**, which
   is a better pointer than a deleted plan.

The three LSP multi-workspace-folder citations are deleted rather than
repointed: that work is editor/project infrastructure, not a dialect feature,
and has no design document. The comments are section labels.

## Deferred work

Two genuine gaps found, neither previously tracked, both filed:

- **#1476** — `AND_THEN` parses and analyzes but codegen refuses it, because
  short-circuit evaluation needs conditional-branch codegen that does not
  exist. `OR_ELSE` has the same gap. Notable because the operator exists for
  guarded pointer access, so it is least available exactly where it is most
  needed.
- **#1477** — struct/FB-instance initializer expressions parse, round-trip and
  analyze, but codegen cannot evaluate them at instance construction time.

Both refusals are deliberate and pinned by tests. The comments describing them
are retained; only the plan links go.

## File map

- `compiler/parser/src/` — `tests/common.rs` (4), `tests/case.rs`,
  `tests/struct_init_expressions.rs`, `parser.rs`
- `compiler/dsl/src/textual.rs`
- `compiler/analyzer/src/` — `stages.rs` (3), `xform_resolve_expr_types.rs`
- `compiler/plc2plc/src/tests/struct_init_expressions.rs`
- `compiler/ironplc-cli/src/lsp_project.rs`
- `compiler/project/src/project.rs`
- `compiler/sources/src/project.rs`
- `compiler/codegen/tests/it/` — `compile_bool.rs`, `end_to_end_struct.rs`,
  `end_to_end_case.rs`, `end_to_end_constant_initializer_expressions.rs`,
  `end_to_end_mixed_located_var_declarations.rs`

## Tasks

- [ ] Repoint the 4 pragma and AND_THEN citations
- [ ] Delete the 17 self-contained citations
- [ ] Reference #1476 and #1477 from the two codegen-refusal comments
- [ ] Confirm no citation in this cluster remains
- [ ] `cd compiler && just` passes
- [ ] Delete this plan file before merge

## Note

Per the process introduced in #1456, this file is deleted before its own PR
merges. Its content is reviewable in the commit that adds it.
