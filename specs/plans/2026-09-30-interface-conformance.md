# IMPLEMENTS conformance (#1891, part 2)

## Goal

A function block that `IMPLEMENTS` an interface must provide every method
and property the interface declares, including those of the interfaces it
extends, with a matching signature. A missing member and a mismatched one
are each reported.

Stacked on #1900 (part 1: interface types and conversion).

## Rules

- **Method:** the function block, or a base in its `EXTENDS` chain, has a
  method of the same name. Its return type is the prototype's, and its
  `VAR_INPUT`/`VAR_OUTPUT`/`VAR_IN_OUT` variables match the prototype's
  parameters one to one: same names, same kinds, same types, same order.
  Locals (`VAR`, `VAR_TEMP`) are not compared.
- **Property:** the function block or a base has a property of the same
  name and type, with every accessor the prototype declares (`GET`, `SET`).
  An extra accessor is allowed.
- **`ABSTRACT` function blocks** follow the same rules.
- An interface named in `IMPLEMENTS` that is not declared is not this
  rule's concern; it is skipped.

The strict signature rule and the `ABSTRACT` rule are not verified in XAE.
The TwinCAT corpus from #1199 builds in TwinCAT, so any finding there is a
false positive; it is the check for both.

## Architecture

A new rule, `rule_interface_conformance`, walks each function block with an
`IMPLEMENTS` clause. Required members are collected from the interface and,
transitively, the interfaces it extends. Members are looked up through the
existing `FunctionBlocks` chain walk (`callee_resolution`), extended with a
property lookup. Types are compared by the resolved `TypeId` of each
parameter, and by name (and length for strings) for return and property
types.

Two new problem codes: P4067 (member missing) and P4068 (member signature
does not match). Both point at the interface's name in the `IMPLEMENTS`
clause, with the prototype as a secondary label.

## Prefactoring

None needed. `FunctionBlocks` already walks the `EXTENDS` chain in one
private helper that `resolve_method` and `declaring_block` share; the
property lookup is a third user of it, not a copy.

## Design doc reference

- `specs/design/beckhoff-twincat-dialect.md` §1.3 / §1.4: conformance is
  now checked.
- ADR-0041: the "`IMPLEMENTS` conformance rule" it refers to now exists.

## File map

- `analyzer/src/callee_resolution.rs`: property lookup
- `analyzer/src/rule_interface_conformance.rs` (+ `tests.rs`), `lib.rs`,
  `stages.rs`
- `problems/resources/problem-codes.csv`, `docs/reference/compiler/problems/P4067.rst`, `P4068.rst`
- `docs/reference/language/object-orientation/implements.rst`,
  `interface.rst`, `index.rst`
- `specs/design/beckhoff-twincat-dialect.md`,
  `specs/adrs/0041-staged-method-and-interface-dispatch.md`

## Tasks

- [ ] Property lookup in `FunctionBlocks`
- [ ] Rule with P4067/P4068, tests for each rule above
- [ ] Problem pages and docs, design doc and ADR updates
- [ ] Corpus: rerun, investigate every finding as a possible false positive
- [ ] `git rm` this plan; `cd compiler && just`
