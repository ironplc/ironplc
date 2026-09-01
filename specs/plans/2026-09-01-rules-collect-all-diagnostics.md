# Semantic rules collect all diagnostics

Fixes [#1566](https://github.com/ironplc/ironplc/issues/1566).

## Goal

`ironplcc check` reports every semantic problem a rule finds, not the first one.
Today a file with two undefined variables reports one, and adding a second file
to the invocation makes the first file's diagnostics disappear entirely.

## Background

`stages::semantic()` concatenates the `Vec<Diagnostic>` returned by each of the
30 rules, so the pipeline above the rules is already collecting. The ceiling is
inside a rule: ten rules end `apply` with

```rust
visitor.walk(lib).map_err(|e| vec![e])
```

The visitor's error type is a single `Diagnostic`, so `walk` unwinds at the
first `Err` and the `vec![e]` wrapper — which reads like a collection — can only
ever hold one element. Whichever declaration the toposort happens to visit last
is the one whose diagnostic survives, which is why adding `b.st` hides `a.st`.

Of those ten, four (`rule_abstract_not_instantiated`,
`rule_case_bit_string_label`, `rule_mixed_located_var_declarations`,
`rule_struct_initializer_expression_allowed`) already accumulate into their own
`diagnostics` field and never return `Err` — they hand-roll the boilerplate that
`rule_support::run_rule` exists to provide, and the `map_err` is dead code. The
remaining five genuinely short-circuit.

This contradicts `specs/steering/compiler-standards.md`: "Collect multiple
diagnostics rather than failing on the first, where practical". The issue notes
we get this wrong often. We do, because nothing stops us: `Visitor<Diagnostic>`
offers an error channel that looks like the natural way to report a problem.

## Architecture

Take the error channel away from rule visitors. `DiagnosticVisitor` — the trait
`run_rule` drives — becomes `Visitor<Infallible, Value = ()>`. A rule visitor
then has no way to say "stop here": the only way to report anything is to push
onto the accumulator that `into_diagnostics` surrenders. The class of bug is
gone at compile time rather than by review.

`run_rule` already exists and 21 rules already use it, so this is a tightening
of an established shape, not a new abstraction.

## Prefactoring

Two shape changes before the behaviour change:

1. Move the four hand-rolled accumulator rules onto `run_rule`. Behaviour
   preserving — they already collect; this only deletes duplicated boilerplate
   and the dead `map_err`.
2. Add `rule_errn!` / `rule_errn_with!` / `rule_ctx_errn!` to
   `analyzer/src/test_macros.rs`, the multi-diagnostic counterparts of the
   existing `rule_err1!` family. Without these every new "reports both" test is
   a hand-written 6-line body, which is the duplication the macros were
   introduced to remove.

The `Infallible` tightening cannot come first: the five short-circuiting rules
would not compile against it. It lands last, as the guard rail on work already
done.

## File map

Prefactor:
- `compiler/analyzer/src/rule_abstract_not_instantiated.rs`
- `compiler/analyzer/src/rule_case_bit_string_label.rs`
- `compiler/analyzer/src/rule_mixed_located_var_declarations.rs`
- `compiler/analyzer/src/rule_struct_initializer_expression_allowed.rs`
- `compiler/analyzer/src/test_macros.rs`

Fix:
- `compiler/analyzer/src/rule_function_block_invocation.rs`
- `compiler/analyzer/src/rule_method_call_declared.rs`
- `compiler/analyzer/src/rule_use_declared_enumerated_value.rs`
- `compiler/analyzer/src/rule_use_declared_symbolic_var.rs`
- `compiler/analyzer/src/rule_var_decl_global_const_requires_external_const.rs`
- `compiler/analyzer/src/call_assignment_check.rs` (shared by two of the above)
- `compiler/analyzer/src/stages.rs` (test)

Guard rail:
- `compiler/analyzer/src/rule_support.rs`
- every `compiler/analyzer/src/rule_*.rs` with an `impl Visitor<Diagnostic>`
- `specs/adrs/0048-semantic-rules-accumulate-diagnostics.md`
- `specs/steering/compiler-architecture.md`
- `specs/steering/compiler-standards.md`

## Tasks

- [ ] Prefactor: four rules onto `run_rule`
- [ ] Prefactor: `rule_errn!` test macros
- [ ] `call_assignment_check::check_assignments` returns `Vec<Diagnostic>`
- [ ] Five short-circuiting rules accumulate
- [ ] Multi-diagnostic test per converted rule
- [ ] `stages.rs` test asserting a two-problem program yields two diagnostics
- [ ] `DiagnosticVisitor: Visitor<Infallible>`; convert all rule visitors
- [ ] ADR + steering updates
- [ ] `cd compiler && just`
- [ ] `git rm` this plan

## Out of scope

- **Diagnostic ordering.** `xform_toposort_declarations` reorders declarations,
  so diagnostics come out in dependency order rather than file order. That no
  longer hides anything once all diagnostics are collected, but the output is
  still not sorted by file and position. Separate change.
- **The twelve `xform_*` passes** with the same `map_err(|e| vec![e])` idiom. A
  transform must produce a `Library`, so aborting is defensible in a way it is
  not for a rule; `stages::resolve_types` already reverts to a pre-pass clone on
  failure. Reshaping those is its own change.
