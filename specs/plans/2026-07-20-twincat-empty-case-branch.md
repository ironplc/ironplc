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
