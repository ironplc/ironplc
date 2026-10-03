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

1. **Unit under test.** `super::apply` on a library resolved by
   `parse_and_resolve_types_with_options`. Use the fresh (empty) context only
   when the rule's `apply` ignores `_context`. Pipeline behaviour belongs in
   `spec_conformance_*.rs` or the `plc2x` tests.
2. **Assertion.** The ordered list of diagnostic codes equals an expected
   `&[Problem]`. An empty list means `Ok`. This one assertion covers "ok",
   "exactly one", "exactly N" and "mixed codes", and it catches duplicate,
   missing and wrong-code reports.
3. **Codes are `Problem` variants**, never `"P####"` literals.
4. **Location.** A rule whose diagnostic names a construct has at least one
   `_at` test pinning the primary label to the offending text.
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

One function does the work; the macros become thin wrappers around it.

```rust
// test_helpers.rs
pub type Rule = fn(&Library, &SemanticContext, &CompilerOptions) -> SemanticResult;

/// Resolves `program`, runs `rule`, and asserts it reports exactly `expected`, in order.
pub fn assert_rule(rule: Rule, program: &str, opts: &CompilerOptions, expected: &[Problem]) { … }
/// As `assert_rule`, with an empty context, for a rule that ignores it.
pub fn assert_rule_fresh(rule: Rule, program: &str, opts: &CompilerOptions, expected: &[Problem]) { … }
/// As `assert_rule` with one expected problem whose label points at the first `at` in `program`.
pub fn assert_rule_at(rule: Rule, program: &str, opts: &CompilerOptions, expected: Problem, at: &str) { … }
```

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
rule_ctx_err1!(
    apply_when_array_extents_differ_then_reports_mismatch,
    &program_with("a : ARRAY[1..3] OF INT; …", "a := b;"),
    Problem::AggregateAssignmentTypeMismatch
);
```

Or, as an `#[rstest]` case:

```rust
#[rstest]
#[case::extents("a : ARRAY[1..3] OF INT; b : ARRAY[1..4] OF INT;")]
#[case::element_type("a : ARRAY[1..3] OF INT; b : ARRAY[1..3] OF REAL;")]
fn apply_when_array_types_differ_then_reports_mismatch(#[case] decls: &str) {
    assert_rule(apply, &program_with(decls, "a := b;"), &opts(),
        &[Problem::AggregateAssignmentTypeMismatch]);
}
```

### Macro set after the change

Keep: `rule_ok`, `rule_err1`, `rule_errn`, `rule_err1_at` and their `ctx_` /
`_with` forms, re-expressed over `assert_rule*`. Add `rule_errs!(name,
program, [Problem::A, Problem::B])` for mixed codes, and `rule_ctx_err1_at!`
(missing today, which is why `rule_pou_hierarchy` used the fresh form).

Delete: `rule_err`, `rule_ctx_err`, `rule_err_code`, `rule_ctx_err_code` and
their `_with` forms (38 uses). Each assertion they make is weaker than the
`err1`/`errn` form that replaces it.

The parser token rules take `(&[Token], &CompilerOptions)`, so they get the
same three-function helper over tokens in `parser/src/`; the standard is the
same.

## Enforcement

A build check is possible, in two layers.

**1. Compile time: remove the weak macros.** Once the 38 uses are migrated,
deleting the six weak macros makes the "any error" one-liners impossible to
write. No new tooling.

**2. Test time: a conventions meta-test.** This follows the precedent of
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
| a fresh-context form (`rule_ok!`, `rule_err1!`, `assert_rule_fresh`, …) in a file whose `apply` takes `context:` rather than `_context:` | rules tested against an empty context |
| no `rule_ok`/`rule_ctx_ok`/empty `assert_rule` call, or no `Problem::` reference | a rule missing its ok or its err side |

The parser crate gets the same test over `parser/src/rule_*.rs`.

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
  crate to maintain for six patterns.

The steering text moves to
[compiler-standards.md](../steering/compiler-standards.md) (Testing
Standards → "Rule tests") and replaces the paragraph in
[compiler-architecture.md](../steering/compiler-architecture.md). The meta-test's
failure message links to it.

## Prefactoring

The helper functions are the prefactor. Adding `assert_rule*` and
re-expressing the existing macros over them changes no test's behaviour, and
every test must pass unchanged. That PR lands before any test is migrated.

## Design doc reference

None. Testing convention only; the outcome lands in the steering files.

## File map

- `compiler/analyzer/src/test_helpers.rs`: add `Rule`, `assert_rule`,
  `assert_rule_fresh`, `assert_rule_at`
- `compiler/analyzer/src/test_macros.rs`: re-express over the helpers; add
  `rule_errs!`, `rule_ctx_err1_at!`; delete the weak macros
- `compiler/analyzer/src/rule_test_conventions.rs` (new): the meta-test
- `compiler/analyzer/src/lib.rs`: register the meta-test module
- `compiler/parser/src/test_helpers` (or the existing parser test support):
  token-rule helpers and meta-test
- The rule files listed under Tasks
- `specs/steering/compiler-standards.md`, `specs/steering/compiler-architecture.md`

## Tasks

### Prefactor PR: shared assertion helpers

- [ ] Add `assert_rule`, `assert_rule_fresh`, `assert_rule_at` to `test_helpers.rs`
- [ ] Re-express every `test_macros.rs` macro over them, keeping each macro's
      current semantics (the weak ones too, for now)
- [ ] Add `rule_errs!` and `rule_ctx_err1_at!`
- [ ] `cd compiler && just` passes with no test edited

### Core PR 1: replace the weak macros (38 tests)

- [ ] `rule_err!` → `rule_err1!`/`rule_errn!`: `rule_function_block_invocation`,
      `rule_var_decl_const_initialized`, `rule_decl_struct_element_unique_names`,
      `rule_enumeration_values_unique`, `rule_program_task_definition_exists`,
      `rule_task_names_unique`, `rule_var_decl_const_not_fb`, `rule_case_bit_string_label`
- [ ] `rule_ctx_err!` / `rule_ctx_err_code!` → `ctx_err1`/`ctx_errn`:
      `rule_function_call_type_check`, `rule_function_call_declared`,
      `rule_use_declared_symbolic_var`, `rule_var_decl_global_const_requires_external_const`
- [ ] `rule_err_code*!` → `rule_err1*!`: `rule_loop_control_inside_loop`
- [ ] Delete the six weak macros and their `_with` forms

### Core PR 2: right context, codes as `Problem`

- [ ] `rule_var_decl_const_initialized`, `rule_pou_hierarchy`: move the fresh-context tests to `rule_ctx_*`
- [ ] `rule_assignment_aggregate_type_compat`, `rule_unsupported_extension`,
      `rule_use_declared_symbolic_var`, `rule_function_block_call_unsupported`:
      string codes → `Problem` variants, exact-list assertions
- [ ] `rule_member_qualifier_invalid`: assert codes; keep the message only where it is the feature

### Core PR 3: hand-written and `#[rstest]` bodies onto `assert_rule*`

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
      `rule_mixed_located_var_declarations`: drop the private scaffolds, call `assert_rule*`
- [ ] Parser token rules (5 files): token helper plus exact-code assertions

### Core PR 4: enforcement and steering

- [ ] Add `rule_test_conventions.rs` (analyzer) and its parser twin; each check
      first shown to fail on a seeded violation, then pass on the tree
- [ ] "Rule tests" section in `compiler-standards.md`; update `compiler-architecture.md`
