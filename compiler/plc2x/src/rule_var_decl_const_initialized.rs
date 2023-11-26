//! Semantic rule that variables declared with the `CONSTANT`
//! qualifier class must have initial values.
//!
//! See section 2.4.3.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK LOGGER
//!    VAR CONSTANT
//!       ResetCounterValue : INT := 1;
//!    END_VAR
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION_BLOCK LOGGER
//!    VAR CONSTANT
//!       ResetCounterValue : INT;
//!    END_VAR
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Todo
//!
//! I don't know if it is possible to have an external
//! reference where one part declares the value and another
//! references the value (and still be constant).
use ironplc_dsl::{
    common::*,
    diagnostic::{Diagnostic, Label},
    visitor::{visit_var_decl, Visitor},
};
use ironplc_problems::Problem;

pub fn apply(lib: &Library) -> Result<(), Diagnostic> {
    let mut visitor = RuleConstantVarsInitialized {};
    visitor.walk(lib)
}

struct RuleConstantVarsInitialized {}

impl Visitor<Diagnostic> for RuleConstantVarsInitialized {
    type Value = ();

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), Diagnostic> {
        if node.var_type == VariableType::External {
            // If the variable type is external, than it must be initialized
            // somewhere else and therefore we do not need to check here.
            return visit_var_decl(self, node);
        }

        match node.qualifier {
            DeclarationQualifier::Constant => match &node.initializer {
                InitialValueAssignmentKind::None => return Err(Diagnostic::todo(file!(), line!())),
                InitialValueAssignmentKind::Simple(si) => match si.initial_value {
                    Some(_) => {}
                    None => {
                        return Err(Diagnostic::problem(
                            Problem::ConstantMustHaveInitializer,
                            Label::source_loc(&node.position, "Variable"),
                        )
                        .with_context("variable", &node.identifier.to_string()));
                    }
                },
                InitialValueAssignmentKind::String(str) => match str.initial_value {
                    Some(_) => {}
                    None => {
                        return Err(Diagnostic::problem(
                            Problem::ConstantMustHaveInitializer,
                            Label::source_loc(&node.position, "Variable declaration"),
                        )
                        .with_context("variable", &node.identifier.to_string()));
                    }
                },
                InitialValueAssignmentKind::EnumeratedValues(spec) => match spec.initial_value {
                    Some(_) => {}
                    None => {
                        return Err(Diagnostic::problem(
                            Problem::ConstantMustHaveInitializer,
                            Label::source_loc(&node.position, "Variable declaration"),
                        )
                        .with_context("variable", &node.identifier.to_string()));
                    }
                },
                InitialValueAssignmentKind::EnumeratedType(type_init) => {
                    match type_init.initial_value {
                        Some(_) => {}
                        None => {
                            return Err(Diagnostic::problem(
                                Problem::ConstantMustHaveInitializer,
                                Label::source_loc(&node.position, "Variable declaration"),
                            )
                            .with_context("variable", &node.identifier.to_string()))
                        }
                    }
                }
                InitialValueAssignmentKind::FunctionBlock(_) => {
                    return Err(Diagnostic::todo(file!(), line!()))
                }
                InitialValueAssignmentKind::Subrange(_) => {
                    return Err(Diagnostic::todo(file!(), line!()))
                }
                InitialValueAssignmentKind::Structure(_) => {
                    return Err(Diagnostic::todo(file!(), line!()))
                }
                InitialValueAssignmentKind::Array(_) => {
                    return Err(Diagnostic::todo(file!(), line!()))
                }
                InitialValueAssignmentKind::LateResolvedType(_) => {
                    return Err(Diagnostic::todo(file!(), line!()))
                }
            },
            // Do not care about the following qualifiers
            DeclarationQualifier::Unspecified => {}
            DeclarationQualifier::Retain => {}
            DeclarationQualifier::NonRetain => {}
        }

        visit_var_decl(self, node)
    }
}

#[cfg(test)]
mod test {
    use ironplc_dsl::core::FileId;

    use super::*;

    use crate::stages::parse;

    #[test]
    fn apply_when_const_simple_type_missing_initializer_then_error() {
        let program = "
FUNCTION_BLOCK LOGGER
VAR CONSTANT
ResetCounterValue : INT;
END_VAR

END_FUNCTION_BLOCK";

        let library = parse(program, &FileId::default()).unwrap();
        let result = apply(&library);

        assert!(result.is_err())
    }

    #[test]
    fn apply_when_const_enum_type_missing_initializer_then_error() {
        let program = "
TYPE
LOGLEVEL : (CRITICAL) := CRITICAL;
END_TYPE

FUNCTION_BLOCK LOGGER
VAR CONSTANT
ResetCounterValue : LOGLEVEL;
END_VAR

END_FUNCTION_BLOCK";

        let library = parse(program, &FileId::default()).unwrap();
        let result = apply(&library);

        assert!(result.is_err())
    }

    #[test]
    fn apply_when_const_enum_values_type_missing_initializer_then_error() {
        let program = "
FUNCTION_BLOCK LOGGER
VAR CONSTANT
ResetCounterValue : (INFO, WARN);
END_VAR

END_FUNCTION_BLOCK";

        let library = parse(program, &FileId::default()).unwrap();
        let result = apply(&library);

        assert!(result.is_err())
    }

    #[test]
    fn apply_when_const_enum_values_type_has_initializer_then_ok() {
        let program = "
FUNCTION_BLOCK LOGGER
VAR CONSTANT
ResetCounterValue : (INFO, WARN) := INFO;
END_VAR

END_FUNCTION_BLOCK";

        let library = parse(program, &FileId::default()).unwrap();
        let result = apply(&library);

        assert!(result.is_ok())
    }

    #[test]
    fn apply_when_const_simple_external_type_missing_initializer_then_ok() {
        let program = "
TYPE
LOGLEVEL : (CRITICAL) := CRITICAL;
END_TYPE

FUNCTION_BLOCK LOGGER
VAR_EXTERNAL CONSTANT
ResetCounterValue : LOGLEVEL;
END_VAR

END_FUNCTION_BLOCK";

        let library = parse(program, &FileId::default()).unwrap();
        let result = apply(&library);

        assert!(result.is_ok())
    }

    #[test]
    fn apply_when_const_simple_has_initializer_then_ok() {
        let program = "
FUNCTION_BLOCK LOGGER
VAR CONSTANT
ResetCounterValue : INT := 1;
END_VAR

END_FUNCTION_BLOCK";

        let library = parse(program, &FileId::default()).unwrap();
        let result = apply(&library);

        assert!(result.is_ok())
    }
}
