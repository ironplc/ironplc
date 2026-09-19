# Plan: Every P9999 from codegen points at the IEC 61131-3 source

Issue: https://github.com/ironplc/ironplc/issues/1734

## Goal

A P9999 (NotImplemented) diagnostic raised while compiling a program should
carry the span of the construct the compiler cannot handle yet, so the CLI
renders `┌─ file.st:7:10` with the offending line instead of `┌─ :1:1` with
an empty file name, and an embedding tool can place the problem in the editor.

The `Diagnostic::todo()` constructor labels its problem with
`SourceSpan::default()`. Fifteen codegen sites still call it, and a handful
more in the analyzer and PLCopen XML importer do too. The issue also lists two
`Diagnostic::internal_error()` sites and a `build_struct_fields(…,
&SourceSpan::default())` call in codegen that have the same defect.

The issue's second question (should `check` also run codegen?) is out of scope:
`check` stays parse + analysis. It is expected that `compile` rejects programs
`check` accepts while the compiler is still being built.

## Architecture

- **Remove `Diagnostic::todo()`** (the span-less variant). Every site converts
  to `Diagnostic::not_implemented(Label::span(span, "what is here"))`, or to
  `Diagnostic::todo_with_span(span)` when there is nothing more specific to
  say than "not implemented". With the constructor gone, a new span-less P9999
  cannot compile, which is the same prevention approach used for
  `Problem::NotImplemented` itself.
- **Thread a span** into the three helpers that raise P9999 without one in
  scope: `compile_body` (takes the POU name's span), `array_spec_from_named` /
  `intermediate_type_to_name` (the declaration span the caller already holds),
  and `initialize_struct_fields` (the declaration span, which it forwards to
  `build_struct_fields` for nested structures).
- **Use the span in scope** everywhere else: `expr.span()` for the
  resolved-type helpers, `selector_expr.span()` for the CASE arms,
  `assignment.target.span()` for a dereferenced store, the enumerated value's
  span for an array initializer, the return-type keyword span for
  STRING-returning functions and methods, `decl.identifier.span()` for the
  late-resolved-type invariant, and the first program name for the
  "fewer than two programs" invariant.
- Labels describe what is at the location (per `diagnostic.rs` guidelines),
  e.g. "CASE selector is not an integer type", not how to fix it.

## Prefactoring

Thread the spans through `compile_body`, `array_spec_from_named`,
`intermediate_type_to_name` and `initialize_struct_fields` first, in its own
commit, with no change to which diagnostics are produced. Once every P9999 site
has a span in scope, the conversion is a one-line change per site.

## Design doc reference

None. `specs/steering/problem-code-management.md` documents the constructors
and is updated to drop the `todo()` example.

## File map

- `compiler/dsl/src/diagnostic.rs` — remove `todo()` and its test.
- `compiler/codegen/src/compile_stmt.rs`, `compile_expr.rs`, `compile_call.rs`,
  `compile_array.rs`, `compile_fn.rs`, `compile_method.rs`, `compile.rs`,
  `compile_setup.rs`, `compile_struct.rs` — spans threaded and sites converted.
- `compiler/analyzer/src/xform_toposort_declarations.rs`,
  `rule_var_decl_global_const_requires_external_const.rs`,
  `intermediates/structure.rs` — sites converted.
- `compiler/sources/src/xml/transform.rs` — sites converted (labelled with the
  file, since PLCopen XML type nodes carry no span).
- `compiler/codegen/tests/it/compile_case.rs` (or a new test module) — a CASE
  on a REAL selector and a direct-address write both report P9999 at the
  construct.
- `specs/steering/problem-code-management.md` — constructor guidance.

## Tasks

- [ ] Commit this plan.
- [ ] Prefactor: thread spans into `compile_body`, `array_spec_from_named`,
      `intermediate_type_to_name`, `initialize_struct_fields`.
- [ ] Convert every `Diagnostic::todo()` site; remove `todo()`.
- [ ] Convert the two codegen `internal_error()` sites and the
      `build_struct_fields(…, &SourceSpan::default())` call.
- [ ] Add codegen tests asserting the primary label lands on the construct.
- [ ] Update `problem-code-management.md`.
- [ ] `cd compiler && just`; delete this plan.
