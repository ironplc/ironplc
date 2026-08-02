# Plan: `CASE` Branch With No Statements

## Goal

A `CASE` branch whose body has zero statements (a label that falls
through to nothing — just a comment, or a genuine placeholder) fails to
parse. The error surfaces at the *next* case label, not at the empty
branch itself, which made the original survey framing ("bare-integer
`CASE` label") misleading:

```
CASE x OF
    1: y := 1;
    5: (* no statement here *)
    10: y := 3;    // <- P0002 syntax error reported HERE, not at "5:"
END_CASE;
```

## Verification against real code

Initially assumed (matching the survey's framing) that a bare integer
literal simply couldn't start a `CASE` branch at all. Tested that
directly — it parses fine, including with a comment right after the
label and the real statement on the next line. The actual failure only
appears when a branch has **no statement at all** before the next label.

Root cause: `case_element()`'s grammar is `case_list() ':'
statement_list()`, and `statement_list()` is `statements_or_empty()+` —
a **one-or-more** repetition. An empty branch has nothing for that `+`
to match, so `case_element()` fails entirely and backtracks; the parser
then tries to interpret the next label's bare integer as the start of a
statement for the (empty) previous branch, which it can't, producing a
confusingly-located error at the next label instead of at the real
empty branch.

Confirmed this is genuinely valid TwinCAT syntax against a real
TcXaeShell instance (not just documentation) — three cases all compiled
clean: a normal `CASE` with every branch populated, an empty branch
followed by another label, and an empty branch as the last one before
`END_CASE`.

## Design

```rust
rule statement_list_or_empty() -> Vec<StmtKind> = statement_list() / { vec![] }
```

Try the existing one-or-more `statement_list()` first, falling back to
an empty `Vec` — same shape as the `semisep_or_empty` combinator already
used elsewhere in this grammar for optional lists. `case_element()`
switches from `statement_list()` to `statement_list_or_empty()`; no
other change needed (`CaseStatementGroup.statements` is already a plain
`Vec<StmtKind>`, so an empty vec is a valid value with no downstream
handling changes).

## Non-goals

- `case_statement()`'s own `ELSE` clause (`ELSE (* nothing to do *)
  END_CASE`) has the exact same root cause (also built on
  `statement_list()`, not `statement_list()?`), and is plausibly also
  broken -- but wasn't part of the verified survey item, and hasn't been
  separately tested against real TcXaeShell. Left alone for now;
  worth revisiting if a real file needs it.
- `IF`/`ELSIF`/`FOR`/`WHILE`/`REPEAT` bodies likely share the same
  underlying gap (all built directly on `statement_list()`, not
  `statement_list()?`) -- `if_statement()`'s own top-level `body` is
  already `statement_list()?` (so `IF x THEN END_IF` already works), but
  its `ELSIF`/`ELSE` bodies and the other loop constructs aren't. Out of
  scope here -- not part of the verified survey item, no evidence any
  real file needs it.

## File Map

| File | Change |
|------|--------|
| `compiler/parser/src/parser.rs` | New `statement_list_or_empty()`; `case_element()` uses it |

## Testing Strategy

- Parser tests: a `CASE` branch with no statement (followed by another
  label, and as the last branch before `END_CASE`) parses with an empty
  `statements` vec; regression -- a branch with a real statement still
  parses unaffected.
- plc2plc round-trip test for the empty-branch shape.
- End-to-end: verify via the CLI that the original repro now parses
  clean.

## Tasks

- [x] Write plan (this document)
- [x] Grammar fix
- [x] Tests from Testing Strategy
- [x] Verify end-to-end via CLI
- [x] Run full CI pipeline (`cd compiler && just`)
- [ ] Push branch to fork
- [ ] Merge into `twincat-dev`, update `twincat-status.md`, push

## Implementation Notes

- Verified this was a real gap against actual TcXaeShell (not just
  documentation) before implementing: three synthetic files (a normal
  populated `CASE`, an empty branch followed by another label, an empty
  branch as the last one before `END_CASE`) all compiled clean in real
  TwinCAT.
- The plc2plc renderer already had explicit handling for an empty
  `CaseStatementGroup.statements` (`self.write_ws("(* empty *)");
  self.write_ws(";");`) -- someone had anticipated this shape on the
  render side already, even though the parser couldn't produce it until
  this fix. No renderer change needed.

## Revision: address maintainer feedback on PR #1214

garretfick (maintainer) requested changes: this isn't strictly valid
IEC 61131-3 (Annex B's `statement_list` is one-or-more `statement ';'`,
and the `NIL` alternative only legalizes an *explicit* empty statement,
`5: ;` -- not a dropped `;` falling straight through). Making
`case_element()` use an unconditional `statement_list_or_empty()`
therefore relaxed the strict default dialect too, since
`parse_library()` has no access to `CompilerOptions` to gate a raw
grammar rule. Per the syntax-support-guide rule ("anything not in the
standard must ride on an `--allow-x` flag"), this needed to route
through the already-existing `--allow-missing-semicolon` flag instead
(the flag that exists precisely for "the source dropped a required
`;`" cases), reusing the token-insertion pass in `xform_tokens.rs`
rather than the grammar itself.

**Fix**:

- `compiler/parser/src/parser.rs`: reverted `case_element()` to plain
  `statement_list()`, removed `statement_list_or_empty()`. Strict IEC
  is back to rejecting a truly-empty branch (only `5: ;` is legal).
- `compiler/parser/src/xform_tokens.rs`:
  `insert_keyword_statement_terminators()` (already gated behind
  `allow_missing_semicolon`, already used to inject a missing `;`
  after `END_IF`/`END_CASE`/etc.) gained a second, independent piece of
  state: while inside `CASE...END_CASE` (tracked with a depth counter
  for nesting), tokens seen right after a case label's `:` are
  buffered rather than emitted immediately, because until the next
  token disambiguates it, they could be either the start of a real
  statement or the constants of the *next* case label (e.g. is `5:`
  followed by `6:` an empty branch, or is `6` about to turn out to be
  `6 := ...`? Token-level information alone can't tell until we see
  what comes after `6`). The buffer resolves the moment we see either
  an unambiguous statement token (`:=`, `(`, `.`, `[`, `^`, or a
  statement keyword -- a real statement, flush the buffer unchanged)
  or the next branch terminator (another `:`, `ELSE`, `END_CASE` --
  the branch was empty, inject a `;` before flushing). This correctly
  handles the exact case the plan's repro needs: multiple *consecutive*
  empty numeric labels (`5:` immediately followed by `6:` immediately
  followed by `7: y := 1;`), which a non-buffering single-token
  lookahead would get wrong (it would misread the bare numeral `6` as
  "a statement started" and never insert the needed `;` for label
  `5`).
- Tests: the two success-case parser tests now build
  `CompilerOptions` via the existing `with_missing_semicolon_flag()`
  helper instead of `CompilerOptions::default()`; added
  `parse_when_case_branch_empty_and_flag_not_set_then_err` asserting
  the default dialect still rejects it. The plc2plc round-trip test
  needs the flag only for the *first* parse (the source's dropped
  `;`) -- the renderer already writes an explicit `(* empty *) ;` for
  an empty branch, so the second parse (of the rendered output) is
  already strict-grammar-valid and needs no flag, unchanged from
  before this revision.
- `docs/explanation/enabling-dialects-and-features.rst`: documents the
  empty-`CASE`-branch behavior under the existing
  `--allow-missing-semicolon` entry rather than as a new flag.
