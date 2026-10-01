# Initial Values for a Variable of a Named Array Type

Plan for [#1971](https://github.com/ironplc/ironplc/issues/1971): an initial
value on a variable of a named array type is a syntax error.

## Goal

A declaration of a named array type takes an array initializer wherever the
inline form `ARRAY[...] OF ... := [...]` is accepted today:

```
TYPE
  A3 : ARRAY[1..3] OF DINT;
  S : STRUCT f : A3 := [1, 2, 3]; END_STRUCT;
END_TYPE

PROGRAM main
  VAR_INPUT i : A3 := [2(0), 1]; END_VAR
  VAR x : A3 := [1, 2, 3]; r : DINT; END_VAR
  r := x[2];
END_PROGRAM

CONFIGURATION config
  VAR_GLOBAL g : A3 := [4, 5, 6]; END_VAR
  ...
END_CONFIGURATION
```

`check` accepts these, runs the same element checks as for the inline form,
and `r` is 2 after one scan. IEC 61131-3 (2013) B.1.4.3 allows it:
`array_spec_init ::= array_spec [':=' array_init]` with
`array_spec ::= array_type_access | 'ARRAY' ...`.

## Cause

`array_spec_init` (`compiler/parser/src/parser.rs`) has only the inline
alternative of `array_spec`. A declaration `x : A3` takes another path: it is
recorded as a late-bound type (`LateResolvedType`) and
`xform_resolve_late_bound_type_initializer` turns it into
`Array(ArrayInitialValueAssignment { spec: Named(A3), initial_values: [] })`.
The rules that take a user type name with `:=` (`VAR`, `VAR_GLOBAL`,
structure fields) accept an expression, a member list or an enumerated value
after it, never `[`.

Everything after the parser already handles a named array with values:
`variable_type::resolve_initializer` resolves `Named` through the type
environment, `rule_constant_range` checks each element against the element
type (P2026), and `emit_initial_values` stores the values. A quick experiment
that adds the alternative below confirms it: local, `VAR_INPUT` of a
`PROGRAM`, global, repeated (`[2(7), 1]`), two-dimensional and `STRING`
element cases run with the right values, and an out-of-range element reports
P2026.

## Architecture

### Parser

`array_spec_init` gains a second alternative, a type name followed by an
array initializer:

```
/ type_name:array_type_name() _ tok(TokenType::Assignment) _ a:array_initialization() {
    ArrayInitialValueAssignment { spec: SpecificationKind::Named(type_name), initial_values: a }
}
```

The bracket is what makes the shape certain, so the parser emits the `Array`
kind directly. ADR-0050 leaves a user-typed initializer to the resolver
because `T := (a := 1)` and `T := Red` fit more than one kind of type; a
bracketed list fits only an array, as a qualified `T := T#Red` fits only an
enumeration, which the parser already settles. The alternative needs no new
AST node, and every rule that reaches `array_spec_init` gets it at once:

- `VAR`, `VAR_INPUT`, `VAR_OUTPUT`, `VAR_TEMP` and the other blocks that use
  `var_init_decl` (through `array_var_init_decl`);
- `VAR_GLOBAL` and located variables (through `located_var_spec_init`);
- structure fields (through `structure_element_declaration`);
- `TYPE B3 : A3 := [...]` (through `array_type_declaration`), which already
  resolves to an alias of `A3`; its values then share the fate of every
  `TYPE`-level array default (see Not covered).

`array_type_name` is `type_name()`, so an elementary type such as
`x : DINT := [1, 2]` stays a syntax error.

### Analyzer

Two declarations the parser now accepts have no meaning, and nothing reports
them today (the experiment: `check` passes, `compile` fails with P9998):

- the named type is not declared: `x : NOPE := [1, 2]` reports P2008
  (`UndeclaredUnknownType`), the code a bare `x : NOPE;` reports;
- the named type is not an array: `x : S := [1, 2]` with `S` a structure
  reports P4022 (`InitializerTypeMismatch`).

Both come from a new rule module, `rule_array_initializer_type.rs`, which
visits every `ArrayInitialValueAssignment` with a `Named` spec and values (in
a variable, a structure field and an `ArrayDeclaration`). A new module keeps
`rule_var_decl_initializer_type_compat`'s scalar check unchanged; no new
problem code is needed.

Element range (P2026) needs no change: `rule_constant_range` already walks
the values of a `Named` array. The count of values and the element types are
not checked for any array, inline or named; those are separate issues (see
Not covered), and when they are fixed the named form gets the fix through the
same resolution.

### Code generation

No change. `emit_initial_values` already resolves `Named` to the array's
layout and stores the values, for locals, program inputs and configuration
globals.

### plc2plc

No change: `visit_array_initial_value_assignment` writes the name and the
values. Round-trip tests cover each position. A configuration `VAR_GLOBAL` is
not rendered at all today; #1915 fixes that, and the round-trip test for the
global form is added once it lands (or is written against the
`RESOURCE`-less form if #1915 is not merged first).

### Relation to open pull requests

- #1972 rewrites `Simple(A3)` globals to `Array(Named(A3))` in
  `xform_resolve_decl_types`. A global with a bracketed initializer is
  `Array(Named(A3))` from the parser, so the two changes do not overlap; the
  end-to-end global test does not depend on #1972.
- #1907 restructures the named-type initializer rules in `parser.rs`
  (`simple_spec_init__with_value`). This change touches only
  `array_spec_init`, so the two do not conflict textually.

## Prefactoring

None needed. The change is one parser alternative and one new rule module;
no `match` gains an arm in more than one place, no function is copied, and no
module crosses 1000 lines (`parser.rs` is the generated grammar).

## Design doc reference

No `specs/design/` document covers declaration initializers. ADR-0050 says
the parser "still emits `EnumeratedType` for a qualified value ... because
those are unambiguous"; the core change adds the bracketed array initializer
to that sentence, so the record says why the parser settles this one.

## File map

Core change PR:

- `compiler/parser/src/parser.rs` — the `array_spec_init` alternative.
- `compiler/parser/src/tests/named_array_initializers.rs` (new) — AST shape
  in each position.
- `compiler/analyzer/src/rule_array_initializer_type.rs` (new),
  `compiler/analyzer/src/stages.rs`, `compiler/analyzer/src/lib.rs` — P2008
  and P4022 for a named spec that is undeclared or not an array.
- `compiler/analyzer/src/rule_constant_range/tests.rs` — P2026 on a named
  array element, in a variable and a structure field.
- `compiler/codegen/tests/it/end_to_end_named_array_initializer.rs` (new),
  `compiler/codegen/tests/it/main.rs` — values after one scan.
- `compiler/plc2plc/src/tests/declarations.rs` — round trip.
- `docs/reference/language/data-types/derived/array-types.rst` — an "Initial
  values" section (inline and named, repeat count).
- `docs/reference/compiler/problems/P4022.rst`, `P2008.rst` — an example
  each if the existing pages do not already cover the case.
- `specs/adrs/0050-parser-records-user-typed-initializers-for-the-resolver.md`
  — the sentence above.

## Tasks

Core change PR (`fix/named-array-initializer`):

- [ ] Parser tests (seen failing): `VAR`, `VAR_INPUT` of a `FUNCTION_BLOCK`
      and a `PROGRAM`, `VAR_GLOBAL` in a `CONFIGURATION`, a located variable,
      a structure field, `TYPE B3 : A3 := [...]`, repeat count
      `[2(0), 1]`, enumerated values `[Red, Green]`, an elementary type name
      stays P0002.
- [ ] Add the `array_spec_init` alternative.
- [ ] Analyzer tests (seen failing): P2008 for an undeclared named type,
      P4022 for a structure and for an elementary alias, in a variable, a
      field and a `TYPE` alias; no diagnostic for an array alias
      (`B3 : A3; x : B3 := [...]`).
- [ ] Add `rule_array_initializer_type`.
- [ ] Range tests: P2026 for an out-of-range element of a named array in a
      variable and in a structure field (expected to pass already).
- [ ] End-to-end tests: local, `PROGRAM` `VAR_INPUT`, configuration global,
      repeat count, two-dimensional, `STRING` element, array alias, and the
      value persists after a write over two scans.
- [ ] plc2plc round trip for every parsed position above.
- [ ] Docs, ADR-0050 sentence.
- [ ] `cd compiler && just`, `cd specs && just`, docs build with `-W`.

## Not covered

These were found while planning and fail for the inline form too; each is
its own issue:

- #1986 — the values of a `TYPE`-level array default are ignored at run time
  (so `TYPE B3 : A3 := [...]` parses and checks, but its variables read
  zeros, as with `TYPE A3 : ARRAY[...] OF DINT := [...]` today).
- #1987 — plc2plc drops a `TYPE`-level array default.
- #1988 — more values than elements passes `check` and fails at run time.
- #1989 — an element of the wrong type (`TRUE` for `DINT`) passes `check`.
- #1990 — an array of structures cannot take initial values
  (`[(a := 1), (a := 2)]` is a syntax error, and code generation refuses
  them).

Known limitations that the named form inherits unchanged:

- A structure field's array initial values are ignored at run time (#1542):
  `S : STRUCT f : A3 := [1, 2, 3]; END_STRUCT` parses and checks, `s.f[2]`
  reads 0.
- An array input or local of a `FUNCTION` or `FUNCTION_BLOCK` does not
  compile (#1977, #1483), so the end-to-end input test uses a `PROGRAM`.
- An array of a named enumeration does not compile (#1941).
