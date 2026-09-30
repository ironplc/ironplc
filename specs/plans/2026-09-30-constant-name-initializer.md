# Plan: a variable initializer that is a bare constant name

Issue: #1890

## Goal

`x : UDINT := C;`, where `C` is a named `CONSTANT`, parses, is checked by the
analyzer against the constant's declared type and the variable's range, and
compiles to a variable that starts with the constant's value. Today it is a
syntax error (P0002) in every dialect, while `x : UDINT := C + 1;` parses.

## Architecture

The grammar already accepts an initializer expression after a simple
specification (`simple_spec_init`, used by `VAR_GLOBAL` and located
declarations). The rule used by ordinary `VAR` blocks and structure fields,
`simple_or_enumerated_or_subrange_ambiguous_struct_spec_init`, rejects a bare
identifier after *any* simple specification so that the enumerated-value
alternative can take it. That is right after a user type name (`T := Red` may
be an enumeration value, ADR-0050) and wrong after an elementary type, which
is a keyword and cannot be an enumeration.

1. **Parser.** One shared rule, `simple_spec_init__with_value`, used by both
   `simple_spec_init` and the ambiguous rule:
   - elementary type: the expression, as today (`Simple` for a literal,
     otherwise `SimpleExpr`);
   - user type name and a bare identifier: the ADR-0050 placeholder
     `LateResolvedType` with `LateResolvedInitialValue::Value`, which the
     type resolver already turns into an enumeration default or a
     `SimpleExpr`;
   - user type name and anything else: the expression, as today.

   This also fixes `VAR_GLOBAL ge : Color := Green;`, which today becomes a
   `SimpleExpr` and is reported as P4038 because `simple_spec_init` never
   asked the resolver.

2. **Analyzer** (`xform_fold_initializer_expressions`). The bare name is a
   constant expression, so it goes through the existing fold: P4037 without
   `--allow-constant-initializer-expressions` (the Edition 2 grammar allows
   only a literal), P4038 when the name is not a constant, otherwise the
   constant's value. Three changes:
   - a constant whose own initializer is a constant expression
     (`D : UDINT := C;`) is registered with its folded value, so it can be
     used in turn;
   - a substituted value carries the span of the reference, so a range
     problem (P2026) is reported where the constant is used, not where it is
     declared;
   - an initializer that is exactly one named constant is checked against the
     variable's type with the same compatibility relation as an assignment
     (`type_compat::are_types_compatible`), reporting P4022. The literal
     check alone accepts `x : INT := C` for a `UDINT` constant.

3. **Codegen.** Nothing to change: after the fold the initializer is an
   ordinary `Simple` literal.

## Prefactoring

`xform_fold_initializer_expressions.rs` is 743 lines, about 400 of them
inline tests. Move the tests to `xform_fold_initializer_expressions/tests.rs`
unchanged, so the new behaviour and its tests fit under the 1000-line limit.

## Design doc reference

ADR-0050 (the parser records a user-typed initializer; the resolver
classifies it). No design document covers initializer folding.

## File map

- `compiler/parser/src/parser.rs` — shared initializer rule
- `compiler/parser/src/tests/constant_name_initializers.rs` (new) — AST shape
- `compiler/analyzer/src/xform_fold_initializer_expressions.rs` — fold changes
- `compiler/analyzer/src/xform_fold_initializer_expressions/tests.rs` (moved)
- `compiler/analyzer/src/stages.rs` — pass the type environment to the fold
- `compiler/plc2plc/src/tests/` and `compiler/resources/test/` — round trip
- `compiler/codegen/tests/it/end_to_end_constant_name_initializer.rs` (new)
- `docs/reference/compiler/problems/P4022.rst`, `P4037.rst` — named constant
- user documentation of the extension where constant initializers are described

## Tasks

- [ ] Commit this plan
- [ ] Prefactor: move the fold tests to their own file
- [ ] Parser: failing tests, then the shared rule
- [ ] Analyzer: chained constants, reference span, type check (tests first)
- [ ] End-to-end tests in the default dialect with the flag and in TwinCAT
- [ ] plc2plc round trip
- [ ] Documentation
- [ ] Open issues for what is left out: a constant of an enumerated type used
      as an enumeration initializer (reported as P2006 today), structure-field
      defaults that name a constant (P9999, #1523/#1866), a `STRING`
      initializer naming a constant
- [ ] `git rm` this plan, `cd compiler && just`, `cd specs && just`, docs build
