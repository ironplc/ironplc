# Remove TwinCAT OOP Plan Citations

**Goal:** Remove the 29 `specs/plans/` citations in the TwinCAT OOP cluster,
repointing each at the durable document that owns the rationale or deleting it
where the surrounding comment already stands alone.

**Architecture:** Three existing documents already own this material:
`specs/design/beckhoff-twincat-dialect.md` (dialect parsing scope),
`specs/adrs/0041-staged-method-and-interface-dispatch.md` (dispatch decisions),
and `ironplc_dsl::extension::LanguageExtension` (the in-code marker). Citations
move to those. Where a comment fully states its own fact, the citation is
deleted outright.

One design correction is required first: the design document describes
behaviour the implementation does not have.

**Issue:** #1464 (Phase 1)

**Design doc reference:** `specs/design/beckhoff-twincat-dialect.md` §1.3, §1.4

---

## Scope

29 citation sites across five crates, covering five plans:

| Plan | Sites |
|---|---|
| `2026-07-18-twincat-extends-implements-interface.md` | 14 |
| `2026-07-20-twincat-extends-field-inheritance.md` | 7 |
| `2026-08-12-oop-method-declarations-static-dispatch.md` | 6 |
| `2026-07-20-twincat-extends-duplicate-field.md` | 1 |
| `2026-07-20-twincat-abstract-instantiation.md` | 1 |

The remaining 48 sites (DAP, codegen loops, MCP, and miscellaneous) are
separate slices under #1464.

## Prefactoring

`specs/design/beckhoff-twincat-dialect.md` §1.3 states the parser "recognizes
`INTERFACE name ... END_INTERFACE` as a top-level declaration containing method
signatures (method declarations without bodies)." The implementation parses
only the interface header; method and property signatures are not parsed, and
interfaces are modelled as empty structures purely so that variables declared
with an interface type resolve.

Eleven of the citations in this slice exist because that limitation was
recorded in a plan instead of in the design document. Correcting §1.3 to
describe what shipped is the prefactoring — without it, repointing those
comments would send readers to a document that contradicts the code.

The design document also omits the `.TcIO` file extension; it mentions only
the `<Itf>` XML element. `compiler/sources/src/file_type.rs` is the only place
that fact is written down.

## Triage

Each citation resolves one of three ways:

- **Repoint to the design document** — comments describing what the dialect
  parses and what it does not.
- **Repoint to ADR-0041** — comments describing dispatch behaviour or the
  staged Phase 1 / Phase 2 split.
- **Delete** — comments that already state their fact in full, where the
  citation only recorded where the decision was originally written down.

Comments recording deferred implementation work (method calls in expression
position, STRING/WSTRING method returns, inherited-field storage in codegen,
`<Method>` XML wiring) keep their prose: the deferral is documented by the
comment itself, so removing the plan link loses nothing.

## File map

**Modify — design:**

- `specs/design/beckhoff-twincat-dialect.md` — §1.3 as-shipped interface
  parsing scope; `.TcIO` file extension

**Modify — citations:**

- `compiler/analyzer/src/` — `rule_unsupported_extension.rs` (4),
  `xform_resolve_type_decl_environment.rs` (2),
  `intermediates/inherited_fields.rs`, `rule_use_declared_symbolic_var.rs`,
  `xform_resolve_expr_types.rs`, `xform_toposort_declarations.rs`,
  `rule_extends_field_duplicated.rs`, `rule_abstract_not_instantiated.rs`
- `compiler/parser/src/` — `parser.rs` (2), `tests/common.rs` (2),
  `tests/fb_inheritance.rs`, `tests/methods.rs`
- `compiler/dsl/src/` — `common.rs` (2), `extension.rs`, `textual.rs`
- `compiler/plc2plc/src/` — `renderer.rs`, `tests/fb_inheritance.rs`,
  `tests/methods.rs`
- `compiler/sources/src/` — `twincat_parser.rs`, `twincat_parser/tests.rs`,
  `file_type.rs`, `xml/transform.rs`
- `compiler/codegen/src/` — `compile_stmt.rs`, `compile_method.rs`

## Tasks

- [ ] Correct `beckhoff-twincat-dialect.md` §1.3 to describe the as-shipped
      interface parsing scope, and record the `.TcIO` extension
- [ ] Repoint or delete the 14 `twincat-extends-implements-interface` citations
- [ ] Repoint or delete the 7 `twincat-extends-field-inheritance` citations
- [ ] Repoint or delete the 6 `oop-method-declarations-static-dispatch`
      citations
- [ ] Repoint or delete the `twincat-extends-duplicate-field` and
      `twincat-abstract-instantiation` citations
- [ ] Confirm no citation in this cluster remains
- [ ] `cd compiler && just` passes
- [ ] Delete this plan file before merge

## Note

Per the process introduced in #1456, this file is deleted before its own PR
merges. Its content is reviewable in the commit that adds it.
