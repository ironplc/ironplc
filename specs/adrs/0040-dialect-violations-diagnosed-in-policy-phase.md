# ADR-0040: Dialect Violations Are Diagnosed in a Policy Phase, Not by Gating the Grammar

status: proposed
date: 2026-07-24

## Context and Problem Statement

IronPLC accepts Structured Text under several configurations — the IEC 61131-3
editions and the vendor-compatible dialects (`rusty`, `codesys`, and TwinCAT,
which is CODESYS-derived) — expressed as [`Dialect`] presets plus per-feature
`--allow-*` flags on [`CompilerOptions`] (ADR-0036, ADR-0038). The set of
supported dialects is expected to grow, so the mechanism for gating dialect
features has to scale to many dialects, not just today's. A recurring question
is *where* the compiler should reject a construct that is syntactically
recognizable but disabled by the current dialect, and *what* it should report
when it does.

The parser is deliberately option-free. `parse_library`/`parse_statements`
(`compiler/parser/src/parser.rs`) take only tokens; [`CompilerOptions`] never
reaches the PEG grammar. Instead, options are "compiled away" into the token
stream *before* parsing, inside `tokenize_program`
(`compiler/parser/src/lib.rs`). Conditional features are handled by mechanisms
at two points in the pipeline.

**Pre-parse, at the token level** (documented in
[`syntax-support-guide.md`](../steering/syntax-support-guide.md)):

1. **Token demotion** (`xform_demote_*.rs`) — a keyword token is rewritten to
   `Identifier` when its flag is off, so the vendor grammar path structurally
   cannot match and the word remains usable as an ordinary identifier. This was
   introduced to handle syntax that is otherwise ambiguous or hard to parse, and
   it is a mechanism the project intends to keep.
2. **Token rejection rule** (`rule_token_no_*.rs`) — the lexer always tokenizes
   the construct; a token-level rule with access to options emits a specific
   diagnostic when the flag is off.
3. **Token transform** (`xform_tokens.rs`) — fix up the stream (e.g. missing
   semicolons).

**Post-parse, over the AST** — the analyzer already rejects option-gated
constructs after parsing (e.g. `rule_ref_to.rs` gates on the `--allow-ref-*`
flags) and emits specific codes. So "reject after parsing" is not a new idea;
it is an established mechanism in the codebase, and this ADR builds on it rather
than inventing it.

The pre-parse mechanisms all require a *distinguishing token*. This breaks down
for **structural extensions**, where the vendor grammar overlaps the standard
grammar with nothing to demote — a constant *expression* where the standard
allows only a literal initializer ([PR #1220]), or an `AT`-located variable
inside an ordinary `VAR` block ([PR #1221]). For these, the TwinCAT series
([#1199]) parsed permissively and carried the syntactic distinction downstream
to the analyzer through an **AST provenance marker**
(`InitialValueAssignmentKind::SimpleExpr`, `in_mixed_var_block`). Rejecting
after parsing is fine; smuggling parse-path provenance through a marker field on
the DSL type is the part that is new and uncomfortable — it smears the "is this
legal in this dialect?" question across the parser, a marker on the AST, and an
analyzer rule.

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
* **Keep code analysis and generation simple** — downstream stages (semantic
  analysis, code generation) should see a clean, uniform AST and not branch on
  dialect provenance. This is the constraint that led to *collapsing* distinct
  declaration shapes into one representation in the first place; it is real and
  the policy design must respect it (see rule 4).
* **Keep the AST honest** — the DSL types (`compiler/dsl/src/`) describe the
  language, not the provenance of a particular parse path. Marker fields whose
  only purpose is to carry "which syntactic route produced this node" downstream
  are a smell to avoid where the structure can be represented faithfully or
  derived from the tree.

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

The one accepted exception is a **demoted keyword** (rule 3). Demotion
intentionally makes the word indistinguishable from an identifier when the flag
is off, so a program that wrote it expecting the feature is, in that
configuration, genuinely ambiguous input — the compiler cannot know the keyword
was intended. A `P0002` or undeclared-identifier outcome there is expected, not
a defect. This is the deliberate cost of demotion, accepted because the
ambiguity it resolves is real.

### 3. Reject at the earliest phase that can name the feature

Placement follows *what the illegality is visible in*:

| The disabled feature is distinguishable by… | Mechanism |
| --- | --- |
| a token whose spelling could also be a valid identifier (`REF_TO`, `AND_THEN`, `TIME`) | **token demotion** (`xform_demote_*`), pre-parse |
| a token that is unambiguously non-identifier syntax (`//`, `.%Xn`) | token rejection rule (`rule_token_no_*`), pre-parse |
| structure only (located var in a plain block, expression where a literal is required) | a post-parse policy/analyzer check over the AST |

**Token demotion stays a first-class mechanism, not a fallback to migrate away
from.** It exists precisely because a keyword's spelling can also be a legal
identifier, and *we cannot assume any keyword is never used as an identifier* —
`REF_TO`, `AND_THEN`, `TIME`, and the like can all legitimately appear as
variable or type names in real programs. Demoting them to `Identifier` when
their flag is off is the correct way to keep those programs parsing. Its one
inherent cost is the error *message* in the opposite case (a program that meant
the keyword but had the flag off): because the word is deliberately made
identifier-shaped, the compiler cannot tell the two apart, so a specific "you
meant the keyword" message is not generally possible there (rule 2's exception).
That trade is accepted; it does not justify replacing demotion with a rejection
rule, which would break the identifier use that demotion is there to protect.

Use a **token rejection rule** only for syntax that *cannot* be confused with an
identifier (so nothing legitimate is lost by rejecting it), and a **post-parse
check** for structural cases, where there is no token to key on at all.

### 4. Expose structure through AST accessors, rather than collapsing it or marking it

The provenance markers in the structural cases (`in_mixed_var_block`,
`SimpleExpr`) trace back to an earlier decision to *collapse* distinct
declaration shapes into one AST representation to keep code generation simple
(the "keep analysis and generation simple" driver). Collapsing discards the
distinction the policy check later needs, which then has to be re-introduced as
a marker field — the collapse and the marker are two halves of the same mistake.

The better shape is neither collapse-then-mark nor a runtime grammar guard: keep
the AST faithful to the source and **offer functions on the AST that derive the
properties each stage cares about.** For example, an accessor that reports
whether a declaration is located, or whether a `VAR` block mixes located and
plain declarations. Then:

* analysis and code generation call the accessor and stay simple — they get the
  uniform view they wanted without the AST having to be lossy;
* the policy check calls the *same* accessor to decide whether the construct is
  permitted in the current dialect;
* no provenance flag is threaded through the DSL, and the tree still describes
  the program as written.

A dedicated marker field is the last resort — only when the distinction cannot
be represented faithfully in the tree or derived from it by an accessor.

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
* Good, because the AST stays faithful to the source and stops accumulating
  parse-provenance marker fields; analysis and codegen get their simple, uniform
  view through accessors instead of through a lossy collapse.
* Neutral, because a new structural feature still needs its own policy check and
  its own `P####`; the work moves from "collapse + marker + analyzer rule" to
  "AST accessor + policy check + problem code," which is comparable effort with a
  better result.
* Bad, because some context the parser had (the exact rule that matched) must be
  re-derived from the AST by an accessor. This is the cost of keeping the grammar
  option-free and the AST honest, and rule 4 bounds it: derive via an accessor
  where possible, marker only where the structure cannot be represented at all.
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
