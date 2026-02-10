//! Standard library function definitions for IEC 61131-3.
//!
//! This module defines the standard library functions specified in
//! IEC 61131-3 Section 2.5.1.5, including:
//! - Type conversion functions (INT_TO_REAL, REAL_TO_INT, etc.)
//!
//! These functions are automatically available in the function environment
//! and do not need to be declared by the user.

use ironplc_dsl::common::TypeName;
use ironplc_dsl::core::Id;

use crate::function_environment::FunctionSignature;
use crate::intermediate_type::IntermediateFunctionParameter;

/// Helper to create an input parameter.
fn input_param(name: &str, param_type_name: &str) -> IntermediateFunctionParameter {
    IntermediateFunctionParameter {
        name: Id::from(name),
        param_type: TypeName::from(param_type_name),
        is_input: true,
        is_output: false,
        is_inout: false,
    }
}

/// Creates a type conversion function signature.
///
/// Type conversion functions follow the naming convention `<SOURCE>_TO_<TARGET>`
/// and take a single input parameter of the source type, returning the target type.
fn build_conversion_function(source_name: &str, target_name: &str) -> FunctionSignature {
    let name = format!("{}_TO_{}", source_name, target_name);
    FunctionSignature::stdlib(
        &name,
        TypeName::from(target_name),
        vec![input_param("IN", source_name)],
    )
}

// =============================================================================
// Type Conversion Function Definitions (IEC 61131-3 Section 2.5.1.5)
// =============================================================================

/// Signed integer type names for conversion functions.
const SIGNED_INT_TYPES: &[&str] = &["SINT", "INT", "DINT", "LINT"];

/// Unsigned integer type names for conversion functions.
const UNSIGNED_INT_TYPES: &[&str] = &["USINT", "UINT", "UDINT", "ULINT"];

/// Real (floating-point) type names for conversion functions.
const REAL_TYPES: &[&str] = &["REAL", "LREAL"];

/// Generates all integer-to-integer conversion functions.
///
/// Creates functions like INT_TO_DINT, DINT_TO_INT, SINT_TO_LINT, etc.
fn get_int_to_int_conversions() -> Vec<FunctionSignature> {
    let mut functions = Vec::new();

    // All signed integer types
    for source_name in SIGNED_INT_TYPES {
        for target_name in SIGNED_INT_TYPES {
            if source_name != target_name {
                functions.push(build_conversion_function(source_name, target_name));
            }
        }
    }

    // All unsigned integer types
    for source_name in UNSIGNED_INT_TYPES {
        for target_name in UNSIGNED_INT_TYPES {
            if source_name != target_name {
                functions.push(build_conversion_function(source_name, target_name));
            }
        }
    }

    // Signed to unsigned conversions
    for source_name in SIGNED_INT_TYPES {
        for target_name in UNSIGNED_INT_TYPES {
            functions.push(build_conversion_function(source_name, target_name));
        }
    }

    // Unsigned to signed conversions
    for source_name in UNSIGNED_INT_TYPES {
        for target_name in SIGNED_INT_TYPES {
            functions.push(build_conversion_function(source_name, target_name));
        }
    }

    functions
}

/// Generates all integer-to-real conversion functions.
///
/// Creates functions like INT_TO_REAL, DINT_TO_LREAL, UINT_TO_REAL, etc.
fn get_int_to_real_conversions() -> Vec<FunctionSignature> {
    let mut functions = Vec::new();

    // Signed integer to real
    for source_name in SIGNED_INT_TYPES {
        for target_name in REAL_TYPES {
            functions.push(build_conversion_function(source_name, target_name));
        }
    }

    // Unsigned integer to real
    for source_name in UNSIGNED_INT_TYPES {
        for target_name in REAL_TYPES {
            functions.push(build_conversion_function(source_name, target_name));
        }
    }

    functions
}

/// Generates all real-to-integer conversion functions.
///
/// Creates functions like REAL_TO_INT, LREAL_TO_DINT, REAL_TO_UINT, etc.
fn get_real_to_int_conversions() -> Vec<FunctionSignature> {
    let mut functions = Vec::new();

    // Real to signed integer
    for source_name in REAL_TYPES {
        for target_name in SIGNED_INT_TYPES {
            functions.push(build_conversion_function(source_name, target_name));
        }
    }

    // Real to unsigned integer
    for source_name in REAL_TYPES {
        for target_name in UNSIGNED_INT_TYPES {
            functions.push(build_conversion_function(source_name, target_name));
        }
    }

    functions
}

/// Generates all real-to-real conversion functions.
///
/// Creates functions like REAL_TO_LREAL, LREAL_TO_REAL.
fn get_real_to_real_conversions() -> Vec<FunctionSignature> {
    let mut functions = Vec::new();

    for source_name in REAL_TYPES {
        for target_name in REAL_TYPES {
            if source_name != target_name {
                functions.push(build_conversion_function(source_name, target_name));
            }
        }
    }

    functions
}

// =============================================================================
// Public API
// =============================================================================

/// Returns all standard library function definitions.
///
/// Each function is returned as a FunctionSignature ready to be inserted
/// into the FunctionEnvironment.
pub fn get_all_stdlib_functions() -> Vec<FunctionSignature> {
    let mut functions = Vec::new();

    // Type conversion functions
    functions.extend(get_int_to_int_conversions());
    functions.extend(get_int_to_real_conversions());
    functions.extend(get_real_to_int_conversions());
    functions.extend(get_real_to_real_conversions());

    functions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_all_stdlib_functions_returns_expected_count() {
        let functions = get_all_stdlib_functions();
        // Int-to-int: 4 signed × 3 targets + 4 unsigned × 3 targets + 4×4 signed-to-unsigned + 4×4 unsigned-to-signed
        // = 12 + 12 + 16 + 16 = 56
        // Int-to-real: 4 signed × 2 reals + 4 unsigned × 2 reals = 8 + 8 = 16
        // Real-to-int: 2 reals × 4 signed + 2 reals × 4 unsigned = 8 + 8 = 16
        // Real-to-real: 2 × 1 = 2
        // Total: 56 + 16 + 16 + 2 = 90
        assert_eq!(functions.len(), 90);
    }

    #[test]
    fn build_conversion_function_when_called_then_has_correct_signature() {
        let sig = build_conversion_function("INT", "REAL");

        assert_eq!(sig.name.original(), "INT_TO_REAL");
        assert!(sig.is_stdlib());
        assert_eq!(sig.parameters.len(), 1);
        assert_eq!(sig.parameters[0].name.original(), "IN");
        assert!(sig.parameters[0].is_input);
        // Parameter type is now TypeName, not IntermediateType
        assert_eq!(sig.parameters[0].param_type, TypeName::from("INT"));
        // Return type is now TypeName, not IntermediateType
        assert_eq!(sig.return_type, Some(TypeName::from("REAL")));
    }

    #[test]
    fn get_int_to_int_conversions_contains_expected_functions() {
        let functions = get_int_to_int_conversions();

        // Check some specific conversions exist
        assert!(functions.iter().any(|f| f.name.original() == "INT_TO_DINT"));
        assert!(functions.iter().any(|f| f.name.original() == "DINT_TO_INT"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "SINT_TO_LINT"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "UINT_TO_UDINT"));
        assert!(functions.iter().any(|f| f.name.original() == "INT_TO_UINT"));
        assert!(functions.iter().any(|f| f.name.original() == "UINT_TO_INT"));

        // Self-conversions should not exist
        assert!(!functions.iter().any(|f| f.name.original() == "INT_TO_INT"));
        assert!(!functions
            .iter()
            .any(|f| f.name.original() == "DINT_TO_DINT"));
    }

    #[test]
    fn get_int_to_real_conversions_contains_expected_functions() {
        let functions = get_int_to_real_conversions();

        assert!(functions.iter().any(|f| f.name.original() == "INT_TO_REAL"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "INT_TO_LREAL"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "DINT_TO_REAL"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "UINT_TO_REAL"));
    }

    #[test]
    fn get_real_to_int_conversions_contains_expected_functions() {
        let functions = get_real_to_int_conversions();

        assert!(functions.iter().any(|f| f.name.original() == "REAL_TO_INT"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "LREAL_TO_INT"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "REAL_TO_DINT"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "LREAL_TO_UINT"));
    }

    #[test]
    fn get_real_to_real_conversions_contains_expected_functions() {
        let functions = get_real_to_real_conversions();

        assert_eq!(functions.len(), 2);
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "REAL_TO_LREAL"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "LREAL_TO_REAL"));
    }

    #[test]
    fn stdlib_functions_have_builtin_span() {
        for func in get_all_stdlib_functions() {
            assert!(
                func.is_stdlib(),
                "Expected builtin span for stdlib function {}",
                func.name.original()
            );
        }
    }
}
