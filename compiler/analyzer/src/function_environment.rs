//! Function environment for tracking function signatures.
//!
//! This module provides the FunctionEnvironment struct which maintains a registry
//! of function signatures (both standard library functions and user-defined functions).
//! Unlike TypeEnvironment which tracks types, this tracks callable functions and
//! their signatures.
//!
//! Function signatures store type names (not resolved types) to allow building
//! complete signatures even when type resolution fails. Types can be resolved
//! on-demand via `TypeEnvironment` when needed for validation.

use std::collections::HashMap;

use ironplc_dsl::common::TypeName;
use ironplc_dsl::core::{Id, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;

use crate::intermediate_type::IntermediateFunctionParameter;
use crate::intermediates::stdlib_function::get_all_stdlib_functions;

/// Represents a function signature in the function environment.
///
/// Stores type names (not resolved types) to allow building complete signatures
/// even when type resolution fails. Use `resolve_return_type()` and
/// `resolve_param_type()` to get the actual types via TypeEnvironment.
#[derive(Debug, Clone)]
pub struct FunctionSignature {
    /// Name of the function
    pub name: Id,
    /// Return type name of the function (None for procedures)
    pub return_type: Option<TypeName>,
    /// List of function parameters
    pub parameters: Vec<IntermediateFunctionParameter>,
    /// Source location (builtin for stdlib functions)
    pub span: SourceSpan,
}

impl FunctionSignature {
    /// Creates a new function signature.
    pub fn new(
        name: Id,
        return_type: Option<TypeName>,
        parameters: Vec<IntermediateFunctionParameter>,
        span: SourceSpan,
    ) -> Self {
        Self {
            name,
            return_type,
            parameters,
            span,
        }
    }

    /// Creates a stdlib function signature with a builtin span.
    pub fn stdlib(
        name: &str,
        return_type: TypeName,
        parameters: Vec<IntermediateFunctionParameter>,
    ) -> Self {
        Self {
            name: Id::from(name),
            return_type: Some(return_type),
            parameters,
            span: SourceSpan::builtin(),
        }
    }

    /// Returns true if this is a standard library function.
    pub fn is_stdlib(&self) -> bool {
        self.span.is_builtin()
    }

    /// Returns the number of input parameters.
    pub fn input_parameter_count(&self) -> usize {
        self.parameters.iter().filter(|p| p.is_input).count()
    }
}

/// The function environment tracks all function signatures.
#[derive(Debug)]
pub struct FunctionEnvironment {
    /// Map from lowercase function name to signature
    table: HashMap<String, FunctionSignature>,
}

impl FunctionEnvironment {
    /// Creates a new empty function environment.
    pub fn new() -> Self {
        Self {
            table: HashMap::new(),
        }
    }

    /// Inserts a function signature into the environment.
    ///
    /// Uses case-insensitive lookup (stores lowercase key).
    ///
    /// Returns an error if a function already exists with the same name.
    pub fn insert(&mut self, signature: FunctionSignature) -> Result<(), Diagnostic> {
        let key = signature.name.lower_case().to_string();
        let span = signature.span.clone();

        self.table.insert(key, signature).map_or_else(
            || Ok(()),
            |existing| {
                Err(Diagnostic::problem(
                    Problem::FunctionDeclNameDuplicated,
                    Label::span(span, "Function declaration"),
                )
                .with_secondary(Label::span(existing.span, "Previous declaration")))
            },
        )
    }

    /// Gets a function signature by name.
    ///
    /// Uses case-insensitive lookup.
    pub fn get(&self, name: &Id) -> Option<&FunctionSignature> {
        self.table.get(&name.lower_case().to_string())
    }

    /// Returns true if the function exists in the environment.
    pub fn contains(&self, name: &Id) -> bool {
        self.table.contains_key(&name.lower_case().to_string())
    }

    /// Returns an iterator over all function signatures.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &FunctionSignature)> {
        self.table.iter()
    }

    /// Returns the number of functions in the environment.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.table.len()
    }

    /// Returns true if the environment is empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }
}

impl Default for FunctionEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for constructing a FunctionEnvironment.
pub struct FunctionEnvironmentBuilder {
    has_stdlib_functions: bool,
}

impl FunctionEnvironmentBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self {
            has_stdlib_functions: false,
        }
    }

    /// Adds standard library functions to the environment.
    ///
    /// This will include type conversion functions like INT_TO_REAL,
    /// REAL_TO_INT, etc. when implemented.
    pub fn with_stdlib_functions(mut self) -> Self {
        self.has_stdlib_functions = true;
        self
    }

    /// Builds the function environment.
    pub fn build(self) -> FunctionEnvironment {
        let mut env = FunctionEnvironment::new();

        if self.has_stdlib_functions {
            for sig in get_all_stdlib_functions() {
                // Stdlib functions should never have duplicates, so unwrap is safe here
                env.insert(sig)
                    .expect("Stdlib function names should be unique");
            }
        }

        env
    }
}

impl Default for FunctionEnvironmentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_signature(name: &str, return_type_name: &str) -> FunctionSignature {
        FunctionSignature::stdlib(name, TypeName::from(return_type_name), vec![])
    }

    #[test]
    fn function_environment_new_when_created_then_empty() {
        let env = FunctionEnvironment::new();
        assert!(env.is_empty());
        assert_eq!(env.len(), 0);
    }

    #[test]
    fn function_environment_insert_when_function_added_then_can_retrieve() {
        let mut env = FunctionEnvironment::new();
        let sig = create_test_signature("INT_TO_REAL", "REAL");

        assert!(env.insert(sig).is_ok());

        assert!(!env.is_empty());
        assert_eq!(env.len(), 1);
        assert!(env.contains(&Id::from("INT_TO_REAL")));
    }

    #[test]
    fn function_environment_insert_when_duplicate_then_error() {
        let mut env = FunctionEnvironment::new();
        let sig1 = create_test_signature("INT_TO_REAL", "REAL");
        let sig2 = create_test_signature("INT_TO_REAL", "REAL");

        assert!(env.insert(sig1).is_ok());
        assert!(env.insert(sig2).is_err());

        // Should still only have one entry
        assert_eq!(env.len(), 1);
    }

    #[test]
    fn function_environment_get_when_case_insensitive_then_finds_function() {
        let mut env = FunctionEnvironment::new();
        let sig = create_test_signature("INT_TO_REAL", "REAL");

        env.insert(sig).unwrap();

        // Should find with different cases
        assert!(env.get(&Id::from("INT_TO_REAL")).is_some());
        assert!(env.get(&Id::from("int_to_real")).is_some());
        assert!(env.get(&Id::from("Int_To_Real")).is_some());
    }

    #[test]
    fn function_environment_get_when_not_found_then_returns_none() {
        let env = FunctionEnvironment::new();
        assert!(env.get(&Id::from("NONEXISTENT")).is_none());
    }

    #[test]
    fn function_signature_is_stdlib_when_builtin_span_then_true() {
        let sig = FunctionSignature::stdlib("INT_TO_REAL", TypeName::from("REAL"), vec![]);
        assert!(sig.is_stdlib());
    }

    #[test]
    fn function_signature_is_stdlib_when_user_defined_then_false() {
        let sig = FunctionSignature::new(
            Id::from("MY_FUNC"),
            Some(TypeName::from("BOOL")),
            vec![],
            SourceSpan::default(),
        );
        assert!(!sig.is_stdlib());
    }

    #[test]
    fn function_signature_input_parameter_count_when_mixed_params_then_counts_inputs() {
        let params = vec![
            IntermediateFunctionParameter {
                name: Id::from("IN1"),
                param_type: TypeName::from("INT"),
                is_input: true,
                is_output: false,
                is_inout: false,
            },
            IntermediateFunctionParameter {
                name: Id::from("IN2"),
                param_type: TypeName::from("INT"),
                is_input: true,
                is_output: false,
                is_inout: false,
            },
            IntermediateFunctionParameter {
                name: Id::from("OUT1"),
                param_type: TypeName::from("INT"),
                is_input: false,
                is_output: true,
                is_inout: false,
            },
        ];

        let sig = FunctionSignature::new(
            Id::from("MY_FUNC"),
            Some(TypeName::from("BOOL")),
            params,
            SourceSpan::default(),
        );

        assert_eq!(sig.input_parameter_count(), 2);
    }

    #[test]
    fn function_environment_builder_when_default_then_empty() {
        let env = FunctionEnvironmentBuilder::new().build();
        assert!(env.is_empty());
    }

    #[test]
    fn function_environment_builder_when_with_stdlib_functions_then_builds() {
        let env = FunctionEnvironmentBuilder::new()
            .with_stdlib_functions()
            .build();
        // Should have 90 conversion + 5 numeric = 95 stdlib functions
        assert_eq!(env.len(), 95);
        // Should be able to find conversion functions
        assert!(env.contains(&Id::from("INT_TO_REAL")));
        assert!(env.contains(&Id::from("REAL_TO_INT")));
        assert!(env.contains(&Id::from("DINT_TO_LINT")));
    }

    #[test]
    fn function_environment_iter_when_functions_added_then_iterates_all() {
        let mut env = FunctionEnvironment::new();
        env.insert(create_test_signature("FUNC1", "BOOL")).unwrap();
        env.insert(create_test_signature("FUNC2", "INT")).unwrap();

        let names: Vec<_> = env.iter().map(|(name, _)| name.clone()).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"func1".to_string()));
        assert!(names.contains(&"func2".to_string()));
    }
}
