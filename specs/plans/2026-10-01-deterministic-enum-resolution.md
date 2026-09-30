# Deterministic Enumeration Value Resolution

Issue: #1945

## Goal

`ironplcc check` gives the same, correct answer on every run for a library
that declares more than one enumeration. Today a valid initial value such as
`st : A2 := S1` is reported as `P2006` on about half of the runs when another
enumeration shares the `TYPE` block.

## Root cause

`xform_resolve_type_aliases` decides whether a type is an alias by searching
`TypeEnvironment::iter()` (a `HashMap`, iterated in a per-process random order)
for *another type with an equal representation*. Every enumeration has the
representation `Enumeration { underlying_type }`, which carries no values, so
any two enumerations with the same underlying type look like aliases of each
other. Whichever one the hash order visits first becomes the "base" of the
other, and `duplicate_enumeration_values_for_alias` re-inserts the base's
values under the alias type. Because the symbol table is keyed by value name,
that re-insertion *moves* the values from one enumeration to the other, and
the enumeration that lost its values then rejects its own members.

A second, deterministic defect shares the cause: the per-type value list is
derived from a name-keyed table, so two enumerations that declare the same
value name (`A : (X, Y); B : (X, Z)`) keep only the later one, and `a : A := X`
is rejected.

## Behaviour after the change

- The values of an enumeration are the values its declaration lists, in
  declaration order.
- The values of an enumeration alias (`B : A;`) are the values of the
  enumeration the alias names, following alias chains.
- Two enumerations with the same underlying type and different values are not
  aliases of each other.
- Two enumerations may declare the same value name; each keeps it.

## Architecture

- A new module `analyzer/src/enumeration_values.rs` holds the values per
  enumeration type (`IndexMap<TypeName, Vec<Id>>`) and the alias links
  (`IndexMap<TypeName, TypeName>`); a lookup follows the alias chain to the
  declaring enumeration.
- `SymbolEnvironment` records each enumeration value there as well as in the
  name-keyed table, and answers `get_enumeration_values_for_type` from it.
- `xform_resolve_type_aliases` reads alias links from the declarations
  (`EnumerationDeclaration` with a `Named` specification) instead of guessing
  them from representation equality.

## Prefactoring

The structure and array branches of `xform_resolve_type_aliases` are dead:
structure elements are inserted without a structure type, so
`get_structure_fields_for_type` only ever returns fields that an earlier alias
duplication inserted, and array duplication is a no-op. Remove them
(`duplicate_structure_fields_for_alias`, `insert_structure_field`,
`get_structure_fields_for_type`, `duplicate_array_elements_for_alias`,
`SymbolInfo::struct_type`) in a separate, behaviour-preserving commit. This also
shrinks `symbol_environment.rs`, which is above the 1000-line limit.

## Audit of other hash-order dependencies

Audit `HashMap`/`HashSet` iteration in the analyzer and codegen for effects on
diagnostics or output. Fix the ones that change observable output (for
example, the enumeration definitions in the debug section are emitted in hash
order, so the same source compiles to different containers). File a follow-up
issue for anything too large for this change.

## File map

- `compiler/analyzer/src/enumeration_values.rs` (new)
- `compiler/analyzer/src/symbol_environment.rs`
- `compiler/analyzer/src/xform_resolve_type_aliases.rs`
- `compiler/analyzer/src/stages.rs`
- `compiler/analyzer/src/lib.rs`
- `compiler/analyzer/src/rule_use_declared_enumerated_value.rs` (tests only;
  enable the ignored alias test)
- `compiler/codegen/src/compile.rs` (sorted enumeration debug definitions)
- `docs/reference/compiler/problems/P2006.rst`

## Tasks

- [ ] Prefactor: remove dead structure/array alias duplication
- [ ] Failing tests: two enumerations in one block (repeated analysis), alias
      values, shared value names, alias chain
- [ ] Add `EnumerationValues` and use it from `SymbolEnvironment`
- [ ] Rewrite alias detection from declarations
- [ ] Failing test then fix for deterministic container output
- [ ] Audit remaining hash-order iteration; fix or file an issue
- [ ] Docs: P2006 note on aliases and shared value names
- [ ] `git rm` this plan; `cd compiler && just`; `cd specs && just`
