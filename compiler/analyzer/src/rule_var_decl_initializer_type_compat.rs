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
    time::TemporalWidth,
    visitor::Visitor,
};
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    intermediate_type::{ByteSized, IntermediateType},
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    type_environment::TypeEnvironment,
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleInitializerTypeCompat {
            type_environment: context.types(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleInitializerTypeCompat<'a> {
    type_environment: &'a TypeEnvironment,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleInitializerTypeCompat<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

/// Checks whether a constant literal is type-compatible with the target type.
/// Whether a temporal literal of `width` can initialize storage of `size`.
///
/// The 32-bit member widens into the 64-bit one, holding the same unit in more
/// bits, so it fits either. The 64-bit member fits only 64-bit storage.
fn fits_width(width: TemporalWidth, size: &ByteSized) -> bool {
    match width {
        TemporalWidth::Short => true,
        TemporalWidth::Long => matches!(size, ByteSized::B64),
    }
}

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
        // A temporal literal names its own member of the family, so the size
        // is compared as well as the kind: `TIME#` initializes an `LTIME`
        // because the short member widens, while `LTIME#` does not initialize
        // a `TIME` -- the long member exists to hold what the short one
        // cannot.
        IntermediateType::Time { size } => {
            matches!(constant, ConstantKind::Duration(lit) if fits_width(lit.width, size))
        }
        IntermediateType::Date { size } => {
            matches!(constant, ConstantKind::Date(lit) if fits_width(lit.width, size))
        }
        IntermediateType::TimeOfDay { size } => {
            matches!(constant, ConstantKind::TimeOfDay(lit) if fits_width(lit.width, size))
        }
        IntermediateType::DateAndTime { size } => {
            matches!(constant, ConstantKind::DateAndTime(lit) if fits_width(lit.width, size))
        }
        IntermediateType::Subrange { base_type, .. } => is_compatible(constant, base_type),
        // Complex types (Enumeration, Structure, Array, FunctionBlock, Function)
        // use different InitialValueAssignmentKind variants, not Simple.
        _ => true,
    }
}

impl Visitor<Infallible> for RuleInitializerTypeCompat<'_> {
    type Value = ();

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), Infallible> {
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
    use crate::test_helpers::parse_and_resolve_types_with_options;

    use super::*;
    use ironplc_parser::options::{CompilerOptions, Dialect};
    use ironplc_problems::Problem;

    rule_ctx_ok!(
        apply_when_int_var_with_integer_literal_then_ok,
        "
PROGRAM main
VAR
    x : INT := 10;
END_VAR
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_real_var_with_real_literal_then_ok,
        "
PROGRAM main
VAR
    x : REAL := 10.0;
END_VAR
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_real_var_with_integer_literal_then_ok,
        "
PROGRAM main
VAR
    x : REAL := 10;
END_VAR
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_bool_var_with_boolean_literal_then_ok,
        "
PROGRAM main
VAR
    x : BOOL := TRUE;
END_VAR
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_no_initializer_then_ok,
        "
PROGRAM main
VAR
    x : INT;
END_VAR
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_uint_var_with_integer_literal_then_ok,
        "
PROGRAM main
VAR
    x : UINT := 5;
END_VAR
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_int_var_with_negative_integer_literal_then_ok,
        "
PROGRAM main
VAR
    x : INT := -10;
END_VAR
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_real_var_with_negative_real_literal_then_ok,
        "
PROGRAM main
VAR
    x : REAL := -10.0;
END_VAR
END_PROGRAM"
    );

    rule_ctx_err1!(
        apply_when_int_var_with_real_literal_then_error,
        "
PROGRAM main
VAR
    dummy : INT := 10.0;
END_VAR
END_PROGRAM",
        Problem::InitializerTypeMismatch
    );

    rule_ctx_err1!(
        apply_when_bool_var_with_integer_literal_then_error,
        "
PROGRAM main
VAR
    x : BOOL := 1;
END_VAR
END_PROGRAM",
        Problem::InitializerTypeMismatch
    );

    rule_ctx_err1!(
        apply_when_real_var_with_boolean_literal_then_error,
        "
PROGRAM main
VAR
    x : REAL := TRUE;
END_VAR
END_PROGRAM",
        Problem::InitializerTypeMismatch
    );

    rule_ctx_err1!(
        apply_when_int_var_with_string_literal_then_error,
        "
PROGRAM main
VAR
    x : INT := 'hello';
END_VAR
END_PROGRAM",
        Problem::InitializerTypeMismatch
    );

    #[test]
    fn apply_when_bool_var_with_integer_one_and_rusty_dialect_then_ok() {
        let program = "
PROGRAM main
VAR
    x : BOOL := 1;
END_VAR
END_PROGRAM";

        let options = CompilerOptions::from_dialect(Dialect::Rusty);
        let (library, context) = parse_and_resolve_types_with_options(program, &options);
        let result = apply(&library, &context, &options);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_bool_var_with_integer_zero_and_rusty_dialect_then_ok() {
        let program = "
PROGRAM main
VAR
    x : BOOL := 0;
END_VAR
END_PROGRAM";

        let options = CompilerOptions::from_dialect(Dialect::Rusty);
        let (library, context) = parse_and_resolve_types_with_options(program, &options);
        let result = apply(&library, &context, &options);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_bool_var_with_integer_two_and_rusty_dialect_then_error() {
        let program = "
PROGRAM main
VAR
    x : BOOL := 2;
END_VAR
END_PROGRAM";

        let options = CompilerOptions::from_dialect(Dialect::Rusty);
        let (library, context) = parse_and_resolve_types_with_options(program, &options);
        let result = apply(&library, &context, &options);
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert_eq!(1, errors.len());
        assert_eq!(Problem::InitializerTypeMismatch.code(), errors[0].code);
    }
}
