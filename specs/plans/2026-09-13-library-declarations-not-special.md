# Library Declarations Are Not Special

Part of the follow-up to [#1525](https://github.com/ironplc/ironplc/issues/1525).

## Goal

An activated compatibility library's declarations merge into the
compilation unit as ordinary source. A user declaration with the same name
in the same scope is a duplicate and is diagnosed as one, exactly as two
user declarations would be. Nothing distinguishes a library declaration
from a user declaration once the merge has happened.

Today a user `FUNCTION` named like a library function causes the library's
function to be dropped before the merge, so the user's wins silently. That
makes libraries special, and it is unsafe: a library body that calls the
dropped function now calls the user's version, with whatever signature and
behaviour that has. If a user does not want a library's declaration, the
answer is to not activate the library, or to rename their own declaration.

This applies to the global scope only. Cross-scope hiding is unchanged: a
function, function block or method local may still hide a global, library
or not, as ADR-0051 decided, and `EXTENDS` field rules are untouched.

## Architecture

Delete `remove_shadowed_functions` from `ironplc_sources::libraries` and
its three callers.

**Deleting the helper alone changes nothing observable.** The duplicate
function it would have left in the merge is collapsed by
`xform_toposort_declarations`, which keeps one declaration per name (the
later one, so the user's), and every later pass sees only that one. The
same collapse hides a repeated `FUNCTION`, `FUNCTION_BLOCK`, `PROGRAM` or
`TYPE` in plain user source. The environments were always the intended
detection point: `FunctionEnvironment::insert` returns `P4016` and
`TypeEnvironment::insert_type` returned `P2007` on a repeated name, and
neither ever fired because the repeat never reached them.

So the environments do the check, as they were meant to:

- The toposort keeps every declaration. Ordering is unaffected, since the
  dependency graph is keyed by name either way.
- `SymbolEnvironment::insert` reports a repeated global type, function
  block, program or configuration name (`P2007` for a type repeating a
  type, `P4013` when a program organization unit is involved) and keeps the
  first declaration. `FunctionEnvironment::insert` keeps reporting `P4016`.
  A function that repeats a symbol, or a symbol that repeats a function, is
  `P4013` from the transform that feeds both environments.
- `TypeEnvironment::insert_type` keeps the first type and no longer reports:
  the symbol environment owns the diagnostic, so a repeated function block
  is one `P4013` rather than a `P2007` and a `P4013` for the same line.
- The symbol and function environment transform runs best-effort, so the
  diagnostics are collected and analysis continues on the first
  declaration instead of the whole library reverting on the first repeat.

`rule_pou_hierarchy` is deleted: it emitted `P4013` for a repeated POU name
after the collapse, so it never fired, and the call hierarchy it is named
for was never implemented there. The real hierarchy rule is #1731.

The design document's `REQ-CL-analyzer-004` says "a user declaration
shadows an activated library declaration of the same name". Its conformance
test exercises a function block local hiding the library global `PI`, which
is cross-scope hiding and stays true. The requirement is reworded to the
claim its test checks, and a new `REQ-CL-analyzer-007` states the
same-scope rule with a conformance test.

## Prefactoring

None. The transform's global-scope inserts are gathered behind one helper
as part of the change, because that helper is where the function
cross-check lives; there is nothing to reshape before it.

## Design doc reference

`specs/design/compatibility-libraries.md` (`REQ-CL-analyzer-004`,
`REQ-CL-analyzer-007`).

## File map

- `compiler/analyzer/src/xform_toposort_declarations.rs` — keep every
  declaration
- `compiler/analyzer/src/symbol_environment.rs` — report a repeated global
  declaration, keep the first
- `compiler/analyzer/src/type_environment.rs` — keep the first type, do
  not report
- `compiler/analyzer/src/xform_resolve_symbol_and_function_environment.rs`
  — collect diagnostics, cross-check functions against symbols
- `compiler/analyzer/src/stages.rs`, `compiler/analyzer/src/lib.rs` — run
  the transform best-effort; drop `rule_pou_hierarchy`
- `compiler/sources/src/libraries/mod.rs` — delete the helper and its tests
- `compiler/project/src/project.rs` — delete the call; the two tests that
  asserted shadowing now assert `P4016`
- `compiler/codegen/tests/it/end_to_end_tc2_math.rs`,
  `compiler/codegen/tests/it/end_to_end_tc2_utilities.rs` — delete the
  call and the two tests that pinned the dropped behaviour
- `compiler/sources/src/project.rs`, `compiler/playground/src/lib.rs` —
  comments that describe shadowing
- `specs/design/compatibility-libraries.md` — reword 004, add 007
- `compiler/analyzer/src/spec_conformance.rs` — 007 conformance test
- `docs/how-to-guides/twincat/use-beckhoff-libraries.rst` — one paragraph
  on name clashes with an activated library

## Tasks

- [ ] Commit this plan
- [ ] Delete the helper, callers and pinned tests; flip the project tests
- [ ] Toposort keeps every declaration; environments report the repeat
- [ ] Requirement rewording, 007 and its test; comments; how-to paragraph
- [ ] `cd compiler && just`, `cd docs && just compile`, `cd specs && just`
- [ ] `git rm` this plan
