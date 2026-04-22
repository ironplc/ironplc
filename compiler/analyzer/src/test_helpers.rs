use crate::result::SemanticResult;
use crate::semantic_context::SemanticContext;
use crate::stages::resolve_types;
use ironplc_dsl::common::*;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;

#[cfg(test)]
pub fn parse_only(program: &str) -> Library {
    use ironplc_parser::{options::CompilerOptions, parse_program};

    parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap()
}

#[cfg(test)]
pub fn parse_and_resolve_types(program: &str) -> Library {
    use ironplc_parser::{options::CompilerOptions, parse_program};

    let library = parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();
    let (library, _context) = resolve_types(&[&library], &CompilerOptions::default()).unwrap();
    library
}

/// Parses a program and resolves types, returning both the library and semantic context.
/// Use this when testing rules that need access to the type environment or other context.
#[cfg(test)]
pub fn parse_and_resolve_types_with_context(program: &str) -> (Library, SemanticContext) {
    use ironplc_parser::{options::CompilerOptions, parse_program};

    let library = parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();
    resolve_types(&[&library], &CompilerOptions::default()).unwrap()
}

/// Parses a program with custom options and resolves types, returning both library and context.
/// Use this when testing dialect-specific behavior.
#[cfg(test)]
pub fn parse_and_resolve_types_with_options(
    program: &str,
    options: &ironplc_parser::options::CompilerOptions,
) -> (Library, SemanticContext) {
    use ironplc_parser::parse_program;

    let library = parse_program(program, &FileId::default(), options).unwrap();
    resolve_types(&[&library], options).unwrap()
}

/// Type alias for a semantic rule's `apply` function. Every rule in
/// `analyzer/src/rule_*.rs` exposes an `apply` with this signature, which lets
/// the `assert_rule_*` helpers work uniformly across rules.
#[cfg(test)]
pub type RuleApplyFn = fn(&Library, &SemanticContext, &CompilerOptions) -> SemanticResult;

/// Parses `program`, resolves types, applies `rule`, and asserts the result is `Ok`.
///
/// Collapses the 4-line scaffold that every `rule_*.rs` test used to write
/// (parse → resolve → apply → `assert!(result.is_ok())`) into one call, which
/// removes a large dupe cluster that `cargo dupes` used to flag across the
/// analyzer rule tests.
#[cfg(test)]
pub fn assert_rule_ok(rule: RuleApplyFn, program: &str) {
    let (library, context) = parse_and_resolve_types_with_context(program);
    let result = rule(&library, &context, &CompilerOptions::default());
    assert!(result.is_ok(), "expected Ok, got {:?}", result.err());
}

/// Like [`assert_rule_ok`] but asserts the rule produces exactly one diagnostic
/// with the given problem code. `expected_code` is typically
/// `Problem::FooBar.code()`.
#[cfg(test)]
pub fn assert_rule_err(rule: RuleApplyFn, program: &str, expected_code: &str) {
    let (library, context) = parse_and_resolve_types_with_context(program);
    let result = rule(&library, &context, &CompilerOptions::default());
    let diagnostics = result.expect_err("expected Err");
    assert_eq!(
        diagnostics.len(),
        1,
        "expected exactly 1 diagnostic, got {diagnostics:?}"
    );
    assert_eq!(diagnostics[0].code, expected_code);
}

/// Same shape as [`assert_rule_ok`] but passes a blank `SemanticContext`
/// (built by `SemanticContextBuilder::new()`) instead of one produced by the
/// analyzer pipeline. Use this for rules whose `apply` takes `_context` (i.e.,
/// they don't consult the type environment), since the blank context avoids
/// forcing the program through full type resolution.
#[cfg(test)]
pub fn assert_rule_ok_blank_ctx(rule: RuleApplyFn, program: &str) {
    let library = parse_and_resolve_types(program);
    let context = crate::semantic_context::SemanticContextBuilder::new()
        .build()
        .unwrap();
    let result = rule(&library, &context, &CompilerOptions::default());
    assert!(result.is_ok(), "expected Ok, got {:?}", result.err());
}

/// Like [`assert_rule_ok_blank_ctx`] but asserts the rule returns `Err`.
/// Does not check the diagnostic code — use where the test just confirms the
/// rule fired at all.
#[cfg(test)]
pub fn assert_rule_err_blank_ctx(rule: RuleApplyFn, program: &str) {
    let library = parse_and_resolve_types(program);
    let context = crate::semantic_context::SemanticContextBuilder::new()
        .build()
        .unwrap();
    let result = rule(&library, &context, &CompilerOptions::default());
    assert!(result.is_err(), "expected Err, got {:?}", result.ok());
}

/// Like [`assert_rule_err_blank_ctx`] but uses [`parse_only`] instead of
/// [`parse_and_resolve_types`]. Useful for rules that check program-level
/// structure (duplicate POU names, etc.) whose tests use programs that don't
/// survive full type resolution — the check should fire on the raw library.
#[cfg(test)]
pub fn assert_rule_err_parse_only(rule: RuleApplyFn, program: &str) {
    let library = parse_only(program);
    let context = crate::semantic_context::SemanticContextBuilder::new()
        .build()
        .unwrap();
    let result = rule(&library, &context, &CompilerOptions::default());
    assert!(result.is_err(), "expected Err, got {:?}", result.ok());
}
