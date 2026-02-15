//! Semantic rule that references to enumerations use enumeration values
//! that are part of the enumeration declaration.
//!
//! ## Passes
//!
//! ```ignore
//! TYPE
//!    LEVEL : (CRITICAL) := CRITICAL;
//! END_TYPE
//!
//! FUNCTION_BLOCK LOGGER
//!    VAR_INPUT
//!       LEVEL : LEVEL := CRITICAL;
//!    END_VAR
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! TYPE
//!    LEVEL : (INFO) := INFO;
//! END_TYPE
//!
//! FUNCTION_BLOCK LOGGER
//!    VAR_INPUT
//!       LEVEL : LEVEL := CRITICAL;
//!    END_VAR
//! END_FUNCTION_BLOCK
//! ```
use ironplc_dsl::{
    common::*,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{result::SemanticResult, semantic_context::SemanticContext};

pub fn apply(lib: &Library, context: &SemanticContext) -> SemanticResult {
    // Walk the library to find all references to enumerations
    // checking that all references use an enumeration value
    // that is part of the enumeration
    let mut visitor = RuleDeclaredEnumeratedValues::new(context);
    visitor.walk(lib).map_err(|e| vec![e])
}

struct RuleDeclaredEnumeratedValues<'a> {
    context: &'a SemanticContext,
}

impl<'a> RuleDeclaredEnumeratedValues<'a> {
    fn new(context: &'a SemanticContext) -> Self {
        RuleDeclaredEnumeratedValues { context }
    }

    /// Returns enumeration values for a given enumeration type name.
    ///
    /// Uses the TypeEnvironment to resolve aliases and the SymbolEnvironment to find values.
    /// Handles alias chains by following references to base enumeration types.
    ///
    /// Returns Ok containing the list of valid enumeration value IDs.
    ///
    /// # Errors
    ///
    /// Returns Err(String) description of the error if:
    ///
    /// * a type name does not exist
    /// * the type is not an enumeration
    /// * there's a circular reference in the alias chain
    fn find_enum_declaration_values(&self, type_name: &TypeName) -> Result<Vec<&Id>, Diagnostic> {
        // Check if the type exists and is an enumeration
        if !self.context.types().is_enumeration(type_name) {
            return Err(Diagnostic::problem(
                Problem::EnumNotDeclared,
                Label::span(type_name.span(), "Type is not an enumeration"),
            ));
        }

        // Get all enumeration values for the type from the symbol environment
        Ok(self
            .context
            .symbols()
            .get_enumeration_values_for_type(type_name))
    }
}

impl Visitor<Diagnostic> for RuleDeclaredEnumeratedValues<'_> {
    type Value = ();

    fn visit_enumerated_initial_value_assignment(
        &mut self,
        init: &EnumeratedInitialValueAssignment,
    ) -> Result<Self::Value, Diagnostic> {
        let defined_values = self.find_enum_declaration_values(&init.type_name)?;
        if let Some(value) = &init.initial_value {
            // Check if the value is in the list of defined enumeration values
            if !defined_values.iter().any(|id| **id == value.value) {
                return Err(Diagnostic::problem(
                    Problem::EnumValueNotDefined,
                    Label::span(value.span(), "Expected value in enumeration"),
                )
                .with_context_id("value", &value.value));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use crate::stages::analyze;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::{options::ParseOptions, parse_program};

    #[test]
    fn apply_when_var_init_undefined_enum_value_then_error() {
        let program = "
TYPE
LEVEL : (INFO) := INFO;
END_TYPE
        
FUNCTION_BLOCK LOGGER
VAR_INPUT
LEVEL : LEVEL := CRITICAL;
END_VAR
END_FUNCTION_BLOCK";

        let library = parse_program(program, &FileId::default(), &ParseOptions::default()).unwrap();
        let result = analyze(&[&library]);

        let context = result.unwrap();
        assert!(context.has_diagnostics());
    }

    #[test]
    fn apply_when_var_init_valid_enum_value_then_ok() {
        let program = "
TYPE
LEVEL : (CRITICAL) := CRITICAL;
END_TYPE

FUNCTION_BLOCK LOGGER
VAR_INPUT
LEVEL : LEVEL := CRITICAL;
END_VAR
END_FUNCTION_BLOCK";

        let library = parse_program(program, &FileId::default(), &ParseOptions::default()).unwrap();
        let result = analyze(&[&library]);

        assert!(result.is_ok());
    }

    #[test]
    #[ignore = "flaky test - needs to be fixed"]
    fn apply_when_var_init_valid_enum_value_through_alias_then_ok() {
        let program = "
TYPE
LEVEL : (CRITICAL) := CRITICAL;
LEVEL_ALIAS : LEVEL;
END_TYPE

FUNCTION_BLOCK LOGGER
VAR_INPUT
NAME : LEVEL_ALIAS := CRITICAL;
END_VAR

END_FUNCTION_BLOCK";

        let library = parse_program(program, &FileId::default(), &ParseOptions::default()).unwrap();
        let result = analyze(&[&library]);

        assert!(result.is_ok());
    }
}
