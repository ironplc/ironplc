# Shared OOP corpus fixture for the plc2plc golden corpus

## Goal

Give the OOP extension surface that already parses and round-trips today
(EXTENDS/IMPLEMENTS/INTERFACE, ABSTRACT function blocks, METHOD
declarations, method calls, THIS^/SUPER^) one canonical shared `.st`
fixture, registered in plc2plc's golden-corpus round-trip mechanism —
instead of the current state, where every OOP test input in the project
is an inline Rust string literal duplicated (with small variations)
between `compiler/parser/src/tests/{methods,fb_inheritance,this_super}.rs`
and their `compiler/plc2plc/src/tests/` counterparts.

Closes the still-open part of issue #1428 ("No OOP fixtures exist in any
`.st` corpus or plc2plc golden set"). The issue's other headline item —
enforcing that golden-corpus round-trips actually re-parse — is already
done, via PR #1417 (merged 2026-08-23); this plan does not touch that.

## Architecture

Add one new shared source fixture, `compiler/resources/test/oop.st`,
following the same convention as the 40 other files already in that
directory (referenced by name via `ironplc_test::read_shared_resource`).
Its content is restricted to OOP syntax that is already shipped and
stable: nothing here exercises PROPERTY, INTERFACE bodies with
prototypes, or access specifiers (FINAL/OVERRIDE/ABSTRACT METHOD) — all
still unparsed (#1419–#1424), and still garretfick's own active work.

Register it as one new golden-pair case in
`compiler/plc2plc/src/tests/corpus.rs`, mirroring the file's existing
`write_to_string_when_corpus_source_then_round_trips` pattern (parse →
render → re-parse → compare AST → pin against a committed
`*_rendered.st` file). That existing function hardcodes
`CompilerOptions::default()` for all 17 current cases; since this
fixture needs `allow_fb_inheritance: true`, it gets its own small
sibling function in the same file, matching the established local
pattern already used identically in `fb_inheritance.rs`, `methods.rs`,
and `this_super.rs` (each defines its own `inheritance_options()` /
inline `CompilerOptions { allow_fb_inheritance: true, .. }`).

Not registered in `compiler/parser/src/tests/corpus.rs`: that file's own
doc comment says to add a case there only for a resource the plc2plc
corpus does not render, since the plc2plc round-trip already proves the
parse succeeds (a strictly stronger assertion) — adding it there too
would duplicate the same assertion at a weaker strength, the opposite of
this plan's goal.

The existing inline literal tests in both crates are left as-is. They
test different things (parser negative/edge cases, dialect-gating,
individual syntax-spacing regressions) that a single shared positive
fixture can't replace; de-duplicating them is a separate, larger
decision this plan does not make.

## Prefactoring

None needed. This adds one new file and one new small function
following an existing, already-three-times-repeated local pattern
(`inheritance_options()` / inline `CompilerOptions` override per OOP
test file) — extracting that repetition into something shared would be
the kind of speculative generality the standards explicitly warn against
for a fourth call site that changes nothing about the existing three.
No existing function grows a new branch, no module crosses the line
count limit, and nothing here risks a repeated bug class.

## Design doc reference

- `specs/adrs/0041-staged-method-and-interface-dispatch.md` (METHOD
  declarations, static dispatch, THIS^/SUPER^)
- `specs/design/beckhoff-twincat-dialect.md` §1.3–1.4 (EXTENDS/IMPLEMENTS/
  INTERFACE/ABSTRACT)

Both already exist; this plan adds test coverage for behavior they
describe, not new behavior.

## File map

- `compiler/resources/test/oop.st` — new, the shared source fixture
- `compiler/plc2plc/resources/test/oop_rendered.st` — new, golden
  rendered output (generated from the fixture, not hand-written)
- `compiler/plc2plc/src/tests/corpus.rs` — modified, adds one small
  `#[test]` function registering the new golden pair under
  `allow_fb_inheritance: true`

## Tasks

- [ ] Write `compiler/resources/test/oop.st` covering: a plain
      `INTERFACE`, an `INTERFACE ... EXTENDS` a base interface, a
      `FUNCTION_BLOCK ... EXTENDS ... IMPLEMENTS ...`, a
      `FUNCTION_BLOCK ABSTRACT ...`, METHOD declarations (no
      params/no return, multiple methods on one FB, params + return
      type), a method call with positional args, a method call with a
      named arg, `THIS^` field write, `THIS^` method call, `SUPER^`
      field read, `SUPER^` method call with args
- [ ] Add the new `#[test]` function to
      `compiler/plc2plc/src/tests/corpus.rs`, run it once to generate
      the actual rendered output, and commit that as
      `compiler/plc2plc/resources/test/oop_rendered.st`
- [ ] Run `cd compiler && just` (compile, coverage, clippy, fmt, dupes)
- [ ] `git rm` this plan file before opening the PR
- [ ] Push the branch and open a PR against `ironplc/ironplc` `main`
- [ ] Carry the merged (or pre-merge, if still under review) commit
      onto `twincat-dev` per the usual sync
