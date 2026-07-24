# ADR-0040: Dialect Violations Are Diagnosed in a Policy Phase, Not by Gating the Grammar

status: proposed
date: 2026-07-24

## Context and Problem Statement

IronPLC accepts Structured Text under several configurations — the IEC 61131-3
editions and the vendor-compatible `rusty`/`codesys` dialects — expressed as
[`Dialect`] presets plus per-feature `--allow-*` flags on [`CompilerOptions`]
(ADR-0036, ADR-0038). A recurring question is *where* the compiler should reject
a construct that is syntactically recognizable but disabled by the current
dialect, and *what* it should report when it does.

The parser is deliberately option-free. `parse_library`/`parse_statements`
(`compiler/parser/src/parser.rs`) take only tokens; [`CompilerOptions`] never
reaches the PEG grammar. Instead, options are "compiled away" into the token
stream *before* parsing, inside `tokenize_program`
(`compiler/parser/src/lib.rs`). Today there are three sanctioned mechanisms for
making a feature conditional, documented in
[`syntax-support-guide.md`](../steering/syntax-support-guide.md):

1. **Token demotion** (`xform_demote_*.rs`) — a keyword token is rewritten to
   `Identifier` when its flag is off, so the vendor grammar path structurally
   cannot match and the word remains usable as an ordinary identifier.
2. **Token rejection rule** (`rule_token_no_*.rs`) — the lexer always tokenizes
   the construct; a token-level rule with access to options emits a diagnostic
   when the flag is off.
3. **Token transform** (`xform_tokens.rs`) — fix up the stream (e.g. missing
   semicolons).

All three require a *distinguishing token*. This breaks down for **structural
extensions**, where the vendor grammar overlaps the standard grammar with
nothing to demote — a constant *expression* where the standard allows only a
literal initializer ([PR #1220]), or an `AT`-located variable inside an ordinary
`VAR` block ([PR #1221]). For these, the TwinCAT series ([#1199]) reached for a
fourth, undocumented pattern: parse permissively into a placeholder or marker
AST node (`InitialValueAssignmentKind::SimpleExpr`, `in_mixed_var_block`), then
reject far downstream in the analyzer. This is the "uncomfortable decision"
that prompted the ADR — it smears the "is this legal in this dialect?" question
across the parser, an AST provenance marker, and an analyzer rule.

A related symptom compounds it. The parser itself emits exactly one problem
code:

```
P0002  SyntaxError  "Syntax error"
```

Whenever the grammar simply fails to match — which is precisely what token
demotion produces when a flag is off (the demoted word derails the parse
*somewhere else*) — the user gets a generic `P0002`, often pointing at the
wrong span. Meanwhile the token-rule and analyzer-rule layers already emit
*specific*, feature-named codes following a consistent convention:

```
P0004  CStyleComment                "C-style comment not allowed"
P0011  EmptyVarBlock                "Empty variable block requires --allow-empty-var-blocks flag"
P4028  TopLevelVarGlobalNotAllowed  "Top-level VAR_GLOBAL requires --allow-top-level-var-global flag"
P4033  PartialAccessSyntaxDisabled  "Partial-access syntax (.%Xn) requires --allow-partial-access-syntax flag"
```

Two questions this ADR settles:

1. **Should dialect gating move *into* the grammar** (give the PEG parser
   [`CompilerOptions`] and guard rules on flags), or stay in a phase outside the
   grammar?
2. **How does a disabled feature get a specific problem code** instead of a
   generic `P0002 SyntaxError`?

## Decision Drivers

* **One maximal grammar to reason about** — the grammar should describe the
  *superset* language; dialect policy is a separate concern. Conditionalizing
  rules on runtime options forks every gated rule's behavior, making the grammar
  harder to read and to test (each rule now needs flag-on/flag-off coverage) and
  complicating reasoning about recursion and precedence.
* **Error recovery** — a permissive parse continues and reports many problems; a
  grammar alternative that *fails* on a disabled feature abandons the parse at
  the first one.
* **Error specificity** — a diagnostic can only *name* a problem it has
  *recognized*. A generic syntax error is definitionally "I do not know what
  this is." Rejecting *after* the construct is recognized (with a real node and
  full syntactic context in hand) yields a more specific message, not a less
  specific one.
* **No silent accommodation (ADR-0036) and errors-only (ADR-0039)** — a disabled
  construct must be *actionable*: named, located, and told which flag enables
  it. A generic `P0002` that misidentifies the location weakens that signal.
* **Keep the AST honest** — the DSL types (`compiler/dsl/src/`) describe the
  language, not the provenance of a particular parse path. Marker fields whose
  only purpose is to carry "which syntactic route produced this node" downstream
  are a smell to avoid where the structure can be re-derived.

## Considered Options

* **Gate the grammar on options** — thread `&CompilerOptions` as a rust-peg
  grammar argument and guard vendor alternatives with `&{ options.allow_x }`.
  Single source of truth for acceptance, but couples the grammar to runtime
  policy, forks rule behavior per flag, is awkward inside `precedence!{}`, and —
  unless every guarded alternative is written to emit a message rather than just
  fail — degrades disabled features to `P0002 SyntaxError`.
* **Diagnose in a policy phase with per-feature codes** — keep the grammar
  maximal and option-free; reject disabled constructs in a phase *outside* the
  grammar, at the earliest point that has both options and enough context to
  name the feature, emitting a dedicated `P####` per feature.
* **Status quo** — continue case-by-case: demotion where a token distinguishes
  the feature, and parse-permissive-plus-AST-marker-plus-analyzer-rule for
  structural cases, with no stated rule for which to use or where the reject
  belongs.

## Decision Outcome

Chosen option: **Diagnose in a policy phase with per-feature codes.**

**Dialect and feature-flag violations are diagnosed in a phase outside the
grammar, never by making the grammar's acceptance conditional on options.** The
PEG grammar stays maximal and option-free: it recognizes the superset of every
dialect, and its shape is identical in every configuration. This is what mature
compilers do — parse a permissive superset, then emit a specific, feature-named
diagnostic in a dedicated pass (rustc's feature gates → `E0658`; Clang's
`ExtWarn` extension diagnostics; Roslyn's "feature X is not available in C# N").

Four rules follow.

### 1. The grammar is not gated on options

Do not thread `CompilerOptions` into `plc_parser` to guard rules. Keeping the
grammar option-free preserves single-grammar reasoning (ADR's first driver) and
sidesteps the recursion/precedence and `precedence!{}` complications of runtime
guards. The grammar's job is to *recognize*; deciding whether a recognized
construct is *permitted in this dialect* is policy, and lives elsewhere.

### 2. Every `--allow-*` feature has its own problem code

A disabled feature is reported with a dedicated `P####` whose message names the
construct and the flag that enables it — the convention already established by
`P0004`, `P0011`, `P4028`, `P4029`, `P4033`. The code is documented in
`docs/reference/compiler/problems/P####.rst`. `P0002 SyntaxError` is **reserved
for genuinely unrecognizable input** — text that corresponds to no construct in
the maximal grammar. Any path where a *recognizable but disabled* feature
surfaces as `P0002` is a defect, to be fixed by recognizing the construct and
emitting its specific code.

### 3. Reject at the earliest phase that can name the feature

Placement follows *what the illegality is visible in*:

| The disabled feature is distinguishable by… | Mechanism | AST marker |
| --- | --- | --- |
| a single token (`//`, `.%Xn`, a keyword being present) | token rejection rule (`rule_token_no_*`), pre-parse, has options | none |
| structure only (located var in a plain block, expression where a literal is required) | a dialect-policy pass over the freshly parsed AST | avoid — see rule 4 |

Prefer a **token rejection rule over demotion when the word is not a plausible
identifier in standard code.** Demotion silently turns the keyword into an
identifier and lets the parse fail elsewhere with a generic `P0002`; a rejection
rule keeps the token and emits a specific code (e.g. "`REF_TO` requires
`--allow-ref-to`"). Reserve *demotion* for words that genuinely double as
identifiers in conforming programs (`AND_THEN`, `TIME`), where the trade — a
specific error versus letting existing code use the word as a name — is real
and resolves in favor of demotion.

### 4. Prefer re-deriving structure over adding an AST marker

The provenance markers in the structural cases (`in_mixed_var_block`,
`SimpleExpr`) exist only because rejection was deferred to the *analyzer*, which
had already flattened away the distinguishing structure. Run the policy check
while that structure is still present — in a pass close to parse — and the
marker is unnecessary: "a located variable inside a non-located `VAR` block" is
derivable from the block kind and the variable's location alone. Add a marker
only when the syntactic distinction is genuinely erased by the AST and cannot be
re-derived, and treat that as the exception, not the default.

## Consequences

* Good, because the grammar has one shape in every dialect: it is read, tested,
  and reasoned about once, as the superset. Disabling a flag never changes what
  the grammar *recognizes*, only what the policy phase *permits* — so
  `L(dialect) ⊆ L(superset)` holds by construction.
* Good, because every disabled feature is actionable: a specific `P####` that
  names the construct, points at its real span, and states the enabling flag —
  consistent with ADR-0036 (actionable rejections) and ADR-0039 (errors, not
  softened warnings).
* Good, because `P0002 SyntaxError` regains a precise meaning — "this is not any
  construct we recognize" — instead of doubling as the catch-all for
  disabled-but-valid syntax.
* Good, because the AST stops accumulating parse-provenance marker fields whose
  only consumer is a downstream gate.
* Neutral, because a new structural feature still needs its own policy check and
  its own `P####`; the work moves from "add a marker + analyzer rule" to "add a
  policy check + problem code," which is comparable effort with a better result.
* Bad, because a small amount of context that the parser had (the exact rule that
  matched) must be re-established in the policy pass from the AST. This is the
  cost of keeping the grammar option-free, and rule 4 bounds it: re-derive where
  possible, marker only where the structure is truly gone.
* Bad, because contributors cannot reach for a grammar guard to make a feature
  "just not parse" when off; they must recognize it and reject it with a named
  code. As with ADR-0039, this friction is the point — it forces an actionable
  diagnostic instead of a generic failure.

## More Information

### Relationship to the syntax support guide

[`syntax-support-guide.md`](../steering/syntax-support-guide.md) documents token
demotion, the token rejection rule, and token transforms — all of which assume a
distinguishing token. This ADR adds the missing case (structural extensions with
no distinguishing token) and the cross-cutting rules on problem-code specificity
and marker avoidance. The guide's decision table for "token demotion vs. token
validation rule" is subsumed by rule 3 here and should be updated to point at
this ADR and to add the structural row.

### Relationship to ADR-0036 (No IronPLC Dialect)

ADR-0036 kept every accepted configuration tied to something real and insisted
rejections stay actionable rather than silently smoothed over. This ADR is the
mechanism side of that principle: it says *where* a rejection happens and
guarantees it carries a specific, feature-named code instead of a generic syntax
error.

### Relationship to ADR-0038 and ADR-0039

ADR-0038 established that `--allow-*` flags are a flat, freely composable set;
this ADR keeps the grammar independent of that set so composition never changes
grammar shape. ADR-0039 established errors-only diagnostics; per-feature codes
here are errors that name the enabling flag, not demotable warnings.

### What mainstream compilers do

The "parse a superset, gate in a later pass with a specific code" shape is the
common one: rustc parses unstable syntax and emits `E0658` from a dedicated
feature-gate pass; Clang parses language-version and GNU extensions and reports
them through the `ExtWarn` system (e.g. `-Wc99-extensions`); Roslyn produces a
full node for too-new C# syntax and the checker reports "feature 'X' is not
available in C# N." In each case the parser recognizes first and a policy layer
diagnoses specifically — none makes the core grammar conditional on the
language mode.

[`Dialect`]: ../../compiler/parser/src/options.rs
[`CompilerOptions`]: ../../compiler/parser/src/options.rs
[#1199]: https://github.com/ironplc/ironplc/issues/1199
[PR #1220]: https://github.com/ironplc/ironplc/pull/1220
[PR #1221]: https://github.com/ironplc/ironplc/pull/1221
