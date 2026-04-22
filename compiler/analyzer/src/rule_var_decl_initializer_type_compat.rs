//! Semantic rule that variable initializers must be type-compatible
//! with the declared variable type.
//!
//! See section 2.4.3.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//!    VAR
//!       counter : INT := 10;
//!    END_VAR
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//!    VAR
//!       counter : INT := 10.0;
//!    END_VAR
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{
    intermediate_type::IntermediateType, result::SemanticResult, semantic_context::SemanticContext,
    type_environment::TypeEnvironment,
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    let mut visitor = RuleInitializerTypeCompat {
        type_environment: context.types(),
        diagnostics: Vec::new(),
    };
    visitor.walk(lib).map_err(|e| vec![e])?;

    if !visitor.diagnostics.is_empty() {
        return Err(visitor.diagnostics);
    }
    Ok(())
}

struct RuleInitializerTypeCompat<'a> {
    type_environment: &'a TypeEnvironment,
    diagnostics: Vec<Diagnostic>,
}

/// Checks whether a constant literal is type-compatible with the target type.
fn is_compatible(constant: &ConstantKind, target: &IntermediateType) -> bool {
    match target {
        IntermediateType::Bool => matches!(constant, ConstantKind::Boolean(_)),
        IntermediateType::Int { .. } | IntermediateType::UInt { .. } => {
            matches!(
                constant,
                ConstantKind::IntegerLiteral(_) | ConstantKind::BitStringLiteral(_)
            )
        }
        IntermediateType::Real { .. } => {
            matches!(
                constant,
                ConstantKind::RealLiteral(_) | ConstantKind::IntegerLiteral(_)
            )
        }
        IntermediateType::Bytes { .. } => {
            matches!(
                constant,
                ConstantKind::IntegerLiteral(_) | ConstantKind::BitStringLiteral(_)
            )
        }
        IntermediateType::String { .. } => matches!(constant, ConstantKind::CharacterString(_)),
        IntermediateType::Time { .. } => matches!(constant, ConstantKind::Duration(_)),
        IntermediateType::Date { .. } => matches!(constant, ConstantKind::Date(_)),
        IntermediateType::TimeOfDay { .. } => matches!(constant, ConstantKind::TimeOfDay(_)),
        IntermediateType::DateAndTime { .. } => matches!(constant, ConstantKind::DateAndTime(_)),
        IntermediateType::Subrange { base_type, .. } => is_compatible(constant, base_type),
        // Complex types (Enumeration, Structure, Array, FunctionBlock, Function)
        // use different InitialValueAssignmentKind variants, not Simple.
        _ => true,
    }
}

impl Visitor<Diagnostic> for RuleInitializerTypeCompat<'_> {
    type Value = ();

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), Diagnostic> {
        // TODO: extend type compatibility checking to other InitialValueAssignmentKind
        // variants. Currently only Simple (literal constant) initializers are validated.
        // Other variants that could benefit from checking:
        // - String: validate string initializer against declared type
        // - EnumeratedValues: validate inline enumeration initializer
        // - EnumeratedType: validate named enumeration initializer
        // - Subrange: validate subrange initializer value is within bounds
        // - Structure: validate structure field initializer types
        // - Array: validate array element initializer types
        if let InitialValueAssignmentKind::Simple(si) = &node.initializer {
            if let Some(constant) = &si.initial_value {
                if let Some(type_attrs) = self.type_environment.get(&si.type_name) {
                    if !is_compatible(constant, &type_attrs.representation) {
                        self.diagnostics.push(
                            Diagnostic::problem(
                                Problem::InitializerTypeMismatch,
                                Label::span(node.span(), "Variable declaration"),
                            )
                            .with_context("variable", &node.identifier.to_string()),
                        );
                    }
                }
            }
        }

        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_helpers::{
        assert_rule_err, assert_rule_ok, parse_and_resolve_types_with_options,
    };
    use ironplc_parser::options::{CompilerOptions, Dialect};
    use rstest::rstest;

    fn program_with_var(decl: &str) -> String {
        format!("PROGRAM main VAR {decl} END_VAR END_PROGRAM")
    }

    // All cases use the envelope `PROGRAM main VAR <decl> END_VAR END_PROGRAM`.
    // The default (IEC) dialect requires strict type-literal match.
    #[rstest]
    #[case::int_integer_literal("x : INT := 10;")]
    #[case::real_real_literal("x : REAL := 10.0;")]
    #[case::real_integer_literal("x : REAL := 10;")]
    #[case::bool_true("x : BOOL := TRUE;")]
    #[case::no_initializer("x : INT;")]
    #[case::uint_integer_literal("x : UINT := 5;")]
    #[case::int_negative_integer("x : INT := -10;")]
    #[case::real_negative_real("x : REAL := -10.0;")]
    fn apply_when_compatible_initializer_then_ok(#[case] decl: &str) {
        assert_rule_ok(apply, &program_with_var(decl));
    }

    // Same envelope, each case should produce exactly one
    // `InitializerTypeMismatch` diagnostic.
    #[rstest]
    #[case::int_real_literal("dummy : INT := 10.0;")]
    #[case::bool_integer_literal("x : BOOL := 1;")]
    #[case::real_boolean_literal("x : REAL := TRUE;")]
    #[case::int_string_literal("x : INT := 'hello';")]
    fn apply_when_incompatible_initializer_then_error(#[case] decl: &str) {
        assert_rule_err(
            apply,
            &program_with_var(decl),
            Problem::InitializerTypeMismatch.code(),
        );
    }

    // Rusty dialect relaxes BOOL := 0/1 (truthy integers) but still rejects
    // values outside {0, 1}. These cases can't use `assert_rule_ok/err`
    // because those helpers only pass `CompilerOptions::default()` to the
    // parser, and the dialect flag must reach both parse and apply stages.
    fn run_rusty_rule(decl: &str) -> SemanticResult {
        let options = CompilerOptions::from_dialect(Dialect::Rusty);
        let (library, context) = parse_and_resolve_types_with_options(&program_with_var(decl), &options);
        apply(&library, &context, &options)
    }

    #[rstest]
    #[case::bool_one("x : BOOL := 1;")]
    #[case::bool_zero("x : BOOL := 0;")]
    fn apply_when_rusty_dialect_bool_with_0_or_1_then_ok(#[case] decl: &str) {
        assert!(run_rusty_rule(decl).is_ok());
    }

    #[test]
    fn apply_when_bool_var_with_integer_two_and_rusty_dialect_then_error() {
        let result = run_rusty_rule("x : BOOL := 2;");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(1, errors.len());
        assert_eq!(Problem::InitializerTypeMismatch.code(), errors[0].code);
    }
}
