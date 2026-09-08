# `S=` / `R=` set/reset assignment operators (TwinCAT/CODESYS)

## Goal

Support the Beckhoff TwinCAT/CODESYS `S=` (set) and `R=` (reset)
assignment operators: `bOut S= bCondition;` sets `bOut` to `TRUE` when
the RHS is `TRUE` and leaves it unchanged otherwise (never clears it);
`R=` is the mirror. Closes
[#1680](https://github.com/ironplc/ironplc/issues/1680).

## Correction to the issue's own suggested fix

The issue (written before this investigation) speculated adding
`SetAssign`/`ResetAssign` **tokens**, explicitly flagging "check
whether `REF=` is one token or two before choosing." Having now
checked: `REF=` is **two** separately-lexed tokens
(`ref_bind_op()`, `parser.rs:1163`) — an `Identifier`/`Ref` token
spelled "REF" immediately followed by a bare `Equal` token with **no**
intervening whitespace, matched contextually inside
`assignment_statement()`. It is not a dedicated lexer token, and (more
importantly) **it is not gated by any dialect flag at all** —
confirmed by grep across the analyzer and `xform_demote_keywords.rs`:
nothing checks `allow_reference_to` (or any flag) against `ref_bind`.
`r REF= x;` parses under every dialect today, including plain IEC
61131-3 Ed. 2. This looks like a pre-existing gap, not intentional
design — but it's the only real precedent for this exact class of
feature (an assignment-operator extension), so this plan follows it
rather than inventing new rigor `REF=` itself doesn't have.

This matters a lot for `S`/`R` specifically, more than it did for
`REF`: a dedicated keyword token for a whole word like `REFERENCE` or
`ABSTRACT` is a low collision risk against real variable names.
`S` and `R` are extremely common single-letter variable names in real
PLC code (Speed, Reset, resistance, status flags). Adding them as
demoted keyword tokens would tokenize *every* bare `S`/`R` identifier
in a TwinCAT-dialect program as the keyword first, then rely on
demotion having zero misses — a much larger blast radius than the
existing OOP/reference keyword set. **Following the two-token,
grammar-contextual `REF=` technique instead of a new keyword avoids
this risk entirely**: `S=`/`R=` decompose into an ordinary `Identifier`
token (still `S`/`R`, usable anywhere else in the program exactly as
before) and the existing `Equal` token, joined only by an adjacency
check inside `assignment_statement()`'s own alternation. A bare `S`
or `R` used as a normal variable, anywhere outside that exact
target-operator position, is completely unaffected.

Consequence: **no new `allow_*` flag**, no new token, no demotion
change, no `FLAG_FIXTURES` entry, no CLI wiring. This is a pure grammar
+ DSL + renderer change, smaller in surface area than `VAR PERSISTENT`
was, once the lexing question was actually settled instead of guessed.
If asymmetric treatment vs. other TwinCAT extensions (all flag-gated)
turns out to matter later, that is a `REF=` problem too and belongs in
its own issue, not this one.

## Architecture

1. **DSL** (`compiler/dsl/src/textual.rs:934`, `Assignment` struct):
   two new fields, `set_bind: bool` and `reset_bind: bool`, following
   the exact style of the existing `ref_bind: bool` (a boolean per
   surface-syntax variant, not an enum — matching the codebase's
   established pattern here, not introducing a new one for a fourth
   case). Update all 6 existing literal-construction sites (3 helper
   constructors in `textual.rs:891-927`, 3 grammar alternatives in
   `parser.rs:1917-1948`) to set both to `false`.
2. **Grammar** (`compiler/parser/src/parser.rs`,
   `assignment_statement()`, `:1917`): two new alternatives, mirroring
   `ref_bind_op()`'s technique exactly but written inline (no shared
   helper — `S=` and `R=` are two independent, unrelated operators, not
   variations of one; forcing them through one parameterized rule
   would be the premature abstraction the standards warn against for
   two call sites). Each: `target:variable() _ eq:(one-letter identifier or nothing, case-insensitive) tok(TokenType::Equal) with no gap` _ `value:expression()`.
3. **Renderer** (`compiler/plc2plc/src/renderer.rs:1431`,
   `visit_assignment`): two new `if` branches before the existing
   `ref_bind` branch, each writing `"S="` / `"R="` via `write_ws`
   (matching `REF=`'s rendering exactly) then the value expression.
4. **Codegen — explicit refusal, not silent wrong output**
   (`compiler/codegen/src/compile_stmt.rs`, the `StmtKind::Assignment`
   arm, `:108`): unlike `ref_bind` (which reuses the existing
   `ExprKind::Ref` backend and needs no special codegen case at all),
   `set_bind`/`reset_bind` have no equivalent lowering — actually
   implementing "conditionally write, never clear" requires new
   codegen logic (a guarded store) that does not exist yet. Add an
   early check that returns `Diagnostic::todo_with_span(assignment.span())`
   for either flag, matching the project's explicit stated philosophy
   (garretfick, closing #1199: "better to refuse codegen than generate
   the wrong code") and the identical precedent already set for
   `THIS^`/`SUPER^` in `codegen/tests/it/compile_this_super.rs`.
   Semantic analysis (`ironplcc check`) is untouched and will accept
   `S=`/`R=` normally — parsing and static analysis have no reason to
   reject a `BOOL := BOOL`-shaped assignment; only codegen refuses.

## Prefactoring

None needed. Two new boolean fields on an existing struct (following
its own established per-variant-boolean pattern) and two new
alternatives in an existing choice-of-alternatives grammar rule that
already has this exact shape for `REF=`.

## Design doc reference

None exists yet, matching `PERSISTENT`'s precedent — this is a single,
well-understood grammar extension of an existing pattern, not new
architecture.

## File map

- `compiler/dsl/src/textual.rs` — two new `Assignment` fields, 3
  constructor call sites updated
- `compiler/parser/src/parser.rs` — two new grammar alternatives, 3
  existing call sites updated
- `compiler/parser/src/tests/` — new parse tests
- `compiler/plc2plc/src/renderer.rs` — two new render branches
- `compiler/plc2plc/src/tests/` — new round-trip tests
- `compiler/codegen/src/compile_stmt.rs` — explicit not-implemented
  refusal
- `compiler/codegen/tests/it/` — new test proving the refusal (mirrors
  `compile_this_super.rs`'s `compile_when_self_ref_then_not_implemented`)

## Tasks

- [ ] `Assignment.set_bind` / `.reset_bind` fields + update 6 existing
      construction sites
- [ ] Grammar: two new `assignment_statement()` alternatives
- [ ] Renderer: two new `visit_assignment` branches
- [ ] Codegen: explicit `Diagnostic::todo_with_span` refusal
- [ ] Parser tests: `S=`/`R=` parse, mixed with ordinary `:=` in the
      same block, case-insensitivity, no-whitespace-before-`=`
      requirement (mirroring `ref_bind_when_space_between_ref_and_equals_then_error`)
- [ ] plc2plc round-trip tests for both operators
- [ ] Codegen test proving the explicit refusal (not silent wrong
      output, not a panic)
- [ ] Run `cd compiler && just` (compile, coverage, clippy, fmt, dupes)
- [ ] `git rm` this plan file before opening the PR
- [ ] Push the branch and open a PR against `ironplc/ironplc` `main`
