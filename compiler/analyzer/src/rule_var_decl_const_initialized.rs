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
    intermediate_type::IntermediateType, result::SemanticResult, semantic_context::SemanticContext,
    type_environment::TypeEnvironment,
};

pub fn apply(lib: &Library, context: &SemanticContext) -> SemanticResult {
    let mut visitor = RuleConstantVarsInitialized {
        type_environment: context.types(),
        diagnostics: Vec::new(),
    };
    visitor.walk(lib).map_err(|e| vec![e])?;

    if !visitor.diagnostics.is_empty() {
        return Err(visitor.diagnostics);
    }
    Ok(())
}

struct RuleConstantVarsInitialized<'a> {
    type_environment: &'a TypeEnvironment,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Visitor<Diagnostic> for RuleConstantVarsInitialized<'a> {
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
                    // Function blocks cannot be CONSTANT - this is handled by
                    // rule_var_decl_const_not_fb, so skip initialization checking here.
                }
                InitialValueAssignmentKind::Subrange(_) => {
                    return Err(Diagnostic::internal_error(file!(), line!()))
                }
                InitialValueAssignmentKind::Structure(struct_init) => {
                    // For const structures, verify that all fields without defaults
                    // are explicitly initialized in the variable declaration.
                    self.validate_const_structure_init(node, struct_init);
                }
                InitialValueAssignmentKind::Array(array_init) => {
                    if array_init.initial_values.is_empty() {
                        self.diagnostics.push(
                            Diagnostic::problem(
                                Problem::ConstantMustHaveInitializer,
                                Label::span(node.span(), "Variable declaration"),
                            )
                            .with_context("variable", &node.identifier.to_string()),
                        );
                    }
                }
                InitialValueAssignmentKind::LateResolvedType(_) => {
                    return Err(Diagnostic::internal_error(file!(), line!()))
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

impl<'a> RuleConstantVarsInitialized<'a> {
    /// Validates that a const structure variable has all required fields initialized.
    ///
    /// A const structure is fully initialized if all fields either:
    /// 1. Have a default value in the type definition, OR
    /// 2. Are explicitly initialized in the variable declaration (elements_init)
    fn validate_const_structure_init(
        &mut self,
        node: &VarDecl,
        struct_init: &StructureInitializationDeclaration,
    ) {
        // Look up the structure type in the type environment
        let type_attrs = match self.type_environment.get(&struct_init.type_name) {
            Some(attrs) => attrs,
            None => {
                // Type not found - another rule will catch this error
                return;
            }
        };

        // Extract fields from the structure type
        let fields = match &type_attrs.representation {
            IntermediateType::Structure { fields } => fields,
            _ => {
                // Not a structure type - another rule will catch this error
                return;
            }
        };

        // Collect the names of explicitly initialized fields
        let initialized_fields: std::collections::HashSet<_> = struct_init
            .elements_init
            .iter()
            .map(|init| &init.name)
            .collect();

        // Check each field that doesn't have a default
        for field in fields {
            if !field.has_default {
                // This field needs explicit initialization
                if !initialized_fields.iter().any(|name| **name == field.name) {
                    self.diagnostics.push(
                        Diagnostic::problem(
                            Problem::ConstantMustHaveInitializer,
                            Label::span(node.span(), "Constant structure variable"),
                        )
                        .with_context("variable", &node.identifier.to_string())
                        .with_context("field", &field.name.to_string()),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::semantic_context::SemanticContextBuilder;
    use crate::test_helpers::{parse_and_resolve_types, parse_and_resolve_types_with_context};

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
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);

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
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);

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
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);

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
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);

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
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);

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
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);

        assert!(result.is_ok())
    }

    // Tests for const structure initialization

    #[test]
    fn apply_when_const_struct_all_fields_have_defaults_then_ok() {
        let program = "
TYPE
    Point : STRUCT
        x : INT := 0;
        y : INT := 0;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK MAIN
VAR CONSTANT
    origin : Point;
END_VAR
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_const_struct_missing_defaults_but_explicitly_initialized_then_ok() {
        let program = "
TYPE
    Point : STRUCT
        x : INT;
        y : INT;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK MAIN
VAR CONSTANT
    origin : Point := (x := 10, y := 20);
END_VAR
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_const_struct_partial_defaults_with_remaining_initialized_then_ok() {
        let program = "
TYPE
    Point : STRUCT
        x : INT := 0;
        y : INT;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK MAIN
VAR CONSTANT
    origin : Point := (y := 20);
END_VAR
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_const_struct_missing_initialization_for_field_without_default_then_error() {
        let program = "
TYPE
    Point : STRUCT
        x : INT;
        y : INT;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK MAIN
VAR CONSTANT
    origin : Point := (x := 10);
END_VAR
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        // Check that the error mentions the missing field 'y'
        assert!(errors[0].described.iter().any(|s| s.contains("y")));
    }

    #[test]
    fn apply_when_const_struct_no_defaults_and_no_initialization_then_error() {
        let program = "
TYPE
    Point : STRUCT
        x : INT;
        y : INT;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK MAIN
VAR CONSTANT
    origin : Point;
END_VAR
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_err());
        let errors = result.unwrap_err();
        // Should have errors for both x and y fields
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn apply_when_non_const_struct_missing_initialization_then_ok() {
        // Non-constant structures don't require initialization
        let program = "
TYPE
    Point : STRUCT
        x : INT;
        y : INT;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK MAIN
VAR
    origin : Point;
END_VAR
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_const_nested_struct_inner_has_all_defaults_then_ok() {
        // When a nested structure's type has all fields with defaults,
        // the outer struct field should be considered as having a default
        let program = "
TYPE
    Inner : STRUCT
        a : INT := 0;
        b : INT := 0;
    END_STRUCT;
    Outer : STRUCT
        inner : Inner;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK MAIN
VAR CONSTANT
    myOuter : Outer;
END_VAR
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_const_nested_struct_inner_missing_defaults_then_error() {
        // When a nested structure's type has fields without defaults,
        // the outer const should require initialization
        let program = "
TYPE
    Inner : STRUCT
        a : INT;
        b : INT;
    END_STRUCT;
    Outer : STRUCT
        inner : Inner;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK MAIN
VAR CONSTANT
    myOuter : Outer;
END_VAR
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_err());
    }

    #[test]
    fn apply_when_const_deeply_nested_struct_all_have_defaults_then_ok() {
        // Test deeply nested structures where all fields have defaults
        let program = "
TYPE
    Level3 : STRUCT
        value : INT := 42;
    END_STRUCT;
    Level2 : STRUCT
        inner : Level3;
    END_STRUCT;
    Level1 : STRUCT
        inner : Level2;
    END_STRUCT;
END_TYPE

FUNCTION_BLOCK MAIN
VAR CONSTANT
    deepNested : Level1;
END_VAR
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_const_array_type_missing_initializer_then_error() {
        let program = "
FUNCTION_BLOCK LOGGER
VAR CONSTANT
ResetCounterValue : ARRAY[1..10] OF INT;
END_VAR

END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);

        assert!(result.is_err())
    }

    #[test]
    fn apply_when_const_array_type_has_initializer_then_ok() {
        let program = "
FUNCTION_BLOCK LOGGER
VAR CONSTANT
ResetCounterValue : ARRAY[1..3] OF INT := [1, 2, 3];
END_VAR

END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);

        assert!(result.is_ok())
    }
}
