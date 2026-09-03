# Plan: Reject more than one PROGRAM at code generation

Issue: [#1588](https://github.com/ironplc/ironplc/issues/1588)

## Goal

A library with more than one `PROGRAM` declaration, or a configuration that
instantiates more than one program, currently compiles silently to a container
holding only one of them (which one depends on toposort order). Turn that
silent wrong answer into a compile error at code generation, using the
existing P9999 (not implemented) problem code with labels that point at each
extra program. Actually running several programs is deferred to a separate
enhancement issue.

## Architecture

- `ironplc_codegen::compile::compile` already resolves "the" program via
  `find_program`. Replace it with a lookup that collects every
  `ProgramDeclaration`, keeps P4020 for zero, and returns
  `Diagnostic::not_implemented` for two or more. The primary label sits on the
  second declaration (ordered by source position, not toposort order) and
  says it is the second `PROGRAM`; secondary labels mark the first and any
  further declarations.
- The same check covers program *instances*: every `PROGRAM … WITH … : …`
  across all `RESOURCE` blocks of the configuration. Two instances of the
  same program type would also be dropped silently today.
- No new problem code. P9999 already means "valid, not implemented yet" and
  carries a custom primary label via `Diagnostic::not_implemented`.

## Prefactoring

None needed. The change replaces one small private function and adds one
sibling; no existing branching is duplicated.

## Design doc reference

`specs/design/61131-task-support.md` describes the eventual multi-program
model. This change does not alter it; the limitation is recorded in the
docs website and in the enhancement issue.

## File map

- `compiler/codegen/src/compile.rs` — replace `find_program`, add instance
  check, update the `apply_task_configuration` doc comment, add tests
- `docs/includes/single-program-limitation.rst` — shared admonition
- `docs/reference/language/pous/task.rst`,
  `docs/reference/language/pous/resource.rst`,
  `docs/explanation/execution-cycle.rst`,
  `docs/explanation/program-organization.rst` — include the admonition

## Tasks

- [ ] Open the enhancement issue for multi-program support
- [ ] Codegen: error on a second `PROGRAM` declaration with positional labels
- [ ] Codegen: error on a second program instance in the configuration
- [ ] Unit tests for both cases (two and three programs)
- [ ] Docs: shared limitation admonition on the affected pages
- [ ] `cd compiler && just`, `cd docs && just compile`, `cd specs && just`
- [ ] Delete this plan before merge
