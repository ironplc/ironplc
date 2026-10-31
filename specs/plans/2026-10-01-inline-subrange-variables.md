# Inline Subrange Variable Declarations

Plan for [#1966](https://github.com/ironplc/ironplc/issues/1966).

## Goal

Accept a variable declared with an inline subrange, with or without an
initial value, in every block whose grammar allows it:

```
VAR
    x : INT(0..15);
    y : INT(0..15) := 3;
END_VAR
```

Today both lines are a syntax error (P0002) in every dialect. A named
subrange (`TYPE R : INT(0..15); END_TYPE`, `x : R := 3;`) already works
end to end, and so does the inline form where the parser already accepts it
(`VAR_IN_OUT`, a structure field).

`check` reports an initial value outside the range (P2026, as for a named
subrange), `compile` stores the initial value or, without one, the lower
bound (IEC 61131-3 §2.4.3.1, REQ-SR-031), and `plc2plc` writes the
declaration back as written.

## Where the grammar allows it

IEC 61131-3 edition 2, Annex B.1.4.3:

- `var1_init_decl ::= var1_list ':' (simple_spec_init | subrange_spec_init | enumerated_spec_init)`
  is what `VAR`, `VAR_INPUT`, `VAR_OUTPUT`, `VAR RETAIN`/`NON_RETAIN`,
  `VAR CONSTANT` and (in IronPLC, which already accepts initial values there)
  `VAR_TEMP` use.
- `located_var_spec_init ::= simple_spec_init | subrange_spec_init | ...`
  is what `VAR_GLOBAL` and a located declaration (`AT %IW0 : ...`) use.
- `subrange_spec_init ::= subrange_specification [':=' signed_integer]`.
- `VAR_IN_OUT` (`var1_declaration`) and `VAR_EXTERNAL`
  (`external_declaration_spec`) take a `subrange_specification` with no
  initial value. `VAR_IN_OUT` already parses it. `VAR_EXTERNAL` takes only
  an array or a simple specification today (`TODO` in `parser.rs`); it is
  out of scope here and gets its own issue if wanted.

## Architecture

The AST cannot hold the initial value today:
`InitialValueAssignmentKind::Subrange(SubrangeSpecificationKind)` has no
place for it, and the structure field rule throws the parsed default away
(`subrange_spec_init__with_range()` returns it as `.1`, and the field keeps
only `.0`).

1. **Prefactor (behaviour-preserving).** Replace the variant's payload with
   a struct, the same shape as the other initializers:

   ```rust
   pub struct SubrangeInitialValueAssignment {
       pub spec: SubrangeSpecificationKind,
       pub initial_value: Option<SignedInteger>,
   }
   ```

   Every producer sets `initial_value: None`; every consumer reads `.spec`.
   The `plc2plc` renderer writes ` := value` when it is present (dead until
   the core change). No test changes.

2. **Core change.**
   - *Parser.* Add `subrange_spec_init__with_range()` as an alternative of
     `simple_or_enumerated_or_subrange_ambiguous_struct_spec_init()`, ahead
     of the bare `elementary_type_name()` alternative, so that `VAR`,
     `VAR_INPUT`, `VAR_OUTPUT`, `VAR_TEMP` and located declarations accept
     it; and add it to `located_var_spec_init()` for `VAR_GLOBAL`. The rule
     keeps its initial value. The integer type keyword followed by `(` is
     unambiguous, so no other alternative changes meaning. The new
     alternative reuses `subrange()`, so the radix bounds of #1967 apply
     here too once both land; the two diffs touch different lines.
   - *Analyzer.* `rule_constant_range::check_initializer` gains a `Subrange`
     arm that checks `initial_value` against the declaration's subrange
     type (the anonymous type `xform_resolve_decl_types` already assigns to
     an inline subrange), reporting P2026 as for a named subrange.
     `rule_var_decl_const_initialized` treats a `Subrange` with an initial
     value as initialized.
   - *Codegen.* The existing `Subrange` arm of `compile_setup` emits the
     initial value when present and the lower bound otherwise. Variable
     allocation already handles the inline form (REQ-SR-021).
   - *plc2plc.* Round trip comes from the prefactor's renderer change.

The default stays decided where it is decided today (codegen reads the lower
bound from the specification). Moving defaults into the analyzer is #1968's
concern; this change does not make that move harder, since the initial value
now lives in the AST next to the specification.

## Prefactoring

The `SubrangeInitialValueAssignment` struct above, as its own PR: without it
the core change would have to add an initial value by changing the variant's
shape and its ~15 match sites in the same diff as the new syntax. No other
prefactoring: the parser change is one alternative in two rules, and each
other consumer gains at most one line.

## Design doc reference

[`specs/design/subrange-codegen.md`](../design/subrange-codegen.md):

- REQ-SR-031 already specifies the inline default.
- REQ-SR-032 says an explicit initial value arrives as
  `InitialValueAssignmentKind::Simple`; that stays true for a named
  subrange. Add a requirement that an inline subrange's explicit initial
  value arrives in `SubrangeInitialValueAssignment::initial_value` and is
  emitted in place of the lower bound.

## Out of scope

- A structure field's inline subrange default (`f : INT(0..15) := 3;`) is
  parsed and dropped today: the field reads 0. After the prefactor the
  parser can keep it; making struct initialization use it belongs with
  #1523 / #1866.
- A `TYPE` subrange's default (`TYPE R : INT(0..15) := 20; END_TYPE`) is
  neither range-checked nor applied (#1554).
- `VAR_EXTERNAL x : INT(0..15);` (see above).
- Function parameters of an inline type in call signatures: #1981.

## Related open pull requests

- #1967 changes `subrange()` (radix bounds); this plan reuses the rule and
  does not edit it.
- #1932 rewrites `global_var_decl()`; this plan edits only
  `located_var_spec_init()`, which it calls.
- #1907 rewrites `simple_spec_init()`; not edited here.
- #1868 moves variable initialization out of `compile_setup.rs`, including
  its `Subrange` arm; whichever lands second carries the `.spec` /
  `initial_value` change over.

## File map

Prefactor PR:

- `compiler/dsl/src/common.rs` — new struct, variant payload, `TypeReference`.
- `compiler/parser/src/parser.rs`, `compiler/parser/src/vars.rs` — producers.
- `compiler/analyzer/src/{xform_resolve_decl_types,xform_resolve_expr_types,xform_resolve_late_bound_type_initializer,xform_resolve_late_bound_expr_kind,xform_resolve_type_decl_environment,xform_toposort_declarations,rule_var_decl_const_initialized}.rs`,
  `compiler/analyzer/src/intermediates/structure.rs` — consumers.
- `compiler/codegen/src/compile_setup.rs` — consumers.
- `compiler/plc2plc/src/renderer.rs` — render the optional initial value.

Core change PR:

- `compiler/parser/src/parser.rs` — the two rule alternatives.
- `compiler/parser/src/tests/inline_subrange.rs` (new) — each block kind,
  with and without initial value, located, multiple names.
- `compiler/analyzer/src/rule_constant_range.rs` and its `tests.rs` — the
  range check.
- `compiler/analyzer/src/rule_var_decl_const_initialized.rs` — initialized
  `CONSTANT`.
- `compiler/codegen/src/compile_setup.rs` — initial value.
- `compiler/codegen/tests/it/end_to_end_subrange.rs` — inline default,
  inline initial value, unsigned and 64-bit bases, `VAR_GLOBAL`, function
  block `VAR_INPUT`/`VAR_OUTPUT`.
- `compiler/plc2plc/src/tests/inline_subrange.rs` (new) — round trip.
- `specs/design/subrange-codegen.md` — requirement above.
- `docs/reference/language/data-types/derived/subrange-types.rst` — an
  "Inline Subrange" section; the "Related Problem Codes" entry points to
  P2024 (an array dimension code) and should point to P2026.

## Tasks

### Prefactor PR — subrange initializer struct

- [ ] Add `SubrangeInitialValueAssignment` and change the variant.
- [ ] Update producers (`initial_value: None`) and consumers (`.spec`).
- [ ] Render the optional initial value in `plc2plc`.
- [ ] `cd compiler && just` passes with no test changed.

### Core change PR — inline subrange variables (closes #1966)

- [ ] Failing parser tests for `VAR`, `VAR_INPUT`, `VAR_OUTPUT`,
      `VAR_TEMP`, `VAR_GLOBAL`, located, with and without `:=`.
- [ ] Parser alternatives; parser tests pass.
- [ ] Failing `rule_constant_range` tests (in range, above, below); add the
      `Subrange` arm.
- [ ] Failing end-to-end tests (default and initial value); codegen arm.
- [ ] Failing `plc2plc` round-trip test; passes.
- [ ] Design doc requirement, docs page, `cd compiler && just`,
      `cd specs && just`, docs build.
