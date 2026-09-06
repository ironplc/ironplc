# Plan: the parser records a user-typed initializer; the resolver classifies it

## Goal

Stop the parser guessing what a user-defined type is. Today `x : T := (a := 1)`
becomes a `Structure` initializer and `x : T := Red` becomes an `EnumeratedType`
initializer before anyone knows what `T` is, so a function-block instance
declared with a member initializer reaches every later pass mis-shaped, and an
alias initialized from a named constant is read as an enum default. The
placeholder the parser already uses for `x : T;` (`LateResolvedType`) should
carry the initializer too, and the type resolver, which already turns the
placeholder into the concrete kind for the bare declaration, should do the same
when an initializer is present.

## Architecture

- `InitialValueAssignmentKind::LateResolvedType` carries a
  `LateResolvedInitializer { type_name, initial_value: Option<...> }` where the
  optional value is the syntax as written: `Members(Vec<StructureElementInit>)`
  for `(a := 1, ...)` or `Value(Id)` for a bare identifier.
- The parser emits the placeholder wherever the type is a user name and the
  initializer shape is ambiguous. It still emits `EnumeratedType` for a
  qualified value (`T := T#Red`) and `EnumeratedValues` for an inline
  enumeration, both of which are unambiguous. After this change the parser never
  constructs `Structure` or a named `EnumeratedType` from a bare identifier.
- `xform_resolve_late_bound_type_initializer` maps type kind and initializer
  shape to the concrete kind in one place: a function block with members becomes
  `FunctionBlock` with `init`; a structure with members becomes `Structure`; an
  enumeration with a value becomes `EnumeratedType`; any other known type with a
  value becomes a constant-expression initializer (`SimpleExpr`) for the
  existing fold pass to evaluate or diagnose. An unknown type is diagnosed as
  the bare placeholder already is.
- Codegen reports a member initializer on a function-block instance as not
  implemented instead of allocating the instance and dropping the values.

## Prefactoring

None beyond what #1647 already did. Every consumer of the placeholder reads
its type name and nothing else; they change from destructuring a `TypeName` to
reading a field.

## Design doc reference

ADR-0050 records the decision. No design document changes.

## File map

- `compiler/dsl/src/common.rs`, `visitor.rs`, `fold.rs` -- the placeholder
  payload and its traversal
- `compiler/parser/src/parser.rs`, `vars.rs` -- emit the placeholder
- `compiler/plc2plc/src/renderer.rs` -- render it back to source
- `compiler/analyzer/src/xform_resolve_late_bound_type_initializer.rs` -- the
  classification
- `compiler/analyzer/src/{intermediates/structure,rule_ref_to,
  rule_assignment_aggregate_type_compat,variable_type,
  xform_resolve_expr_types,xform_resolve_late_bound_expr_kind,
  xform_toposort_declarations}.rs` -- read the type name from the payload
- `compiler/codegen/src/compile_setup.rs` and a codegen test -- the guard
- parser and analyzer tests
- `specs/adrs/0050-*.md`

## Tasks

- [ ] Commit this plan
- [ ] DSL payload and traversal
- [ ] Parser emits the placeholder; parser tests follow
- [ ] Consumers read the type name from the payload
- [ ] Resolver classifies members and values; tests for each mapping
- [ ] Renderer prints the placeholder; plc2plc round trips pass
- [ ] Codegen guard and test
- [ ] ADR
- [ ] `cd compiler && just`; `git rm` this plan
