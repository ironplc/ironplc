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
    core::Located,
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{
    result::SemanticResult, symbol_environment::SymbolEnvironment,
    type_environment::TypeEnvironment,
};

pub fn apply(
    lib: &Library,
    _type_environment: &TypeEnvironment,
    _symbol_environment: &SymbolEnvironment,
) -> SemanticResult {
    let mut visitor = RuleConstantVarsInitialized {
        diagnostics: Vec::new(),
    };
    visitor.walk(lib).map_err(|e| vec![e])?;

    if !visitor.diagnostics.is_empty() {
        return Err(visitor.diagnostics);
    }
    Ok(())
}

struct RuleConstantVarsInitialized {
    diagnostics: Vec<Diagnostic>,
}

impl Visitor<Diagnostic> for RuleConstantVarsInitialized {
    type Value = ();

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), Diagnostic> {
        if node.var_type == VariableType::External {
            // If the variable type is external, than it must be initialized
            // somewhere else and therefore we do not need to check here.
            return node.recurse_visit(self);
        }

        match node.qualifier {
            DeclarationQualifier::Constant => match &node.initializer {
                InitialValueAssignmentKind::None(sp) => {
                    return Err(Diagnostic::todo_with_span(sp.clone(), file!(), line!()))
                }
                InitialValueAssignmentKind::Simple(si) => match si.initial_value {
                    Some(_) => {}
                    None => {
                        self.diagnostics.push(
                            Diagnostic::problem(
                                Problem::ConstantMustHaveInitializer,
                                Label::span(node.span(), "Variable"),
                            )
                            .with_context("variable", &node.identifier.to_string()),
                        );
                    }
                },
                InitialValueAssignmentKind::String(str) => match str.initial_value {
                    Some(_) => {}
                    None => {
                        self.diagnostics.push(
                            Diagnostic::problem(
                                Problem::ConstantMustHaveInitializer,
                                Label::span(node.span(), "Variable declaration"),
                            )
                            .with_context("variable", &node.identifier.to_string()),
                        );
                    }
                },
                InitialValueAssignmentKind::EnumeratedValues(spec) => match spec.initial_value {
                    Some(_) => {}
                    None => {
                        self.diagnostics.push(
                            Diagnostic::problem(
                                Problem::ConstantMustHaveInitializer,
                                Label::span(node.span(), "Variable declaration"),
                            )
                            .with_context("variable", &node.identifier.to_string()),
                        );
                    }
                },
                InitialValueAssignmentKind::EnumeratedType(type_init) => {
                    match type_init.initial_value {
                        Some(_) => {}
                        None => self.diagnostics.push(
                            Diagnostic::problem(
                                Problem::ConstantMustHaveInitializer,
                                Label::span(node.span(), "Variable declaration"),
                            )
                            .with_context("variable", &node.identifier.to_string()),
                        ),
                    }
                }
                InitialValueAssignmentKind::FunctionBlock(_) => {
                    return Err(Diagnostic::todo(file!(), line!()))
                }
                InitialValueAssignmentKind::Subrange(_) => {
                    return Err(Diagnostic::internal_error(file!(), line!()))
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

        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod test {
    use crate::test_helpers::parse_and_resolve_types;

    use super::*;

    #[test]
    fn apply_when_const_simple_type_missing_initializer_then_error() {
        let program = "
FUNCTION_BLOCK LOGGER
VAR CONSTANT
ResetCounterValue : INT;
END_VAR

END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

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

        let library = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

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

        let library = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

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

        let library = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

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

        let library = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

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

        let library = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

        assert!(result.is_ok())
    }
}
