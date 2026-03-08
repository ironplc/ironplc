//! Standard library function definitions for IEC 61131-3.
//!
//! This module defines the standard library functions specified in
//! IEC 61131-3 Section 2.5.1, including:
//! - Type conversion functions (INT_TO_REAL, REAL_TO_INT, etc.)
//! - Numeric functions (ABS, SQRT, MIN, MAX, LIMIT)
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

/// All integer type names (signed + unsigned) for BOOL conversion targets.
const ALL_INT_TYPES: &[&str] = &[
    "SINT", "INT", "DINT", "LINT", "USINT", "UINT", "UDINT", "ULINT",
];

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

/// Generates BOOL-to-integer conversion functions.
///
/// Creates functions like BOOL_TO_SINT, BOOL_TO_INT, BOOL_TO_DINT, etc.
/// FALSE converts to 0, TRUE converts to 1.
fn get_bool_to_int_conversions() -> Vec<FunctionSignature> {
    ALL_INT_TYPES
        .iter()
        .map(|target| build_conversion_function("BOOL", target))
        .collect()
}

// =============================================================================
// Numeric Function Definitions (IEC 61131-3 Section 2.5.1.5.2)
// =============================================================================

/// Returns standard numeric function definitions.
///
/// These functions are defined in IEC 61131-3 as operating on generic
/// type categories (ANY_NUM, ANY_REAL). Parameter and return types use
/// the generic type names so that future type validation can check
/// compatibility via `GenericTypeName::is_compatible_with()`.
fn get_numeric_functions() -> Vec<FunctionSignature> {
    vec![
        // ABS: absolute value (ANY_NUM -> ANY_NUM)
        FunctionSignature::stdlib(
            "ABS",
            TypeName::from("ANY_NUM"),
            vec![input_param("IN", "ANY_NUM")],
        ),
        // SQRT: square root (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "SQRT",
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // MIN: minimum of two values (ANY_NUM, ANY_NUM -> ANY_NUM)
        FunctionSignature::stdlib(
            "MIN",
            TypeName::from("ANY_NUM"),
            vec![input_param("IN1", "ANY_NUM"), input_param("IN2", "ANY_NUM")],
        ),
        // MAX: maximum of two values (ANY_NUM, ANY_NUM -> ANY_NUM)
        FunctionSignature::stdlib(
            "MAX",
            TypeName::from("ANY_NUM"),
            vec![input_param("IN1", "ANY_NUM"), input_param("IN2", "ANY_NUM")],
        ),
        // LIMIT: clamp value to range (ANY_NUM, ANY_NUM, ANY_NUM -> ANY_NUM)
        FunctionSignature::stdlib(
            "LIMIT",
            TypeName::from("ANY_NUM"),
            vec![
                input_param("MN", "ANY_NUM"),
                input_param("IN", "ANY_NUM"),
                input_param("MX", "ANY_NUM"),
            ],
        ),
        // SEL: binary selection (BOOL, ANY_NUM, ANY_NUM -> ANY_NUM)
        FunctionSignature::stdlib(
            "SEL",
            TypeName::from("ANY_NUM"),
            vec![
                input_param("G", "BOOL"),
                input_param("IN0", "ANY_NUM"),
                input_param("IN1", "ANY_NUM"),
            ],
        ),
        // LN: natural logarithm (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "LN",
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // LOG: base-10 logarithm (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "LOG",
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // EXP: natural exponential (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "EXP",
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // SIN: sine (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "SIN",
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // COS: cosine (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "COS",
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // TAN: tangent (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "TAN",
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // ASIN: arc sine (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "ASIN",
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // ACOS: arc cosine (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "ACOS",
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // ATAN: arc tangent (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "ATAN",
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
    ]
}

// =============================================================================
// Arithmetic Function Definitions (IEC 61131-3 Section 2.5.1.5.2)
// =============================================================================

/// Returns standard arithmetic function definitions.
///
/// These are the functional equivalents of the arithmetic operators:
/// ADD (+), SUB (-), MUL (*), DIV (/), MOD (MOD).
/// Each takes two inputs and returns a result of the same type.
fn get_arithmetic_functions() -> Vec<FunctionSignature> {
    vec![
        FunctionSignature::stdlib(
            "ADD",
            TypeName::from("ANY_NUM"),
            vec![input_param("IN1", "ANY_NUM"), input_param("IN2", "ANY_NUM")],
        ),
        FunctionSignature::stdlib(
            "SUB",
            TypeName::from("ANY_NUM"),
            vec![input_param("IN1", "ANY_NUM"), input_param("IN2", "ANY_NUM")],
        ),
        FunctionSignature::stdlib(
            "MUL",
            TypeName::from("ANY_NUM"),
            vec![input_param("IN1", "ANY_NUM"), input_param("IN2", "ANY_NUM")],
        ),
        FunctionSignature::stdlib(
            "DIV",
            TypeName::from("ANY_NUM"),
            vec![input_param("IN1", "ANY_NUM"), input_param("IN2", "ANY_NUM")],
        ),
        FunctionSignature::stdlib(
            "MOD",
            TypeName::from("ANY_NUM"),
            vec![input_param("IN1", "ANY_NUM"), input_param("IN2", "ANY_NUM")],
        ),
    ]
}

// =============================================================================
// Comparison Function Definitions (IEC 61131-3 Section 2.5.1.5.3)
// =============================================================================

/// Returns standard comparison function definitions.
///
/// These are the functional equivalents of the comparison operators:
/// GT (>), GE (>=), EQ (=), LE (<=), LT (<), NE (<>).
/// Each takes two inputs and returns BOOL.
fn get_comparison_functions() -> Vec<FunctionSignature> {
    vec![
        FunctionSignature::stdlib(
            "GT",
            TypeName::from("BOOL"),
            vec![input_param("IN1", "ANY_NUM"), input_param("IN2", "ANY_NUM")],
        ),
        FunctionSignature::stdlib(
            "GE",
            TypeName::from("BOOL"),
            vec![input_param("IN1", "ANY_NUM"), input_param("IN2", "ANY_NUM")],
        ),
        FunctionSignature::stdlib(
            "EQ",
            TypeName::from("BOOL"),
            vec![input_param("IN1", "ANY_NUM"), input_param("IN2", "ANY_NUM")],
        ),
        FunctionSignature::stdlib(
            "LE",
            TypeName::from("BOOL"),
            vec![input_param("IN1", "ANY_NUM"), input_param("IN2", "ANY_NUM")],
        ),
        FunctionSignature::stdlib(
            "LT",
            TypeName::from("BOOL"),
            vec![input_param("IN1", "ANY_NUM"), input_param("IN2", "ANY_NUM")],
        ),
        FunctionSignature::stdlib(
            "NE",
            TypeName::from("BOOL"),
            vec![input_param("IN1", "ANY_NUM"), input_param("IN2", "ANY_NUM")],
        ),
    ]
}

// =============================================================================
// Boolean Function Definitions (IEC 61131-3 Section 2.5.1.5.1)
// =============================================================================

/// Returns standard boolean function definitions.
///
/// These are the functional equivalents of the boolean operators:
/// AND, OR, XOR (two inputs), NOT (one input).
/// All take BOOL inputs and return BOOL.
fn get_boolean_functions() -> Vec<FunctionSignature> {
    vec![
        FunctionSignature::stdlib(
            "AND",
            TypeName::from("BOOL"),
            vec![input_param("IN1", "BOOL"), input_param("IN2", "BOOL")],
        ),
        FunctionSignature::stdlib(
            "OR",
            TypeName::from("BOOL"),
            vec![input_param("IN1", "BOOL"), input_param("IN2", "BOOL")],
        ),
        FunctionSignature::stdlib(
            "XOR",
            TypeName::from("BOOL"),
            vec![input_param("IN1", "BOOL"), input_param("IN2", "BOOL")],
        ),
        FunctionSignature::stdlib(
            "NOT",
            TypeName::from("BOOL"),
            vec![input_param("IN", "BOOL")],
        ),
    ]
}

// =============================================================================
// Selection Function Definitions (IEC 61131-3 Section 2.5.1.5.4)
// =============================================================================

/// Returns standard selection function definitions.
///
/// MUX is an extensible multiplexer that selects one of N inputs based on
/// an integer selector K. Unlike SEL (which uses a BOOL selector and exactly
/// 2 inputs), MUX uses an ANY_INT selector and supports 2..16 inputs.
///
/// The declared parameters define the minimum (K + 2 IN values = 3 args).
/// Additional IN arguments are accepted because the signature is extensible.
fn get_selection_functions() -> Vec<FunctionSignature> {
    vec![
        // MUX: multiplexer (ANY_INT, ANY_NUM, ANY_NUM, ... -> ANY_NUM)
        // MUX supports K + 2..16 IN values = 3..17 total input arguments
        FunctionSignature::stdlib_extensible(
            "MUX",
            TypeName::from("ANY_NUM"),
            vec![
                input_param("K", "ANY_INT"),
                input_param("IN0", "ANY_NUM"),
                input_param("IN1", "ANY_NUM"),
            ],
            17,
        ),
    ]
}

// =============================================================================
// Bit shift and rotate functions
// =============================================================================

/// Returns standard bit shift and rotate function definitions.
///
/// IEC 61131-3 defines SHL, SHR, ROL, ROR as standard functions operating
/// on ANY_BIT types with an ANY_INT shift count. The return type matches
/// the input type.
fn get_bitshift_functions() -> Vec<FunctionSignature> {
    vec![
        FunctionSignature::stdlib(
            "SHL",
            TypeName::from("ANY_BIT"),
            vec![input_param("IN", "ANY_BIT"), input_param("N", "ANY_INT")],
        ),
        FunctionSignature::stdlib(
            "SHR",
            TypeName::from("ANY_BIT"),
            vec![input_param("IN", "ANY_BIT"), input_param("N", "ANY_INT")],
        ),
        FunctionSignature::stdlib(
            "ROL",
            TypeName::from("ANY_BIT"),
            vec![input_param("IN", "ANY_BIT"), input_param("N", "ANY_INT")],
        ),
        FunctionSignature::stdlib(
            "ROR",
            TypeName::from("ANY_BIT"),
            vec![input_param("IN", "ANY_BIT"), input_param("N", "ANY_INT")],
        ),
    ]
}

/// Returns standard string function definitions.
///
/// IEC 61131-3 defines LEN as a standard function operating on
/// ANY_STRING types and returning an INT result.
fn get_string_functions() -> Vec<FunctionSignature> {
    vec![
        // LEN: current length of a string (ANY_STRING -> INT)
        FunctionSignature::stdlib(
            "LEN",
            TypeName::from("INT"),
            vec![input_param("IN", "ANY_STRING")],
        ),
    ]
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
    functions.extend(get_bool_to_int_conversions());

    // Numeric functions
    functions.extend(get_numeric_functions());

    // Arithmetic functions (functional forms of +, -, *, /, MOD)
    functions.extend(get_arithmetic_functions());

    // Comparison functions (functional forms of >, >=, =, <=, <, <>)
    functions.extend(get_comparison_functions());

    // Boolean functions (functional forms of AND, OR, XOR, NOT)
    functions.extend(get_boolean_functions());

    // Selection functions
    functions.extend(get_selection_functions());

    // Bit shift and rotate functions
    functions.extend(get_bitshift_functions());

    // String functions
    functions.extend(get_string_functions());

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
        // Bool-to-int: 8 (BOOL to SINT/INT/DINT/LINT/USINT/UINT/UDINT/ULINT)
        // Numeric functions: ABS, SQRT, MIN, MAX, LIMIT, SEL, LN, LOG, EXP, SIN, COS, TAN, ASIN, ACOS, ATAN = 15
        // Arithmetic functions: ADD, SUB, MUL, DIV, MOD = 5
        // Comparison functions: GT, GE, EQ, LE, LT, NE = 6
        // Boolean functions: AND, OR, XOR, NOT = 4
        // Selection functions: MUX = 1
        // Bit shift/rotate functions: SHL, SHR, ROL, ROR = 4
        // String functions: LEN = 1
        // Total: 56 + 16 + 16 + 2 + 8 + 15 + 5 + 6 + 4 + 1 + 4 + 1 = 134
        assert_eq!(functions.len(), 134);
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
    fn get_numeric_functions_when_called_then_contains_all_functions() {
        let functions = get_numeric_functions();

        assert_eq!(functions.len(), 15);

        assert!(functions.iter().any(|f| f.name.original() == "ABS"));
        assert!(functions.iter().any(|f| f.name.original() == "SQRT"));
        assert!(functions.iter().any(|f| f.name.original() == "MIN"));
        assert!(functions.iter().any(|f| f.name.original() == "MAX"));
        assert!(functions.iter().any(|f| f.name.original() == "LIMIT"));
        assert!(functions.iter().any(|f| f.name.original() == "SEL"));
        assert!(functions.iter().any(|f| f.name.original() == "LN"));
        assert!(functions.iter().any(|f| f.name.original() == "LOG"));
        assert!(functions.iter().any(|f| f.name.original() == "EXP"));
        assert!(functions.iter().any(|f| f.name.original() == "SIN"));
        assert!(functions.iter().any(|f| f.name.original() == "COS"));
        assert!(functions.iter().any(|f| f.name.original() == "TAN"));
        assert!(functions.iter().any(|f| f.name.original() == "ASIN"));
        assert!(functions.iter().any(|f| f.name.original() == "ACOS"));
        assert!(functions.iter().any(|f| f.name.original() == "ATAN"));
    }

    #[test]
    fn get_numeric_functions_when_abs_then_has_one_input() {
        let functions = get_numeric_functions();
        let abs = functions
            .iter()
            .find(|f| f.name.original() == "ABS")
            .unwrap();

        assert_eq!(abs.input_parameter_count(), 1);
        assert_eq!(abs.parameters[0].name.original(), "IN");
        assert!(abs.is_stdlib());
    }

    #[test]
    fn get_numeric_functions_when_sqrt_then_has_one_input() {
        let functions = get_numeric_functions();
        let sqrt = functions
            .iter()
            .find(|f| f.name.original() == "SQRT")
            .unwrap();

        assert_eq!(sqrt.input_parameter_count(), 1);
        assert_eq!(sqrt.parameters[0].name.original(), "IN");
        assert!(sqrt.is_stdlib());
    }

    #[test]
    fn get_numeric_functions_when_min_then_has_two_inputs() {
        let functions = get_numeric_functions();
        let min = functions
            .iter()
            .find(|f| f.name.original() == "MIN")
            .unwrap();

        assert_eq!(min.input_parameter_count(), 2);
        assert_eq!(min.parameters[0].name.original(), "IN1");
        assert_eq!(min.parameters[1].name.original(), "IN2");
        assert!(min.is_stdlib());
    }

    #[test]
    fn get_numeric_functions_when_max_then_has_two_inputs() {
        let functions = get_numeric_functions();
        let max = functions
            .iter()
            .find(|f| f.name.original() == "MAX")
            .unwrap();

        assert_eq!(max.input_parameter_count(), 2);
        assert_eq!(max.parameters[0].name.original(), "IN1");
        assert_eq!(max.parameters[1].name.original(), "IN2");
        assert!(max.is_stdlib());
    }

    #[test]
    fn get_numeric_functions_when_limit_then_has_three_inputs() {
        let functions = get_numeric_functions();
        let limit = functions
            .iter()
            .find(|f| f.name.original() == "LIMIT")
            .unwrap();

        assert_eq!(limit.input_parameter_count(), 3);
        assert_eq!(limit.parameters[0].name.original(), "MN");
        assert_eq!(limit.parameters[1].name.original(), "IN");
        assert_eq!(limit.parameters[2].name.original(), "MX");
        assert!(limit.is_stdlib());
    }

    #[test]
    fn get_numeric_functions_when_sel_then_has_three_inputs() {
        let functions = get_numeric_functions();
        let sel = functions
            .iter()
            .find(|f| f.name.original() == "SEL")
            .unwrap();

        assert_eq!(sel.input_parameter_count(), 3);
        assert_eq!(sel.parameters[0].name.original(), "G");
        assert_eq!(sel.parameters[1].name.original(), "IN0");
        assert_eq!(sel.parameters[2].name.original(), "IN1");
        assert!(sel.is_stdlib());
    }

    #[test]
    fn get_selection_functions_when_called_then_contains_mux() {
        let functions = get_selection_functions();

        assert_eq!(functions.len(), 1);
        assert!(functions.iter().any(|f| f.name.original() == "MUX"));
    }

    #[test]
    fn get_selection_functions_when_mux_then_has_three_minimum_inputs() {
        let functions = get_selection_functions();
        let mux = functions
            .iter()
            .find(|f| f.name.original() == "MUX")
            .unwrap();

        assert_eq!(mux.input_parameter_count(), 3);
        assert_eq!(mux.parameters[0].name.original(), "K");
        assert_eq!(mux.parameters[1].name.original(), "IN0");
        assert_eq!(mux.parameters[2].name.original(), "IN1");
        assert!(mux.is_stdlib());
        assert!(mux.is_extensible);
    }

    #[test]
    fn get_bool_to_int_conversions_when_called_then_contains_all_targets() {
        let functions = get_bool_to_int_conversions();

        assert_eq!(functions.len(), 8);
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "BOOL_TO_SINT"));
        assert!(functions.iter().any(|f| f.name.original() == "BOOL_TO_INT"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "BOOL_TO_DINT"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "BOOL_TO_LINT"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "BOOL_TO_USINT"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "BOOL_TO_UINT"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "BOOL_TO_UDINT"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "BOOL_TO_ULINT"));
    }

    #[test]
    fn get_bool_to_int_conversions_when_called_then_has_correct_signature() {
        let functions = get_bool_to_int_conversions();
        let bool_to_int = functions
            .iter()
            .find(|f| f.name.original() == "BOOL_TO_INT")
            .unwrap();

        assert_eq!(bool_to_int.input_parameter_count(), 1);
        assert_eq!(bool_to_int.parameters[0].name.original(), "IN");
        assert_eq!(bool_to_int.parameters[0].param_type, TypeName::from("BOOL"));
        assert_eq!(bool_to_int.return_type, Some(TypeName::from("INT")));
        assert!(bool_to_int.is_stdlib());
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
