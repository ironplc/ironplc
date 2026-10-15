# Plan: Analysis Rejects a Field Selection That Names No Field

Checked against `main` at `524b882` (after #2303 and #2298). Line numbers are
as of that commit. Every reproduction below was rerun there, and the design
was prototyped against it (not pushed) to confirm what it accepts, what it
rejects and which existing tests it affects.

## Goal

`specs/design/lowered-program.md` ("Delivery Constraints") requires every
check that only codegen makes to move into an analyzer rule before the code
that compiles from the AST is deleted: a program analysis accepts must never
reach a codegen error. §6 of the same document already says "The checks only
codegen made are already analyzer rules". For a field selection that names no
field, that is not true yet.

After this work:

- **Analysis reports a structured variable `prefix.field` whose prefix has a
  structure or function block type with no field of that name**, with a new
  problem code, at the field name, naming the field and the variable and
  suggesting the closest field name.
- **What a selection may name is answered by the type environment**: the
  fields of a structure, the fields a function block declares or inherits
  through `EXTENDS`, and (new) the properties it declares or inherits.
- **No codegen test depends on codegen catching an unknown field.** The two
  that do become analyzer tests.

## Current behaviour

### Reproductions

`ironplcc check` and `ironplcc compile` on `main`. `DOUBLER` has `x`
(`VAR_INPUT`) and `y` (`VAR_OUTPUT`); `Item` is
`STRUCT a : DINT; b : DINT; END_STRUCT`.

| # | Selection | `check` | `compile` | After |
|---|---|---|---|---|
| 1 | `fb.bogus := 1;`, `fb : DOUBLER` | accepted | P9999, `compile_place.rs:141` (`instance_field`): "Unknown field 'bogus' on function block 'fb' (a PROPERTY is not supported yet)" | P-field |
| 2 | `r := fb.bogus;` | accepted | the same | P-field |
| 3 | `result := arr[1].missing;`, `arr : ARRAY[1..3] OF Item` | accepted | P9999, `compile_struct.rs:199` (`find_field_in_type`) | P-field |
| 4 | `result := one.missing;`, `one : Item` | accepted | P9999, `compile_struct.rs:280` (the named-structure path) | P-field |
| 5 | `b := timer.BOGUS;`, `timer : TON` | accepted | P9999, `compile_place.rs:141` | P-field |
| 6 | `p^.missing` (`REF_TO Item`), `io.missing` (`VAR_IN_OUT io : Item`), `nest.inner.missing`, `h.o.missing` (`o` a `VAR_OUTPUT` of type `Item`) | accepted | | P-field each |
| 7 | `nest.nope.a`, `nope` not a field of `nest` | accepted | | one P-field, at `nope` |
| 8 | `d.count`, `count` a `VAR_INPUT` of a base block (one or two `EXTENDS` up) | accepted | P9999 (#1438: codegen does not lay out inherited variables) | accepted |
| 9 | `d.Speed`, `Speed` a `PROPERTY` of the block or of a base | accepted | P9999 | accepted |
| 10 | `timer.q` (case differs from `Q`) | accepted | compiles | accepted |
| 11 | `a.Position`, `a` of an `INTERFACE` with a property `Position` (`--dialect twincat`) | P2008, P9999 (interfaces are not supported) | | unchanged: no P-field |
| 12 | `r.a` and `r.missing`, `r : REFERENCE TO Item` (`--dialect twincat`) | accepted | P9999 "Variable 'r' is not a structure", for both | unchanged |

Rows 8, 9 and 12 are legitimate P9999s: the program names something that
exists, and codegen does not build it yet. Row 11 is the reason the design
below needs more than "the structure has no such field": an interface is
entered in the type environment as an empty structure
(`xform_resolve_type_decl_environment.rs:386-407`), so a rule that trusts the
representation alone reports a false P-field there. A prototype that did so
reported P-field on `a.Position`.

### What already answers "which field is this"

- `variable_type::type_id` (`variable_type.rs:75`, #2296) walks a reference to
  the id of the type it names: the declared type of a variable, `field_type`
  for a field, `element_type` for a subscript, `referenced_type` for a
  dereference. `None` for `THIS^`/`SUPER^` (#1406) and whenever a step has no
  type.
- `TypeEnvironment::field_type` (`type_environment/fields.rs:137`) gives the
  declared type of a field of a structure, of an alias of one, or of a
  function block, own or inherited. It is `None` both when there is no such
  field and when the field's declared type did not resolve
  (`BlockField::field_type` is `None` for `VAR x : Undeclared`). The rule must
  tell those apart, or it reports a second, false problem beside the one the
  declaration's own rule reports.
- `TypeEnvironment::block_field` (`fields.rs:118`) walks the `EXTENDS` chain
  with its own loop over `BlockRecord::base`.
- No environment records a function block's properties.
  `rule_use_declared_symbolic_var` reads them from the declaration of the
  block whose body it is in (`enclosing_properties`, lines 96-104 and 139),
  which serves a bare name inside the block, not a selection on an instance of
  some other block.

### Tests that depend on the gap

The prototype rule was run against `cargo test --workspace`. Only these fail,
both because `CleanAnalysis::new(..).expect(..)` in
`codegen/tests/it/common/run.rs:128` now panics on a program analysis
rejects:

- `end_to_end_when_user_fb_dot_access_write_unknown_field_then_diagnostic`
  (`codegen/tests/it/end_to_end_user_fb.rs:220-245`), row 1.
- `compile_when_top_level_array_of_struct_unknown_field_then_not_implemented`
  (`codegen/tests/it/end_to_end_array_of_struct.rs:767-787`), row 3.

`variable_type.rs:192` selects `f.missing`, but its tests resolve types
without running the rules, so they are unaffected. The unit test at
`codegen/src/compile_struct.rs:697` calls `find_field_in_type` directly and
compiles no program (see [Decisions](#decisions-for-review)).

## Architecture

### The rule

`analyzer/src/rule_use_declared_field.rs`, named like
`rule_use_declared_symbolic_var` and `rule_use_declared_enumerated_value`,
registered in `stages::semantic`. It tracks the scope with `ScopeTracker` and
visits each `StructuredVariable`. It first recurses into the prefix (and so
into any subscript in it), then checks the selection, so two reports in one
statement come out in source order. For `prefix.field`:

1. `record = variable_type::type_id(prefix, ...)`. `None`: report nothing.
   That covers an undeclared prefix (P4007 is `rule_use_declared_symbolic_var`'s),
   `THIS^`/`SUPER^` (#1406), and a prefix that is itself a selection already
   reported (row 7 reports once).
2. `types.field_type(record, field)` is `Some`: a field. Nothing to report.
   This is the common path and the existing answer.
3. `types.field_names(record)` (new) is `None`: the prefix is not a structure
   or block whose fields the environment records (an `INT`, a `REFERENCE TO`,
   the interface placeholder). Report nothing.
4. The names contain `field`: a field whose type did not resolve, which the
   declaration's rule reports. Report nothing.
5. `types.has_property(record, field)` (new): a property, own or inherited.
   Report nothing; codegen keeps its P9999 "not implemented".
6. Otherwise report P-field.

The diagnostic labels the field name ("Undefined field") and carries
`field=<name>`, `variable=<prefix as written>` (`arr[1]`, `p^`, `h.o`) and,
when `find_closest_match` finds one among `field_names`, `did you mean=<name>`.
The suggestion uses field names only, as the request asks (see
[Decisions](#decisions-for-review)).

Method calls are not `StructuredVariable`s and stay P4046's
(`rule_method_call_declared`). A bit or partial access (`n.3`, `w.%X1`) is a
different node and is not visited.

### What the type environment adds

In `type_environment/fields.rs`, beside `field_type` and `block_field`:

- **`field_names(record) -> Option<Vec<&Id>>`**: the names a selection on a
  value of type `record` may name as a field.
  - A function block with a record: its own fields in declaration order, then
    each base's, nearest first, over `inheritance_chain` (see
    [Prefactoring](#prefactoring)). An alias of a block has the block's.
  - A structure the environment recorded (`record_structure`, #2307): the
    field names its type's representation lists, in declaration order. The
    names come from the representation, not from #2307's record, because the
    record leaves out a field whose type did not resolve and keeps no order
    (which would make the suggestion nondeterministic on a tie). The record
    decides only that the type *is* a declared structure. An alias of a
    structure has the structure's.
  - Anything else: `None`. That includes an interface, which is entered as an
    empty structure but never recorded as one.
- **Properties.** `BlockRecord` gains `properties: Vec<Id>`, the names of the
  `PROPERTY`s the block declares. `record_block` takes them with the fields
  and the base, and `xform_resolve_decl_types::fold_function_block_declaration`
  passes `node.properties`, so all three come from the one declaration the
  environment keeps. A standard block has none.
  **`has_property(block, name) -> bool`** asks the block and each block on its
  `inheritance_chain`. Only names are recorded: property access is not
  implemented, and nothing reads a property's type.

Why the type environment holds the properties:

- It already answers "what does an instance of this function block type
  have" for fields, and #2302 plans the base relation and method signatures
  there. A selection starts from the prefix's type, not from a scope.
- Not `FunctionBlocks` (`callee_resolution.rs`). The review on #1996 asked
  that `FunctionBlocks::chain` stay private rather than grow a second walk,
  and its table is by name, so it misses an alias of a block.
- Not the symbol environment. #1996 makes each property a symbol in its
  block's scope, which answers what a *bare* name means inside the block. A
  block's variables are recorded both ways today: as symbols in its scope and
  as fields in the type environment. Properties would be the same, each
  environment answering its own question.

### The problem code

A new code, not P4007. P4007 says a name is not declared in the scope, and
its page tells the reader to declare the variable. Here the variable is
declared and its type has no member of that name. The sibling is P4046
(`MethodNotFound`) for a method.

```csv
P####,FieldNotFound,Field is not declared on the structure or function block or any function block in its EXTENDS chain
```

The number is the next free one when the core change PR opens: P4084 on
`main` today. #2302 plans to take the next two free codes for its core PRs,
and #1901 claims P4066-P4068, so whichever opens first takes P4084.

`docs/reference/compiler/problems/P####.rst` follows the template in
`problem-code-management.md`, with `.. playground::` examples
(`doc-standards.md`): a misspelled structure field and its fix, and a
sentence saying that a function block's inputs, outputs, `VAR` variables and
inherited variables are its fields, and that a property is not reported here.
The rule's tests pin both examples, as `rule_var_external_matches_global`
does for P4082.

### Codegen

No codegen source changes. Its error paths stay until the AST route is
deleted. The two codegen tests above are deleted, and each program becomes an
analyzer test asserting P-field. Neither program compiled by the lowered
route, so the lowered count does not change and `lowered_floor` in
`compiler/justfile` stays as it is.

The rule applies on every dialect and needs no option: a selection that names
nothing is invalid whatever the dialect.

## Prefactoring

One behaviour-preserving prefactor:

**Walk a block's `EXTENDS` chain in one place.** Add
`TypeEnvironment::inheritance_chain(block) -> impl Iterator<Item = TypeId>`
to `fields.rs`: the block, its base, that block's base, and so on, from
`BlockRecord::base`, bounded as `block_field` bounds its loop today.
`block_field` becomes a `find_map` over it. Existing tests pass unchanged; new
unit tests cover no base, one level, two levels, an alias, and a block with
no record.

Without it, the core change adds two more copies of `block_field`'s loop
(`field_names`, `has_property`). The name and shape are the ones #2302's
prefactor 1 plans, so whichever lands second reuses it instead of adding
another. #2302 planned it over an `extended` relation that #2303 has since
folded into `BlockRecord::base`.

Nothing else needs reshaping. The rule reads existing queries, and the
property record is new state, which belongs in the core change.

## Decisions for review

1. **A new code rather than P4007.** Recommended, for the reason under
   [The problem code](#the-problem-code). The alternative is P4007 with a
   `field=` context, which would keep a fix ("declare the variable") that does
   not apply.
2. **Properties recorded in the type environment.** Recommended. The
   alternatives are a `FunctionBlocks` walk, which #1996's review turned
   down, or waiting for #1996 and asking the symbol environment from the
   prefix block's scope, which would block this change on an open PR whose
   timing is not known.
3. **Keep the `find_field_in_type` unit test** (`compile_struct.rs:697`).
   Recommended: it compiles no program, and it pins an error path that stays
   until the AST route is deleted, when the test goes with it.
4. **Suggest field names only, not property names.** Recommended: the request
   asks for the closest field, and a suggested property leads straight to
   P9999. The alternative includes properties in the candidates.
5. **Out of scope, unchanged:** a selection a caller may not make under IEC
   visibility (`fb.io` for a `VAR_IN_OUT`, accepted by analysis and P9999 in
   codegen, which lays out only inputs, outputs and `VAR`); an inherited field
   through an instance (row 8, #1438); a selection through `REFERENCE TO`
   (row 12); property access itself.

## Coordination with open pull requests

- **#2307** (record a structure's fields once) is a prerequisite of the core
  change PR: its structure record is how `field_names` tells a declared
  structure from the interface placeholder. The prefactor does not depend on
  it.
- **#2302** (one `EXTENDS` relation) plans `inheritance_chain`; this plan's
  prefactor delivers it with the same name and shape. It also plans to take
  the next free problem codes (see [The problem code](#the-problem-code)).
- **#1996** (properties as symbols) records properties for bare names in
  their block's scope. It does not overlap with the record added here (see
  [What the type environment adds](#what-the-type-environment-adds)), and
  nothing here touches `rule_use_declared_symbolic_var`.
- **#2000** (types for `THIS^` and `SUPER^`): once `type_id` types them, the
  rule checks `THIS^.missing` with no change of its own. Whichever lands
  second adds that test.

## Design doc reference

- `specs/design/lowered-program.md`, "Delivery Constraints" and §6: the
  constraint this meets. The text already states the outcome, so it is
  unchanged.
- `specs/design/expression-type-resolution.md` (status: implemented)
  describes what `record_block` records. The core change PR adds the property
  names to that paragraph and one requirement (the next free
  `REQ-ETR-analyzer-NNN`, 007 once #2307 lands): `field_names` of a derived
  block lists its own fields, then its base's; `has_property` holds for a
  property its base declares; an interface has no `field_names`. Its test
  lives in `fields.rs` or `xform_resolve_decl_types.rs`.

## File map

| File | Prefactor | Core change |
|---|---|---|
| `compiler/analyzer/src/type_environment/fields.rs` | `inheritance_chain`; `block_field` over it; tests | `BlockRecord::properties`; `record_block` takes them; `field_names`, `has_property`; tests |
| `compiler/analyzer/src/xform_resolve_decl_types.rs` | | pass the block's property names to `record_block`; test that a repeated block keeps its first declaration's properties |
| `compiler/analyzer/src/rule_use_declared_field.rs` (new) | | the rule and its tests |
| `compiler/analyzer/src/lib.rs`, `compiler/analyzer/src/stages.rs` | | register the rule |
| `compiler/problems/resources/problem-codes.csv` | | P-field |
| `docs/reference/compiler/problems/P####.rst` (new) | | P-field's page |
| `compiler/codegen/tests/it/end_to_end_user_fb.rs` | | delete the unknown-field test (and any import only it used) |
| `compiler/codegen/tests/it/end_to_end_array_of_struct.rs` | | delete the unknown-field test |
| `specs/design/expression-type-resolution.md` | | properties in the block record; one requirement |

## Tasks

The work is one prefactor PR and one core change PR, so no tracking issue.

### Prefactor: walk a block's `EXTENDS` chain in one place

- [ ] `TypeEnvironment::inheritance_chain` in `fields.rs`, with unit tests: no
      base, one level, two levels, an alias of a block, no record.
- [ ] `block_field` follows it; existing tests unchanged.
- [ ] `cd compiler && just`.

### Core change: report a field selection that names no field

Starts once #2307 has merged, on `main` with the prefactor merged (or stacked
on the prefactor branch, per the development process).

- [ ] P-field in `problem-codes.csv` (next free code) and its page, with
      playground examples.
- [ ] `BlockRecord::properties`, `record_block`, and
      `fold_function_block_declaration` passing the block's property names.
- [ ] `field_names` and `has_property`, with unit tests: a structure (in
      declaration order), an alias of a structure, a block's own then
      inherited fields, a standard block, an interface placeholder (`None`),
      `INT` (`None`); a property own, inherited, absent, and on a standard
      block.
- [ ] `rule_use_declared_field`, registered in `stages.rs` and `lib.rs`.
      Tests (`rule_err_at!` at the field name for each rejection,
      `rule_ok!` for each acceptance):
  - [ ] rejected: rows 1-5, each prefix kind of row 6 (named variable, array
        element, dereference, nested field, field of a `VAR_IN_OUT`, function
        block output), row 7 reported once;
  - [ ] accepted: a real field of each prefix kind, rows 8-10 (inherited
        field two levels up, own and inherited property, `timer.q`), row 11's
        interface, an undeclared prefix, `THIS^.x`, a block field whose type
        did not resolve, a method call `fb.M()`;
  - [ ] the message: `field=`, `variable=` and `did you mean=` for a
        misspelling, and no suggestion when nothing is close;
  - [ ] two selections in one statement reported in source order;
  - [ ] the page's failing and fixed examples.
- [ ] Delete the two codegen tests.
- [ ] `expression-type-resolution.md`: the block record paragraph and the
      requirement, with its `#[spec_test]`.
- [ ] Rows 1-5 fail `ironplcc check` with P-field; rows 8-11 do not.
- [ ] `cd compiler && just`, `cd specs && just`.

### Cleanup

- [ ] Close this plan PR unmerged.
