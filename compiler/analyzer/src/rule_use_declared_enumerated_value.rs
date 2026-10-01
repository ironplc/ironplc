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
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    // Walk the library to find all references to enumerations
    // checking that all references use an enumeration value
    // that is part of the enumeration
    run_rule(RuleDeclaredEnumeratedValues::new(context), lib)
}

struct RuleDeclaredEnumeratedValues<'a> {
    context: &'a SemanticContext,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> RuleDeclaredEnumeratedValues<'a> {
    fn new(context: &'a SemanticContext) -> Self {
        RuleDeclaredEnumeratedValues {
            context,
            diagnostics: Vec::new(),
        }
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
    fn find_enum_declaration_values(
        &self,
        type_name: &TypeName,
    ) -> Result<Vec<&'a Id>, Diagnostic> {
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

impl DiagnosticVisitor for RuleDeclaredEnumeratedValues<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleDeclaredEnumeratedValues<'_> {
    type Value = ();

    fn visit_enumerated_initial_value_assignment(
        &mut self,
        init: &EnumeratedInitialValueAssignment,
    ) -> Result<Self::Value, Infallible> {
        let defined_values = match self.find_enum_declaration_values(&init.type_name) {
            Ok(values) => values,
            Err(diagnostic) => {
                // The type is not an enumeration, so there is nothing to check
                // this initializer's value against. Report that and carry on to
                // the next declaration.
                self.diagnostics.push(diagnostic);
                return Ok(());
            }
        };
        if let Some(value) = &init.initial_value {
            // Check if the value is in the list of defined enumeration values
            if !defined_values.iter().any(|id| **id == value.value) {
                self.diagnostics.push(
                    Diagnostic::problem(
                        Problem::EnumValueNotDefined,
                        Label::span(value.span(), "Expected value in enumeration"),
                    )
                    .with_context_id("value", &value.value),
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use crate::stages::analyze;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::{options::CompilerOptions, parse_program};

    #[test]
    fn apply_when_two_undefined_enum_values_then_reports_both() {
        let program = "
TYPE
LEVEL : (INFO, WARN) := INFO;
END_TYPE

FUNCTION_BLOCK LOGGER
VAR_INPUT
A : LEVEL := CRITICAL;
B : LEVEL := FATAL;
END_VAR
END_FUNCTION_BLOCK";

        let library =
            parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();
        let (_library, context) = analyze(&[&library], &CompilerOptions::default()).unwrap();

        let reported: Vec<&String> = context
            .diagnostics()
            .iter()
            .flat_map(|d| &d.described)
            .collect();
        assert!(
            reported.iter().any(|d| d.as_str() == "value=CRITICAL"),
            "expected CRITICAL, got {reported:?}"
        );
        assert!(
            reported.iter().any(|d| d.as_str() == "value=FATAL"),
            "expected FATAL, got {reported:?}"
        );
    }

    /// The codes `analyze` reports for `program`.
    fn analyze_codes(program: &str) -> Vec<String> {
        let library =
            parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();
        let (_library, context) = analyze(&[&library], &CompilerOptions::default()).unwrap();
        context
            .diagnostics()
            .iter()
            .map(|d| d.code.clone())
            .collect()
    }

    #[test]
    fn apply_when_two_enumerations_in_one_block_then_ok_on_every_run() {
        // Each analysis builds its hash maps with fresh random keys, so a
        // result that depends on hash iteration order shows up within a
        // few runs (issue #1945).
        let program = "
TYPE A1 : (P, Q, R); A2 : (S0, S1); END_TYPE
PROGRAM main
VAR st : A2 := S1; pt : A1 := R; END_VAR
END_PROGRAM";

        for _ in 0..32 {
            assert_eq!(analyze_codes(program), Vec::<String>::new());
        }
    }

    #[test]
    fn apply_when_two_enumerations_declare_same_value_then_each_accepts_it() {
        let program = "
TYPE A : (X, Y); B : (X, Z); END_TYPE
PROGRAM main
VAR a : A := X; b : B := X; END_VAR
END_PROGRAM";

        assert_eq!(analyze_codes(program), Vec::<String>::new());
    }

    #[test]
    fn apply_when_value_of_other_enumeration_then_enum_value_not_defined() {
        let program = "
TYPE A : (X, Y); B : (X, Z); END_TYPE
PROGRAM main
VAR a : A := Z; END_VAR
END_PROGRAM";

        assert_eq!(analyze_codes(program), vec!["P2006"]);
    }

    #[test]
    fn apply_when_alias_chain_then_values_of_declaring_enumeration() {
        let program = "
TYPE LEVEL : (INFO, WARN); LEVEL1 : LEVEL; LEVEL2 : LEVEL1; END_TYPE
PROGRAM main
VAR ok : LEVEL2 := WARN; bad : LEVEL2 := FATAL; END_VAR
END_PROGRAM";

        for _ in 0..32 {
            assert_eq!(analyze_codes(program), vec!["P2006"]);
        }
    }

    #[test]
    fn apply_when_multiple_enum_values_with_one_undefined_then_error() {
        let program = "
TYPE
LEVEL : (INFO, WARN) := INFO;
END_TYPE

FUNCTION_BLOCK LOGGER
VAR_INPUT
A : LEVEL := INFO;
B : LEVEL := CRITICAL;
END_VAR
END_FUNCTION_BLOCK";

        let library =
            parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();
        let result = analyze(&[&library], &CompilerOptions::default());

        let (_library, context) = result.unwrap();
        assert!(context.has_diagnostics());
    }

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

        let library =
            parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();
        let result = analyze(&[&library], &CompilerOptions::default());

        let (_library, context) = result.unwrap();
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

        let library =
            parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();
        let result = analyze(&[&library], &CompilerOptions::default());

        assert!(result.is_ok());
    }

    #[test]
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

        let library =
            parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();
        let result = analyze(&[&library], &CompilerOptions::default());

        assert!(result.is_ok());
    }
}
