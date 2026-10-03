# Plan: One Shape for Analyzer Rule Tests

## Goal

Make every semantic rule test assert the same thing in the same way, migrate
the existing tests to that shape, and add a check that fails the build when a
new rule test drifts from it.

## Survey

Scope: the 44 `compiler/analyzer/src/rule_*.rs` rules (plus the
`rule_constant_range/`, `rule_function_call_type_check/` and `rule_ref_to/`
test files) and the 5 `compiler/parser/src/rule_*.rs` token rules. Counts are
from a scan of the `#[cfg(test)]` modules on `e97f481`.

### How tests are written

| Form | Count | Notes |
|---|---|---|
| `test_macros.rs` one-liners | 389 | 21 macros: fresh vs. resolved context × {ok, err, err_code, err1, err1_at, errn} × {default, `_with` options} |
| Hand-written `#[test]` / `#[rstest]` bodies | 240 | 40 of them `#[rstest]`, carrying 368 `#[case]`s |
| Property tests (`proptest`) | 0 | `proptest` is used only in `vm` and `codegen`; no rule uses it |

Every analyzer rule shares one signature,
`apply(&Library, &SemanticContext, &CompilerOptions) -> SemanticResult`, so one
helper can drive every rule.

### What the tests assert

From weakest to strongest:

| Assertion | Where | Count |
|---|---|---|
| Full `analyze` pipeline produced *some* diagnostic | `rule_ref_to/tests.rs` (`assert_err`), `rule_bit_and_partial_access_range.rs` (`assert_bit_access_err`) | ~40 tests, ~60 cases |
| `apply(...)` is `Err`, no code checked | `rule_err!` (17), `rule_ctx_err!` (14), hand-written `.is_err()` in 7 analyzer + 4 parser files | ~65 |
| Some diagnostic has code `"P2037"` (string literal) | `rule_assignment_aggregate_type_compat.rs` (14), `rule_unsupported_extension.rs` (5), `rule_use_declared_symbolic_var.rs` (2), `rule_function_block_call_unsupported.rs` (1) | 22 |
| Some diagnostic has `Problem::X.code()` | `rule_err_code!` (5), `rule_ctx_err_code!` (2), hand-written | ~20 |
| Message text only, no code | `rule_member_qualifier_invalid.rs` (`messages(...)`) | 6 tests, 31 cases |
| Exactly one diagnostic, `Problem::X` | `rule_err1!` / `rule_ctx_err1!` (+`_with`) | 69 |
| Exactly N diagnostics, all `Problem::X` | `rule_errn!` / `rule_ctx_errn!` (+`_with`) | 19 |
| Exact list of codes | `rule_condition_type.rs` (`assert_eq!(codes_for(..), vec![p4072()])`) | 1 file |
| Exactly one `Problem::X`, label at the offending text | `rule_err1_at!` | 36 |

Ok-side tests are uniform (`rule_ok!`/`rule_ctx_ok!` or `.is_ok()`).

### Defects the variety hides

1. **Tests that pass for the wrong reason.** An "any error" assertion holds
   when the rule reports an unrelated problem, or reports the right one twice.
   When it runs the full `analyze` pipeline (`rule_ref_to`,
   `rule_bit_and_partial_access_range`), it holds when a *different rule*
   reports something.
2. **Rules tested against the wrong context.** `rule_pou_hierarchy.rs` (1
   test) and `rule_var_decl_const_initialized.rs` (8 tests) read
   `context.symbols()`/`context.types()` but are tested with the fresh-context
   macros, which hand the rule an empty context.
   [compiler-architecture.md](../steering/compiler-architecture.md) already
   says such rules must use `rule_ctx_*`.
3. **String codes.** `"P2037"` is not checked by the compiler, so a
   renumbered or mistyped code fails at run time, or never.
4. **Every `#[rstest]` writes its own scaffold.** The macros cannot take
   `#[case]` arguments, so the 40 parameterised tests each re-implement
   parse → resolve → apply → collect codes, 12 of them as private helpers
   (`codes_for`, `problem_codes`, `diagnostics_of`, `out_of_range_count`, …),
   and each chooses its own assertion.

## The Standard

A rule test calls the rule's own `apply`, never the full pipeline, and asserts
the **exact list of problems** it reports, written as `Problem` variants.

1. **Unit under test.** `super::apply`, given the library and the context
   that `parse_and_resolve_types_with_options` returns. Always the resolved
   context, never an empty one: a rule that ignores its `_context` gets the
   same result either way, and a rule that reads it is only tested
   correctly this way. Pipeline behaviour belongs in
   `spec_conformance_*.rs` or the `plc2x` tests.
2. **Assertion.** The ordered list of diagnostic codes equals an expected
   `&[Problem]`. An empty list means `Ok`. This one assertion covers "ok",
   "exactly one", "exactly N" and "mixed codes", and it catches duplicate,
   missing and wrong-code reports.
3. **Codes are `Problem` variants**, never `"P####"` literals.
4. **Location.** A rule whose diagnostic names a construct has at least one
   `rule_err_at!` test pinning the primary label to the offending text.
5. **Message text** is asserted only when the text is the feature (a
   "did you mean" hint, the named type), and always together with the code
   assertion, never instead of it.
6. **Parameterise with `#[rstest]`**, using the same helper as the macros so
   a case and a one-liner assert identically. Property tests are not needed:
   rule inputs are programs, and boundary values are enumerable as cases
   (as `rule_constant_range/tests.rs` does).
7. **Coverage per rule.** Every rule has at least one ok test and one test
   per `Problem` it can report.

### Shape

One helper does the work, and three macros wrap it.

```rust
// test_helpers.rs
pub type Rule = fn(&Library, &SemanticContext, &CompilerOptions) -> SemanticResult;

/// Resolves `program` under `opts`, runs `rule` against the resolved context,
/// and asserts it reports exactly `expected`, in order (empty means `Ok`).
pub fn assert_rule(rule: Rule, program: &str, opts: &CompilerOptions, expected: &[Problem]) { … }

/// As `assert_rule` with the one problem `expected`, whose primary label
/// points at the first occurrence of `at` in `program`.
pub fn assert_rule_at(rule: Rule, program: &str, opts: &CompilerOptions, expected: Problem, at: &str) { … }
```

```rust
// test_macros.rs: each expands to one #[test] fn calling super::apply;
// the trailing options argument is optional and defaults to CompilerOptions::default().
rule_ok!(name, program);
rule_ok!(name, program, opts);
rule_err!(name, program, [Problem::X]);
rule_err!(name, program, [Problem::X, Problem::X], opts);
rule_err_at!(name, program, Problem::X, "offending text");
rule_err_at!(name, program, Problem::X, "offending text", opts);
```

`rule_ok!(…)` is `assert_rule(super::apply, program, &opts, &[])`,
`rule_err!(…, [..])` is `assert_rule` with that list, and `rule_err_at!` is
`assert_rule_at`. As today, `#[…]` and `///` attributes before an invocation
are forwarded onto the generated test, so `#[spec_test]` and doc comments
survive.

Before (`rule_assignment_aggregate_type_compat.rs`):

```rust
#[test]
fn apply_when_array_extents_differ_then_reports_mismatch() {
    let codes = problem_codes(&program_with("a : ARRAY[1..3] OF INT; …", "a := b;"));
    assert!(codes.contains(&"P2037".to_string()), "got {codes:?}");
}
```

After:

```rust
rule_err!(
    apply_when_array_extents_differ_then_reports_mismatch,
    &program_with("a : ARRAY[1..3] OF INT; …", "a := b;"),
    [Problem::AggregateAssignmentTypeMismatch]
);
```

Or, as an `#[rstest]` case, calling the helper the macros call:

```rust
#[rstest]
#[case::extents("a : ARRAY[1..3] OF INT; b : ARRAY[1..4] OF INT;")]
#[case::element_type("a : ARRAY[1..3] OF INT; b : ARRAY[1..3] OF REAL;")]
fn apply_when_array_types_differ_then_reports_mismatch(#[case] decls: &str) {
    assert_rule(apply, &program_with(decls, "a := b;"), &CompilerOptions::default(),
        &[Problem::AggregateAssignmentTypeMismatch]);
}
```

### Macro set after the change

Three macros replace all 21 in `test_macros.rs` today:

| Today | After |
|---|---|
| `rule_ok`, `rule_ok_with`, `rule_ctx_ok`, `rule_ctx_ok_with` | `rule_ok!` |
| `rule_err1`, `rule_err1_with`, `rule_errn`, `rule_errn_with`, `rule_ctx_err1`, `rule_ctx_err1_with`, `rule_ctx_errn`, `rule_ctx_errn_with` | `rule_err!` with a list |
| `rule_err1_at` | `rule_err_at!` |
| `rule_err`, `rule_err_with`, `rule_ctx_err`, `rule_ctx_err_with`, `rule_err_code`, `rule_err_code_with`, `rule_ctx_err_code`, `rule_ctx_err_code_with` | deleted; each use rewritten as `rule_err!` with the exact list |

Three choices fold the variants away:

- **One context.** The empty-context family (`rule_*`, `resolve_fresh_with`)
  goes. It resolves the program just as the `rule_ctx_*` family does and then
  discards the context, so for a rule that ignores the context the two give
  the same result. For a rule that reads it, the empty context is the
  wrong-context defect above. With it gone, that defect cannot be written.
- **One list.** "Exactly one" and "exactly N of one code" are lists of one
  and of N. A list also covers mixed codes, which no macro covers today.
- **Optional options.** A trailing options argument replaces each `_with`
  twin.

`rule_err!` keeps its name but changes meaning, from "any error" to "exactly
these problems". Its old uses are all rewritten (Core PR 1) before the new
macro takes the name (Core PR 2), and the new form requires a `[…]` list, so
any old-style invocation that was missed fails to compile.

The parser token rules take `(&[Token], &CompilerOptions)`, so they get the
same helper and three macros over tokens in `parser/src/`; the standard is the
same.

## Enforcement

A build check is possible, in two layers.

**1. Compile time: only the three macros exist.** Once the migration is done,
`test_macros.rs` holds `rule_ok!`, `rule_err!` and `rule_err_at!` and nothing
else, and `resolve_fresh_with` is gone. An "any error" one-liner, an
empty-context one-liner, or a `rule_err!` without its list of problems fails
to compile. No new tooling.

**2. Test time: a conventions meta-test** for what the macros cannot stop:
hand-written and `#[rstest]` bodies. This follows the precedent of
`all_spec_requirements_have_tests` in `spec_conformance.rs`. Add
`analyzer/src/rule_test_conventions.rs`, a `#[test]` that reads every
`src/rule_*.rs` and `src/rule_*/*.rs` from `env!("CARGO_MANIFEST_DIR")`, takes
the text after `#[cfg(test)]` (or the whole of a `tests.rs`), and fails,
naming file and line, when it finds:

| Check | Catches |
|---|---|
| a `"P` + four digits literal | string codes |
| `.is_err()` or `has_diagnostics()` | "any error" assertions |
| `stages::analyze` / `analyze(` | rule tests that run the pipeline |
| `SemanticContextBuilder::new()` | a hand-built empty context |
| no `rule_ok!` and no `assert_rule(…, &[])`, or no `Problem::` reference | a rule missing its ok or its err side |

The parser crate gets the same test over `parser/src/rule_*.rs`, without the
context check.

It runs under `cargo test`, so `just`, `just test` and `just coverage` all
enforce it, on every platform, with no new CI step. Text matching is enough
here because the patterns are distinctive. If it proves brittle, parse the
files with `syn` (already in the dependency tree via `dsl_macro_derive`)
instead.

Alternatives considered:

- **A `just` recipe with `grep`**, like `specs/justfile`. It only runs when
  someone runs the recipe, and the `windows-shell := powershell` setting
  makes portable grep awkward.
- **`clippy.toml` `disallowed-macros`/`disallowed-methods`.** This can ban a
  macro by path, but not "`.is_err()` inside a rule's test module". Banning
  `Result::is_err` crate-wide is too broad.
- **A custom lint (`dylint`).** Precise, but a nightly toolchain and a new
  crate to maintain for five patterns.

The steering text moves to
[compiler-standards.md](../steering/compiler-standards.md) (Testing
Standards → "Rule tests") and replaces the paragraph in
[compiler-architecture.md](../steering/compiler-architecture.md) that sends
context-reading rules to the `rule_ctx_*` macros. The meta-test's failure
message links to it.

## Prefactoring

The helpers are the prefactor. Adding `assert_rule` and `assert_rule_at`,
and re-expressing the `rule_ctx_ok`, `rule_ctx_err1` and `rule_ctx_errn`
macros (and their `_with` forms) over `assert_rule`, changes no test's
behaviour: each of those macros already asserts an exact result, which is
what `assert_rule` asserts. Every test must pass unchanged. The weak and
empty-context macros are left alone until the core PRs remove them.

## Design doc reference

None. Testing convention only; the outcome lands in the steering files.

## File map

- `compiler/analyzer/src/test_helpers.rs`: add `Rule`, `assert_rule`,
  `assert_rule_at`; remove `resolve_fresh_with`
- `compiler/analyzer/src/test_macros.rs`: reduce to `rule_ok!`, `rule_err!`,
  `rule_err_at!`
- `compiler/analyzer/src/rule_test_conventions.rs` (new): the meta-test
- `compiler/analyzer/src/lib.rs`: register the meta-test module
- `compiler/parser/src/`: token-rule helper, the same three macros, and the
  meta-test
- Every `compiler/analyzer/src/rule_*` and `compiler/parser/src/rule_*` test
  module
- `specs/steering/compiler-standards.md`, `specs/steering/compiler-architecture.md`

## Tasks

### Prefactor PR: shared assertion helpers

- [ ] Add `Rule`, `assert_rule` and `assert_rule_at` to `test_helpers.rs`
- [ ] Re-express `rule_ctx_ok`, `rule_ctx_err1`, `rule_ctx_errn` and their
      `_with` forms over `assert_rule`
- [ ] `cd compiler && just` passes with no test edited

### Core PR 1: make every assertion exact, in the right context

These edits change what tests assert, so they come before the rename.

- [ ] Rewrite the 38 weak-macro uses as `rule_err1`/`rule_errn` (or the `ctx_`
      form) with the exact problems the rule reports:
      `rule_function_block_invocation`, `rule_var_decl_const_initialized`,
      `rule_decl_struct_element_unique_names`, `rule_enumeration_values_unique`,
      `rule_program_task_definition_exists`, `rule_task_names_unique`,
      `rule_var_decl_const_not_fb`, `rule_case_bit_string_label`,
      `rule_function_call_type_check`, `rule_function_call_declared`,
      `rule_use_declared_symbolic_var`,
      `rule_var_decl_global_const_requires_external_const`,
      `rule_loop_control_inside_loop`
- [ ] Move the empty-context tests of the two rules that read the context to
      the resolved context: `rule_var_decl_const_initialized` (8),
      `rule_pou_hierarchy` (1, via `assert_rule_at`)
- [ ] Delete `rule_err`, `rule_ctx_err`, `rule_err_code`, `rule_ctx_err_code`
      and their `_with` forms

### Core PR 2: collapse to three macros

Mechanical: after Core PR 1 only rules that ignore the context still use the
empty-context macros, so moving them to the resolved context changes no
result. One commit per old macro, each a search-and-replace.

- [ ] Add `rule_ok!`, `rule_err!` (with its list) and `rule_err_at!` over the
      helpers, each with an optional trailing options argument
- [ ] `rule_ok`, `rule_ok_with`, `rule_ctx_ok`, `rule_ctx_ok_with` (188 uses) → `rule_ok!`
- [ ] `rule_err1`, `rule_err1_with`, `rule_ctx_err1`, `rule_ctx_err1_with` (69, plus
      the Core PR 1 rewrites) → `rule_err!(…, [P])`
- [ ] `rule_errn`, `rule_errn_with`, `rule_ctx_errn`, `rule_ctx_errn_with` (19, plus
      the Core PR 1 rewrites) → `rule_err!(…, [P, P, …])`
- [ ] `rule_err1_at` (36) → `rule_err_at!`
- [ ] Delete the old macros and `resolve_fresh_with`

### Core PR 3: hand-written and `#[rstest]` bodies onto the helpers

- [ ] `rule_assignment_aggregate_type_compat`, `rule_unsupported_extension`,
      `rule_use_declared_symbolic_var`, `rule_function_block_call_unsupported`:
      string codes → `Problem` variants, exact lists
- [ ] `rule_member_qualifier_invalid`: assert codes; keep the message only
      where it is the feature
- [ ] `rule_ref_to/tests.rs`, `rule_bit_and_partial_access_range`: from the
      full pipeline to `apply` with exact codes (pipeline cases that need
      more than one rule move to `spec_conformance`)
- [ ] `rule_constant_range/tests.rs`, `rule_condition_type`,
      `rule_case_selector_type`, `rule_function_call_in_out_argument`,
      `rule_function_call_type_check` (+ `composite_tests.rs`),
      `rule_use_declared_enumerated_value`, `rule_no_top_level_var_global`,
      `rule_stdlib_type_redefinition`, `rule_string_encoding_compat`,
      `rule_string_literal_char_range`, `rule_var_decl_initializer_type_compat`,
      `rule_operator_operand_type_check`, `rule_struct_initializer_expression_allowed`,
      `rule_mixed_located_var_declarations`: drop the private scaffolds, call
      `assert_rule` / `assert_rule_at` or the macros
- [ ] Parser token rules (5 files): token helper, the three macros, exact codes

### Core PR 4: enforcement and steering

- [ ] Add `rule_test_conventions.rs` (analyzer) and its parser twin; each check
      first shown to fail on a seeded violation, then pass on the tree
- [ ] "Rule tests" section in `compiler-standards.md`; update `compiler-architecture.md`
