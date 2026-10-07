# The Analyzer Decides and Lowering Translates

status: proposed
date: 2026-10-07

## Context and Problem Statement

With a lowered program between analysis and code generation
([ADR-0057](0057-backends-consume-a-target-neutral-lowered-program.md)), each
decision the language makes could be made in one of two places: by the
analyzer, which records it in the analyzed `Library`, or by lowering.

[ADR-0056](0056-analyzer-records-implicit-conversions-in-the-ast.md) chose the
analyzer for implicit conversions. Its pass, `xform_insert_implicit_conversions`,
now records the conversions of comparison and arithmetic operands, assigned
values and the arguments of user-defined functions, and the type of every
untyped literal.

Lowering runs only behind the clean-analysis gate. A problem that lowering
found would appear only once every other problem in the project was fixed.
`ironplcc check` and the language server show what analysis reports.

Where is each decision made?

## Decision Drivers

* **A problem with the program is reported beside every other problem**, in
  `check` and in the language server.
* **One recorded answer per decision**, which every consumer reads.
* **The language server can show a decision** without running lowering.
* **No rule predicts a decision** that another stage makes.

## Considered Options

* The analyzer makes the decisions that can make a program invalid; lowering
  makes the rest and translates
* Lowering decides as well as translates
* The analyzer records every decision; lowering only translates

## Decision Outcome

Chosen option: "The analyzer makes the decisions that can make a program
invalid; lowering makes the rest and translates", because it keeps every
problem with the program in analysis without recording in the AST decisions no
valid program can get wrong.

* **The analyzer** makes every decision whose outcome can make a program
  invalid, and records it in the `Library`: the type of a literal, an implicit
  conversion, an arithmetic overload, a comparison's operand type, the order
  of arguments, the capacity of a string declared without one, and the ordinal
  of each enumeration member.
* **Lowering** makes the decisions no valid program can get wrong: narrowing
  before a store, a logical or a bitwise operator, the callee, how each
  argument is passed, variable identity, default initial values, the target
  of `EXIT` and `CONTINUE`.
* **Lowering decides no literal type and no implicit conversion.** It
  translates each `ImplicitConversion` into a `Convert`, and reports an
  operand that needs a conversion the analyzer did not record as P9998.
* **Lowering reports only P9999 and P9998:** a construct it cannot express yet,
  or a compiler defect. It never reports that a program is invalid. It
  finishes its walk and reports every problem it found, as rules do
  ([ADR-0048](0048-semantic-rules-cannot-fail.md)).
* **`check` runs lowering** for every POU reachable from a program instance,
  so it reports a construct the compiler cannot generate yet. `compile`
  passes on the lowered program `check` produced.
* **Lowering reads `CompilerOptions`; a backend does not.** A behaviour policy
  ([ADR-0049](0049-behavior-policies-selected-at-compile-time.md)) chooses an
  operation during lowering.

This extends ADR-0056 and does not supersede it. ADR-0056's order stands: its
pass runs after the semantic rules, so a rule sees the program as written.
`rule_constant_range`, which checks literals against their recorded types, runs
after the pass and reads the operand types as written through each conversion.

ADR-0056 calls its pass "a lowering pass". It records decisions rather than
lowering anything, so it is now the **recording pass**, and "lowering" names
only the stage after analysis.

### Consequences

* Good, because every problem with the program is reported by analysis, in
  `check` and the language server, beside every other problem.
* Good, because the language server can show a recorded decision from the
  analyzed tree, as ADR-0056 intends.
* Good, because a second backend has nothing to decide.
* Good, because `check` reports a construct the compiler cannot generate yet,
  which today surfaces only from `compile`.
* Bad, because the analyzed `Library` holds the analyzer's decisions as well as
  what the program wrote, as ADR-0056 already accepted.
* Bad, because the analyzer must record a decision before lowering can cover a
  construct that depends on it, which orders the work.

### Confirmation

The requirements of `specs/design/lowered-program.md` confirm this decision:

* A test walks the analyzed `Library` of a corpus of programs and finds no
  literal left at a generic category and no operand left unconverted
  (REQ-LOW-analyzer-091).
* Lowering's tests assert that a literal lowers to a `Const` of the recorded
  type, and that an operand without a recorded conversion is a P9998
  (REQ-LOW-lowering-094).
* Lowering reports no problem code other than P9999 and P9998, and reports
  two independent problems in one run (REQ-LOW-lowering-109,
  REQ-LOW-lowering-110).
* Analysis reports P2026 for both `x := 300` on a `USINT` and `DINT#300 < s`
  on a `SINT` (REQ-LOW-analyzer-093).

## Pros and Cons of the Options

### The analyzer makes the decisions that can make a program invalid; lowering makes the rest and translates (chosen)

* Good, because a problem is reported by analysis, where every other problem
  is.
* Good, because the AST records only the decisions that matter to analysis.
* Bad, because the division has to be decided for each new decision.

### Lowering decides as well as translates

Lowering chooses literal types and conversions, and reports the checks that
depend on them. ADR-0056 would be superseded.

* Good, because the analyzed tree would stay closer to the source.
* Bad, because lowering runs only on a clean analysis, so `x := 300` on a
  `USINT` would disappear from `check` and the language server whenever
  another error existed anywhere in the project.
* Bad, because the language server could show a conversion only by running
  lowering.

### The analyzer records every decision; lowering only translates

* Good, because lowering would make no decision at all.
* Bad, because the AST would carry decisions no valid program can get wrong,
  such as where to narrow a value, which neither analysis nor the language
  server reads.

## More Information

The design is `specs/design/lowered-program.md`, section "Decisions" and
section "Relationship to ADR-0056". ADR-0056 has a postscript pointing here.
