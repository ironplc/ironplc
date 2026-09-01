# Semantic Rules Accumulate Diagnostics and Cannot Fail

status: accepted
date: 2026-09-01

## Context and Problem Statement

A semantic rule in `compiler/analyzer/src/rule_*.rs` implements
`ironplc_dsl::visitor::Visitor<E>` and walks the merged `Library`. The error
type `E` was `Diagnostic` for every rule, which offered two ways to report a
problem: push it onto a `Vec<Diagnostic>` the rule owns, or return it as `Err`.

The second is a trap. `recurse_visit` propagates `Err` with `?`, so returning a
diagnostic unwinds the whole walk and every problem in the rest of the library
goes unreported. Eleven of the thirty-one rules did exactly that, most of them
ending `apply` with

```rust
visitor.walk(lib).map_err(|e| vec![e])
```

--- a wrapper that reads like a collection but can hold at most one element.

The user-visible result (issue #1566) was that `ironplcc check` reported one
problem per rule per invocation. A file with two undefined variables reported
one. Worse, because `xform_toposort_declarations` reorders declarations, the
surviving diagnostic was not even the first one in the file: adding a second
file to the invocation made the first file's problems vanish, with nothing to
say they had not been checked.

`specs/steering/compiler-standards.md` already said to "collect multiple
diagnostics rather than failing on the first, where practical". The rules that
got it wrong were not disagreeing with that; the type they implemented offered
them a shorter way to be wrong, and review did not catch it eleven times.

## Considered Options

* **Fix the eleven rules.** Convert each to accumulate, leave
  `Visitor<Diagnostic>` in place.
* **Fix the eleven rules and remove the error channel.** Also make
  `DiagnosticVisitor` --- the trait `rule_support::run_rule` drives ---
  require `Visitor<Infallible, Value = ()>`.
* **Change `Visitor<E>` itself** so `E` is always a diagnostic accumulator.

## Decision Outcome

Chosen option: **fix the eleven rules and remove the error channel**.

`DiagnosticVisitor: Visitor<Infallible, Value = ()>`. `Infallible` has no
values, so a rule visitor has no `Err` to return and no early exit to write.
Reporting a problem means pushing onto the accumulator that `into_diagnostics`
surrenders, which leaves the walk running and the rest of the library checked.
Getting this wrong is now a compile error rather than something a reviewer has
to notice.

Fixing the eleven rules without this leaves the trap in place for the twelfth.
The three sites the compiler flagged in `rule_var_decl_const_initialized` ---
a rule whose `apply` looked correct, because the short-circuit was inside its
visitor rather than in the `map_err` idiom --- are the argument: a grep for the
idiom found ten rules, and the type found one more.

Changing `Visitor<E>` itself was rejected as too broad. The twelve `xform_*`
passes share the trait and have a real reason to stop: a transform must return
a `Library`, and `stages::resolve_types` reverts to a pre-pass clone when one
fails. The constraint belongs on rules, which produce only diagnostics, not on
every visitor.

### Consequences

* Good, because the whole class of bug is gone at compile time, in a codebase
  where it recurred across eleven rules.
* Good, because it forces the question at the right moment: a rule author who
  wants to stop must decide what to stop --- descending into one node, which is
  usually right --- rather than reaching for `?`.
* Bad, because a rule that hits a genuinely unanalysable node cannot abandon
  the walk. In practice it should not: it records a diagnostic (`internal_error`
  or `not_implemented`) and stops descending into that node, which is what the
  converted rules now do.
* Neutral, because `Infallible` appears in every rule's `impl Visitor<...>`
  header. `run_rule` absorbs it; rule bodies are unchanged apart from the
  signature.

## More Information

* Issue [#1566](https://github.com/ironplc/ironplc/issues/1566)
* `compiler/analyzer/src/rule_support.rs` --- `DiagnosticVisitor` and `run_rule`
* Diagnostic *ordering* is not addressed here. Because the toposort reorders
  declarations, diagnostics are emitted in dependency order rather than file
  order. That no longer hides anything, but the output is not sorted by file
  and position.
