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
its three callers. The function environment already reports
`P4016 FunctionDeclNameDuplicated` for a repeated function name, so no new
diagnostic is needed. A user global named like a library global gets the
same treatment once the duplicate-variable rule lands (its own change).

The design document's `REQ-CL-analyzer-004` says "a user declaration
shadows an activated library declaration of the same name". Its conformance
test exercises a function block local hiding the library global `PI`, which
is cross-scope hiding and stays true. The requirement is reworded to the
claim its test checks, and a new `REQ-CL-analyzer-007` states the
same-scope rule with a conformance test.

## Prefactoring

None. The change deletes a function and its call sites; there is nothing
to reshape first.

## Design doc reference

`specs/design/compatibility-libraries.md` (`REQ-CL-analyzer-004`,
`REQ-CL-analyzer-007`).

## File map

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
- [ ] Requirement rewording, 007 and its test; comments; how-to paragraph
- [ ] `cd compiler && just`, `cd docs && just compile`, `cd specs && just`
- [ ] `git rm` this plan
