//! Semantic rule that variables declared with the CONSTANT
//! qualifier class must have initial values.
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
    dsl::*,
    visitor::{visit_variable_declaration, Visitor},
};

use crate::error::SemanticDiagnostic;

pub fn apply(lib: &Library) -> Result<(), SemanticDiagnostic> {
    let mut visitor = RuleConstantVarsInitialized {};
    visitor.walk(lib)
}

struct RuleConstantVarsInitialized {}

impl Visitor<SemanticDiagnostic> for RuleConstantVarsInitialized {
    type Value = ();

    fn visit_variable_declaration(&mut self, node: &VarDecl) -> Result<(), SemanticDiagnostic> {
        if node.var_type == VariableType::External {
            // If the variable type is external, than it must be initialized
            // somewhere else and therefore we do not need to check here.
            return visit_variable_declaration(self, node);
        }

        match node.qualifier {
            StorageQualifier::Constant => match &node.initializer {
                TypeInitializer::None => todo!(),
                TypeInitializer::Simple {
                    type_name: _,
                    initial_value,
                } => match initial_value {
                    Some(_) => {}
                    None => {
                        return Err(SemanticDiagnostic::error(
                            "S0001",
                            format!(
                                "Variable is constant but does not define value {} ",
                                node.name
                            ),
                        )
                        .with_location(&node.position));
                    }
                },
                TypeInitializer::EnumeratedValues(spec) => match spec.initial_value {
                    Some(_) => {}
                    None => {
                        return Err(SemanticDiagnostic::error(
                            "S0002",
                            format!(
                                "Variable is constant but does not define value {} ",
                                node.name
                            ),
                        )
                        .with_location(&node.position));
                    }
                },
                TypeInitializer::EnumeratedType(type_init) => match type_init.initial_value {
                    Some(_) => {}
                    None => {
                        return Err(SemanticDiagnostic::error(
                            "S0003",
                            format!(
                                "Variable is constant but does not define value {} ",
                                node.name
                            ),
                        )
                        .with_location(&node.position))
                    }
                },
                TypeInitializer::FunctionBlock(_) => todo!(),
                TypeInitializer::Structure { type_name: _ } => todo!(),
                TypeInitializer::LateResolvedType(_) => todo!(),
            },
            StorageQualifier::Unspecified => {}
            StorageQualifier::Retain => {}
            StorageQualifier::NonRetain => {}
        }

        visit_variable_declaration(self, node)
    }
}

#[cfg(test)]
mod test {
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

        let library = parse(program).unwrap();
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

        let library = parse(program).unwrap();
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

        let library = parse(program).unwrap();
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

        let library = parse(program).unwrap();
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

        let library = parse(program).unwrap();
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

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert!(result.is_ok())
    }
}
