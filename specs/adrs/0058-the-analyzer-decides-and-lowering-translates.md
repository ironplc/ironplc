# The Analyzer Decides and Lowering Translates

status: proposed
date: 2026-10-07

## Context and Problem Statement

With a lowering stage between analysis and code generation, each decision the
language makes could be made in one of two places: in analysis, which records
it, or in lowering.

Some decisions can make a program invalid. The type of an untyped literal is
one: the literal may not fit it. Others cannot: where a value is narrowed
before it is stored is the same for every valid program.

[ADR-0056](0056-analyzer-records-implicit-conversions-in-the-ast.md) chose
analysis for implicit conversions. Analysis now records the conversions of most
operands, and the type of every untyped literal.

Lowering runs only on a program analysis accepted. A problem that lowering
found would therefore appear only once every other problem in the project was
fixed. `check` and the language server show what analysis reports.

Where is each decision made?

## Decision Drivers

* **A problem with a program is reported beside every other problem**, in
  `check` and in the language server.
* **One recorded answer for each decision**, which every later stage reads.
* **The language server can show a decision** without running lowering.
* **No check predicts a decision** that another stage makes.

## Considered Options

* Analysis makes the decisions that can make a program invalid; lowering makes
  the rest and translates
* Lowering decides as well as translates
* Analysis records every decision; lowering only translates

## Decision Outcome

Chosen option: "Analysis makes the decisions that can make a program invalid;
lowering makes the rest and translates", because it keeps every problem with a
program in analysis, without recording decisions no valid program can get
wrong.

* **Analysis** makes every decision whose outcome can make a program invalid,
  and records it.
* **Lowering** makes the decisions no valid program can get wrong, and
  translates what analysis recorded. It does not make a decision analysis
  makes.
* **Lowering never reports that a program is invalid.** It reports only a
  construct the compiler cannot generate yet, and compiler defects.
* **`check` runs lowering**, so it also reports a construct the compiler cannot
  generate yet.

This extends ADR-0056 and does not supersede it. ADR-0056 calls its pass "a
lowering pass". That pass records decisions rather than lowering anything, so
it is now the **recording pass**, and "lowering" names only the stage after
analysis.

### Consequences

* Good, because every problem with a program is reported by analysis, beside
  every other problem.
* Good, because the language server can show a recorded decision without
  running lowering, as ADR-0056 intends.
* Good, because a backend has nothing to decide.
* Good, because `check` reports a construct the compiler cannot generate yet,
  which today only `compile` reports.
* Bad, because the analyzed program holds analysis's decisions as well as what
  the program wrote, as ADR-0056 already accepted.
* Bad, because analysis must record a decision before lowering can rely on it,
  which orders the work.
* Bad, because each new decision has to be placed on one side of the line or
  the other.

## Pros and Cons of the Options

### Analysis makes the decisions that can make a program invalid; lowering makes the rest and translates (chosen)

* Good, because a problem is reported by analysis, where every other problem
  is.
* Good, because analysis records only the decisions that matter to it and to
  the language server.
* Bad, because the line has to be drawn for each new decision.

### Lowering decides as well as translates

Lowering would choose literal types and conversions, and report the checks that
depend on them. ADR-0056 would be superseded.

* Good, because the analyzed program would stay closer to what was written.
* Bad, because a literal that does not fit its type would disappear from
  `check` and the language server whenever another error existed anywhere in
  the project.
* Bad, because the language server could show a conversion only by running
  lowering.

### Analysis records every decision; lowering only translates

* Good, because lowering would make no decision at all.
* Bad, because analysis would record decisions that neither it nor the language
  server reads, such as where to narrow a value.

## More Information

ADR-0056 has a postscript pointing here.
