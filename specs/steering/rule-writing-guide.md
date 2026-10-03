# Rule Writing Guide

**Bottom line:** a rule is a single `rule_*.rs` module. It looks names up in the
environments and pushes diagnostics without ever failing. It is registered in
the pipeline, and its tests call its own `apply` and assert the exact list of
problems it reports.

This guide covers analyzer semantic rules (`compiler/analyzer/src/rule_*.rs`)
and parser token rules (`compiler/parser/src/rule_*.rs`). To add the syntax a
rule checks, see [syntax-support-guide.md](syntax-support-guide.md).

## Checklist

- [ ] **Problem code**: add the code to the CSV and write `docs/compiler/problems/P####.rst`
  ([problem-code-management.md](problem-code-management.md)).
- [ ] **Module**: create `rule_<what_it_checks>.rs`. Start it with a `//!` doc
  comment that says what the rule rejects and why, followed by a `## Passes`
  and a `## Fails` example.
- [ ] **`apply`**: give it the signature
  `pub fn apply(lib: &Library, context: &SemanticContext, options: &CompilerOptions) -> SemanticResult`.
  The body calls `rule_support::run_rule(visitor, lib)`. A token rule instead
  takes `(tokens: &[Token], options: &CompilerOptions)`.
- [ ] **Visitor**: implement `DiagnosticVisitor`. Report a problem by pushing
  onto `diagnostics`. Never return early
  ([Error Handling](compiler-architecture.md#error-handling)).
- [ ] **Lookups**: read names and types from `context.symbols()` and
  `context.types()`, using `ScopeTracker` for the current scope. Never build a
  table of your own
  ([Name and Type Lookup](compiler-architecture.md#name-and-type-lookup)).
- [ ] **Options**: if a `CompilerOptions` flag permits the construct, return
  `Ok(())` from `apply` before the walk. `rule_case_bit_string_label.rs` shows
  this.
- [ ] **Register**: add `mod` and `apply` to `analyzer/src/stages.rs`. For token
  rules, use `parser/src/lib.rs` instead.
- [ ] **Tests**: use `rule_ok!`, `rule_err!`, `rule_err_at!` and the shared
  helpers, or `token_rule_ok!` and `token_rule_err!` for token rules
  ([Rule Tests](compiler-standards.md#rule-tests)). Include at least one test
  that passes, one test per problem the rule reports, and one `rule_err_at!`
  that checks the label lands on the offending text.
- [ ] **CI**: run `cd compiler && just`. The rule-test conventions check
  (`test_rule_conventions.rs`) fails a test that breaks these rules.

## Pitfalls

- **One concern per rule.** If two problems need different walks, write two
  rules.
- **Label the offending text, not its container.** Point the label at the
  argument, name or literal, not at the whole POU.
- **Don't re-report what resolution reports.** If an `xform_*` pass already
  reports a problem, a rule that reports it too shows the user the problem
  twice.
- **Missing information belongs in the environment.** When a rule needs
  something the environment lacks, add it there instead of working around it
  in the rule.
