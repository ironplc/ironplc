# Reject writes to a CONSTANT variable

Issue: #1815

## Goal

A statement that writes a variable declared `CONSTANT` (`VAR CONSTANT`,
`VAR_GLOBAL CONSTANT` through its `VAR_EXTERNAL CONSTANT`) is rejected by
`check` and `compile` with a new problem code, instead of being accepted and
changing the constant at run time.

## Architecture

`xform_mark_unwritten_constants` already finds every write in the library and
resolves it to the declaration it reaches (design:
`specs/design/constant-variable-inference.md`, REQ-CVI-analyzer-010 to 021).
It keeps only the set of written declarations. The new rule needs the same
resolution, plus where each write is and what kind of write it is, so it can
point at the statement and ignore writes that are not statements.

1. The write collector moves to its own module and records every resolved
   write as a site: declaring scope, name, span, and kind (assignment, `FOR`
   control variable, `=>` output binding, `VAR_IN_OUT` argument, address
   taken, instance initializer, access path). The transform keeps using the
   set of `(scope, name)` it derives from the sites, and the unresolved
   `any_scope` names as today.
2. `rule_constant_not_written` runs the collector on the analysed library,
   gathers the `(scope, name)` of every `CONSTANT` declaration (a
   `VAR_EXTERNAL` stands for its global, as in the collector), and reports
   each site of kind assignment, `FOR` control, output binding or `VAR_IN_OUT`
   argument that reaches one.

Writes through a path the collector cannot resolve to one declaration are not
reported: the rule must never reject a program that does not write a
constant. Taking the address (`REF`, `ADR`), an instance initializer and an
access path are not statements that write, so they are not reported either.

The rule runs after `xform_mark_unwritten_constants`. The transform only marks
declarations with no write, so a marked declaration never has a site.

## Prefactoring

Move `WriteCollector`, `Writes` and their helpers out of
`xform_mark_unwritten_constants.rs` into `write_collector.rs`, and make it
record sites. Behaviour-preserving: the transform's tests pass unchanged. This
is its own commit.

## Design doc reference

`specs/design/constant-variable-inference.md` ("What counts as a write"). Add
a short section with REQ IDs for the rejection rule and conformance tests.

## File map

- `compiler/analyzer/src/write_collector.rs` (new, moved code)
- `compiler/analyzer/src/xform_mark_unwritten_constants.rs` (uses it)
- `compiler/analyzer/src/rule_constant_not_written.rs` (new)
- `compiler/analyzer/src/lib.rs`, `stages.rs` (register)
- `compiler/problems/resources/problem-codes.csv` (P4057)
- `docs/reference/compiler/problems/P4057.rst` (new)
- `specs/design/constant-variable-inference.md` (requirements)

## Tasks

- [ ] Prefactor: extract the write collector, record write sites
- [ ] Tests: assignment, array element and field of a constant, `FOR`
      control, `=>` binding, `VAR_IN_OUT` argument, `VAR_EXTERNAL CONSTANT`
      are rejected; reading, `REF(k)`, and a non-constant with the same name
      in another POU are accepted
- [ ] Problem code P4057 and its documentation
- [ ] Rule and registration; design requirements with conformance tests
- [ ] Run `cd compiler && just`, `cd docs && just`
- [ ] Delete this plan
