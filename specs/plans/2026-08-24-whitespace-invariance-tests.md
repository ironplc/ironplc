# Whitespace-invariance test machinery for the parser

## Context

[Issue #1437](https://github.com/ironplc/ironplc/issues/1437) reports that the
parser rejects legal free-format ST: `s . x`, `refs [0]`, `myRef ^`,
`THIS^ .count` and `Some . Located . Item.Path` all fail with **P0002**,
because two families of rules in `compiler/parser/src/parser.rs` juxtapose
tokens with no optional-whitespace rule (`_`) between them.

Proving that fix — and proving the `_`s we already have keep working — means
asserting a *matrix*: many constructs × many gap positions per construct ×
several kinds of filler. Written as one `#[test]` per cell that is hundreds of
near-identical functions, which is the duplication this repo already tracks
with `just dupes-tests` and which `dialect_flags.rs` was refactored to avoid
("Collapses 14 hand-written tests (cargo-dupes group `c3c13bfc`) into one
table").

**Scope: machinery, plus only tests that pass on unmodified `main`.** No
`#[ignore]`d rows and no grammar changes. The rows for #1437 are added by the
follow-up PR that widens the grammar — by then each is one `#[case]` line.

### The fact that makes this possible

`dsl/src/core.rs:158` — `impl PartialEq for SourceSpan { fn eq(&self, _) -> bool { true } }`.

Spans always compare equal, so inserting whitespace shifts every span in the
tree without changing AST equality. That makes **"the spaced spelling parses to
the identical AST as the tight spelling"** a legal assertion, and a far stronger
one than `is_ok()`. It is what lets one helper replace a table of hand-written
expected ASTs.

### Why gaps get filled with more than a space

`parser.rs:286` — `rule _ = (whitespace() / comment() / pragma())*`, where
`whitespace()` is `Whitespace / Newline`. Every legal gap accepts a space, a
newline **and a comment**, repeated. Hand-written tests would realistically only
ever cover the space, so the generative form tests strictly more than the
hand-written one it replaces.

## Approach: marker expansion

Author each snippet **once**, with a marker at every position where whitespace
is legal. A helper expands it into every variant and asserts they all parse to
the same AST as the marker-free spelling.

```rust
#[rstest]
#[case::array_bounds("TYPE A : ARRAY·[·0..3·]·OF INT; END_TYPE", verbatim, CompilerOptions::default)]
#[case::subscript_interior("v := grid·[·1·,·2·];", in_program, CompilerOptions::default)]
#[case::this_caret("THIS·^.count := 1;", in_method, opts_with_fb_inheritance)]
fn parse_when_gap_filled_then_same_ast(
    #[case] template: &'static str,
    #[case] wrap: fn(&str) -> String,
    #[case] options: fn() -> CompilerOptions,
) {
    assert_gaps_accepted(template, wrap, &options());
}
```

One row yields `3N + 3` parses for `N` markers — a dozen rows averaging three
markers is ~150 asserted variants from a dozen lines of test data, with no
duplicated snippets.

Function-pointer `#[case]` columns are an established pattern here:
`tests/common.rs` documents `with_missing_semicolon_flag` as "a function-pointer
`#[case]` value for the parametrized dialect-flag tests".

## Files

| File | Change |
|---|---|
| `compiler/parser/src/tests/whitespace.rs` | **New.** Helpers + both tables. |
| `compiler/parser/src/tests/mod.rs` | One `mod whitespace;` line (sorted, last). |
| `specs/plans/2026-08-24-whitespace-invariance-tests.md` | **New.** This plan, committed first (per CLAUDE.md). |

Nothing else. Per `specs/steering/syntax-support-guide.md` ("Test File
Organization"), a new feature area is a new file plus one `mod` line, not an
append to a shared file. Helpers stay in `whitespace.rs` rather than
`tests/common.rs` because only this file uses them.

## The machinery

All of it lives at the top of `whitespace.rs`.

```rust
/// Marks a position where IEC 61131-3 permits optional whitespace. Chosen
/// because it cannot appear in ST source outside a string literal.
const GAP: char = '·';

/// The strings substituted at a gap. `parser.rs:286` defines
/// `_ = (whitespace() / comment() / pragma())*`, so a legal gap accepts all of
/// these. Pragmas are excluded: `TokenType::Pragma` exists only when
/// `allow_pragmas` is set (see `xform_collapse_pragmas.rs`).
const FILLERS: [&str; 3] = [" ", "\n", "(* gap *)"];

/// Every whitespace variant of `source`: each gap filled individually with
/// each filler, then all gaps filled at once with each filler.
fn gap_variants(source: &str) -> Vec<String>;

/// The canonical tight spelling — all markers removed.
fn tight(source: &str) -> String;
```

Two assertions, each taking a template, a wrapper and options:

- **`assert_gaps_accepted`** — parses `tight(wrap(template))` (must be `Ok`),
  then asserts every variant parses to an AST `==` that baseline. The failure
  message names the offending variant source.
- **`assert_gaps_rejected`** — parses the tight spelling (must be `Ok`), then
  asserts every variant is `Err`. For adjacencies *inside* one lexical unit,
  where free-format does not apply.

Both helpers iterate; the `#[test]` bodies stay a single assertion call, keeping
the "no branching logic in tests" rule in
`specs/steering/development-standards.md` satisfied.

Three wrapper function-pointers so a row carries no boilerplate:

| Wrapper | Shape |
|---|---|
| `in_program` | `PROGRAM main VAR … END_VAR <body> END_PROGRAM` — reuses `wrap_program` in `tests/common.rs`, extended with the fixture variables these rows need (`grid`, `myRef`, a struct with `items`). |
| `in_method` | `FUNCTION_BLOCK FB_Derived EXTENDS FB_Base … METHOD Run <body> END_METHOD …` — same shape as the local `parse_in_method` at `tests/this_super.rs:13`. |
| `verbatim` | Identity, for `TYPE` / `CONFIGURATION` rows, which are whole libraries rather than statements. |

## The two tables

Every row in both tables passes on unmodified `main`. Any candidate that does
not is dropped from this PR, not ignored.

### Positive — gaps the grammar already permits

`fn parse_when_gap_filled_then_same_ast`. This is regression protection for the
`_`s that exist, and it is what proves the machinery works. Candidate rows, each
anchored to a rule that already carries `_`:

| Row | Rule | Gaps |
|---|---|---|
| `ARRAY·[·0..3·]·OF INT` | `array_specification` `:662` | before/inside/after brackets |
| `STRING·[·10·]` | `string_type_declaration` `:828` | same shape as above — the issue's own evidence that `refs [ 0 ]` should parse |
| `0·..·3` | `subrange` `:594` | either side of `..` |
| `grid[·1·,·2·]` | `subscript_list` `:936` | interior only — permissive today, unlike the space *before* `[` |
| `f(·a·,·b·)` | `function_expression` `:1805` | interior and separators |
| `f(·x·=>·y·)` | `param_assignment` `:1882` | around `=>` |
| `IF·c·THEN·…·END_IF` | `if_statement` `:1902` | around each keyword |
| `v := -·10` | `unary_expression` `:1771` | after the unary operator |
| `THIS·^.count := 1` | `self_ref` `:859` | the one gap in the chain that works today |
| `x := 1·;` / `a·,·b` | `semisep` `:292`, `commasep_oneplus` `:295` | around separators |

The `self_ref` row is the direct control for #1437: the issue's matrix confirms
`THIS ^.count` parses while `THIS^ .count` does not, so this row exercises the
exact machinery the follow-up needs and is green today.

### Negative — the tripwire

`fn parse_when_gap_filled_then_rejected`. These adjacencies sit inside a single
lexical unit and **must keep failing**; the issue warns explicitly that "anyone
fixing this should not 'fix' those." This table is what turns "we widened too
far" from a silent regression into a failing test.

Rows: `INT·#·16` (`:359`), `T·#·100ms`, `WORD·#·16#FFFF` (`:396`), the
character-string prefix forms (`:408`, `:415`), `enumerated_value` `E·#·RED`
(`:637`), date/time internals `2024·-·01·-·20` and `14·:·30·:·20` (`:462`,
`:469`, `:478`), and `signed_integer__negative` `·-·10` in a subrange bound
(`:361`).

**`REF=` is deliberately not a row.** `tests/reference_to.rs:113–127` already
asserts it, `parser.rs:1063–1071` documents why, and the issue names that test
by path as the tripwire that must keep passing. A second assertion of the same
fact is the duplication this work exists to remove — leave that test where it
is.

## Notes for the follow-up grammar PR

Recorded here so the fix is not rediscovered. None of this is in scope now.

- Adding the #1437 rows is one `#[case]` line each against the existing
  `assert_gaps_accepted`: `s·.·x`, `refs·[·0·]` in expression and target
  position, `myRef·^`, `THIS·^·.·count`, `PT·^·[·0·]`, `b·.·0`, `b·.·%X0`,
  `s·.·items·[·0·]·.·y`, and the `VAR_CONFIG` / `VAR_ACCESS` dotted paths.
- Three sites the issue does not list: **`access_path` (`:1605–1611`)** is a
  fourth family-B member; `periodsep_oneplus_no_trailing` (`:290`) lacks the
  `_` its sibling `periodsep_no_trailing` (`:291`) has, which is *why* the two
  `VAR_CONFIG` rules already disagree; and `initial_step` (`:1483`) is missing
  a `_` before `END_STEP` — same shape as the `semisep_oneplus` bug #1417
  fixed, and worth its own issue.
- **Hazard:** the `Element::Deref` arm at `:869` ends
  `&(tok(LeftBracket) / tok(Period))`. Widening the repetition alone will not
  fix `THIS^ .count`; that lookahead needs `_` too. The issue's "suggested
  direction" does not mention it.
- `docs/reference/language/object-orientation/this-and-super.rst:~30` presents
  `THIS ^ . member` as accepted. It becomes true with the fix.

## Verification

```bash
cd compiler && just          # compile + coverage (85% gate) + clippy + fmt + dupes
```

Then specifically:

1. **Everything is green** — `cargo test -p ironplc-parser whitespace` passes
   with nothing ignored and nothing skipped.
2. **The tests can fail** — temporarily change `GAP` to a character the
   templates do not contain and confirm the positive table fails. A green table
   that cannot fail proves nothing. Do the same for the negative table by
   temporarily adding `_` around one of its adjacencies in the grammar, then
   revert.
3. **The negative table is honest** — each rejection must be a genuine parse
   error on the *gap*, not an unrelated failure. Spot-check one row's diagnostic
   is **P0002**.
4. **Nothing else moved** — the full workspace suite is unchanged and no
   `plc2plc` golden fixture moves; the renderer emits the tight spelling, and
   the issue notes a moved fixture is the signal something went wrong.
5. **Duplication did not regress** — `just dupes-tests` against the recorded
   baseline of **16.9% exact / 5.0% near** (measured on `e24aed5`). The blocking
   `just dupes` uses `--exclude-tests` so it is unaffected either way; this is
   the informational number the repo keeps for test redundancy.

## Tasks

- [ ] Commit the plan to `specs/plans/2026-08-24-whitespace-invariance-tests.md`
- [ ] Add `GAP`, `FILLERS`, `gap_variants`, `tight` to `tests/whitespace.rs`
- [ ] Add `assert_gaps_accepted` / `assert_gaps_rejected` and the three wrappers
- [ ] Extend `wrap_program` in `tests/common.rs` with the fixture variables
- [ ] Write the negative table; confirm every row passes on unmodified `main`
- [ ] Write the positive table; drop any candidate row that is not green today
- [ ] Confirm both tables fail when deliberately broken (verification step 2)
- [ ] `mod whitespace;` in `tests/mod.rs`
- [ ] `cd compiler && just`; record `just dupes-tests` before/after
