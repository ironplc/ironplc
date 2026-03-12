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

pub fn apply(lib: &Library, context: &SemanticContext) -> SemanticResult {
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
///
/// Note: `IntermediateType::Date` represents DATE, TIME_OF_DAY, and
/// DATE_AND_TIME because the intermediate type system does not distinguish
/// them. We accept all three corresponding constant kinds for `Date` to
/// avoid false positives.
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
        IntermediateType::Date => {
            matches!(
                constant,
                ConstantKind::Date(_) | ConstantKind::TimeOfDay(_) | ConstantKind::DateAndTime(_)
            )
        }
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
    use crate::test_helpers::parse_and_resolve_types_with_context;

    use super::*;
    use ironplc_problems::Problem;

    #[test]
    fn apply_when_int_var_with_integer_literal_then_ok() {
        let program = "
PROGRAM main
VAR
    x : INT := 10;
END_VAR
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_real_var_with_real_literal_then_ok() {
        let program = "
PROGRAM main
VAR
    x : REAL := 10.0;
END_VAR
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_real_var_with_integer_literal_then_ok() {
        let program = "
PROGRAM main
VAR
    x : REAL := 10;
END_VAR
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_bool_var_with_boolean_literal_then_ok() {
        let program = "
PROGRAM main
VAR
    x : BOOL := TRUE;
END_VAR
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_no_initializer_then_ok() {
        let program = "
PROGRAM main
VAR
    x : INT;
END_VAR
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_uint_var_with_integer_literal_then_ok() {
        let program = "
PROGRAM main
VAR
    x : UINT := 5;
END_VAR
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_int_var_with_negative_integer_literal_then_ok() {
        let program = "
PROGRAM main
VAR
    x : INT := -10;
END_VAR
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_real_var_with_negative_real_literal_then_ok() {
        let program = "
PROGRAM main
VAR
    x : REAL := -10.0;
END_VAR
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_int_var_with_real_literal_then_error() {
        let program = "
PROGRAM main
VAR
    dummy : INT := 10.0;
END_VAR
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert_eq!(1, errors.len());
        assert_eq!(Problem::InitializerTypeMismatch.code(), errors[0].code);
    }

    #[test]
    fn apply_when_bool_var_with_integer_literal_then_error() {
        let program = "
PROGRAM main
VAR
    x : BOOL := 1;
END_VAR
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert_eq!(1, errors.len());
        assert_eq!(Problem::InitializerTypeMismatch.code(), errors[0].code);
    }

    #[test]
    fn apply_when_real_var_with_boolean_literal_then_error() {
        let program = "
PROGRAM main
VAR
    x : REAL := TRUE;
END_VAR
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert_eq!(1, errors.len());
        assert_eq!(Problem::InitializerTypeMismatch.code(), errors[0].code);
    }

    #[test]
    fn apply_when_int_var_with_string_literal_then_error() {
        let program = "
PROGRAM main
VAR
    x : INT := 'hello';
END_VAR
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert_eq!(1, errors.len());
        assert_eq!(Problem::InitializerTypeMismatch.code(), errors[0].code);
    }
}
