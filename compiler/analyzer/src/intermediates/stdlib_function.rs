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

/// Bit string type names (excluding BOOL) for conversion functions.
const BIT_STRING_TYPES: &[&str] = &["BYTE", "WORD", "DWORD", "LWORD"];

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

/// Generates integer-to-BOOL conversion functions.
///
/// Creates functions like SINT_TO_BOOL, INT_TO_BOOL, DINT_TO_BOOL, etc.
/// 0 converts to FALSE, any non-zero value converts to TRUE.
fn get_int_to_bool_conversions() -> Vec<FunctionSignature> {
    ALL_INT_TYPES
        .iter()
        .map(|source| build_conversion_function(source, "BOOL"))
        .collect()
}

// =============================================================================
// Bit String Type Conversion Functions (IEC 61131-3 Section 2.5.1.5)
// =============================================================================

/// Generates bit-string-to-bit-string conversion functions.
///
/// Creates functions like BYTE_TO_WORD, WORD_TO_DWORD, DWORD_TO_LWORD, etc.
fn get_bit_string_to_bit_string_conversions() -> Vec<FunctionSignature> {
    let mut functions = Vec::new();

    for source_name in BIT_STRING_TYPES {
        for target_name in BIT_STRING_TYPES {
            if source_name != target_name {
                functions.push(build_conversion_function(source_name, target_name));
            }
        }
    }

    functions
}

/// Generates bit-string-to-integer conversion functions.
///
/// Creates functions like BYTE_TO_INT, WORD_TO_DINT, DWORD_TO_LINT, etc.
fn get_bit_string_to_int_conversions() -> Vec<FunctionSignature> {
    let mut functions = Vec::new();

    for source_name in BIT_STRING_TYPES {
        for target_name in ALL_INT_TYPES {
            functions.push(build_conversion_function(source_name, target_name));
        }
    }

    functions
}

/// Generates integer-to-bit-string conversion functions.
///
/// Creates functions like INT_TO_BYTE, DINT_TO_WORD, LINT_TO_DWORD, etc.
fn get_int_to_bit_string_conversions() -> Vec<FunctionSignature> {
    let mut functions = Vec::new();

    for source_name in ALL_INT_TYPES {
        for target_name in BIT_STRING_TYPES {
            functions.push(build_conversion_function(source_name, target_name));
        }
    }

    functions
}

/// Generates BOOL-to-bit-string and bit-string-to-BOOL conversion functions.
///
/// Creates functions like BOOL_TO_BYTE, BYTE_TO_BOOL, etc.
fn get_bool_bit_string_conversions() -> Vec<FunctionSignature> {
    let mut functions = Vec::new();

    for bit_type in BIT_STRING_TYPES {
        functions.push(build_conversion_function("BOOL", bit_type));
        functions.push(build_conversion_function(bit_type, "BOOL"));
    }

    functions
}

/// Generates bit-string-to-real and real-to-bit-string conversion functions.
///
/// Creates functions like BYTE_TO_REAL, REAL_TO_BYTE, etc.
fn get_bit_string_real_conversions() -> Vec<FunctionSignature> {
    let mut functions = Vec::new();

    for bit_type in BIT_STRING_TYPES {
        for real_type in REAL_TYPES {
            functions.push(build_conversion_function(bit_type, real_type));
            functions.push(build_conversion_function(real_type, bit_type));
        }
    }

    functions
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
// Assignment Function (IEC 61131-3 Section 2.5.1.5.4)
// =============================================================================

/// Returns the MOVE standard function definition.
///
/// MOVE copies the input value to the output, equivalent to assignment.
/// IEC 61131-3 defines MOVE as operating on ANY type, but since the codegen
/// currently supports numeric types we use ANY_NUM here.
fn get_assignment_functions() -> Vec<FunctionSignature> {
    vec![
        // MOVE: assignment (ANY_NUM -> ANY_NUM)
        FunctionSignature::stdlib(
            "MOVE",
            TypeName::from("ANY_NUM"),
            vec![input_param("IN", "ANY_NUM")],
        ),
    ]
}

// =============================================================================
// Truncation Function (IEC 61131-3 Section 2.5.1.5.2)
// =============================================================================

/// Returns the TRUNC function definition.
///
/// TRUNC truncates a real value toward zero, removing the fractional part.
/// It takes ANY_REAL and returns ANY_INT.
fn get_trunc_function() -> Vec<FunctionSignature> {
    vec![FunctionSignature::stdlib(
        "TRUNC",
        TypeName::from("ANY_INT"),
        vec![input_param("IN", "ANY_REAL")],
    )]
}

// =============================================================================
// BCD Conversion Functions (IEC 61131-3 Section 2.5.1.5)
// =============================================================================

/// Returns BCD conversion function definitions.
///
/// BCD_TO_INT converts a BCD-encoded bit string to an integer.
/// INT_TO_BCD converts an integer to a BCD-encoded bit string.
fn get_bcd_functions() -> Vec<FunctionSignature> {
    vec![
        FunctionSignature::stdlib(
            "BCD_TO_INT",
            TypeName::from("ANY_INT"),
            vec![input_param("IN", "ANY_BIT")],
        ),
        FunctionSignature::stdlib(
            "INT_TO_BCD",
            TypeName::from("ANY_BIT"),
            vec![input_param("IN", "ANY_INT")],
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
/// IEC 61131-3 defines string functions operating on ANY_STRING types.
fn get_string_functions() -> Vec<FunctionSignature> {
    vec![
        // LEN: current length of a string (ANY_STRING -> INT)
        FunctionSignature::stdlib(
            "LEN",
            TypeName::from("INT"),
            vec![input_param("IN", "ANY_STRING")],
        ),
        // FIND: find first occurrence of IN2 within IN1 (ANY_STRING, ANY_STRING -> INT)
        FunctionSignature::stdlib(
            "FIND",
            TypeName::from("INT"),
            vec![
                input_param("IN1", "ANY_STRING"),
                input_param("IN2", "ANY_STRING"),
            ],
        ),
        // REPLACE: replace L chars at position P in IN1 with IN2
        // (ANY_STRING, ANY_STRING, ANY_INT, ANY_INT -> ANY_STRING)
        FunctionSignature::stdlib(
            "REPLACE",
            TypeName::from("ANY_STRING"),
            vec![
                input_param("IN1", "ANY_STRING"),
                input_param("IN2", "ANY_STRING"),
                input_param("L", "ANY_INT"),
                input_param("P", "ANY_INT"),
            ],
        ),
        // INSERT: insert IN2 into IN1 after position P
        // (ANY_STRING, ANY_STRING, ANY_INT -> ANY_STRING)
        FunctionSignature::stdlib(
            "INSERT",
            TypeName::from("ANY_STRING"),
            vec![
                input_param("IN1", "ANY_STRING"),
                input_param("IN2", "ANY_STRING"),
                input_param("P", "ANY_INT"),
            ],
        ),
        // DELETE: delete L chars from IN1 starting at position P
        // (ANY_STRING, ANY_INT, ANY_INT -> ANY_STRING)
        FunctionSignature::stdlib(
            "DELETE",
            TypeName::from("ANY_STRING"),
            vec![
                input_param("IN1", "ANY_STRING"),
                input_param("L", "ANY_INT"),
                input_param("P", "ANY_INT"),
            ],
        ),
        // LEFT: return leftmost L characters of IN
        // (ANY_STRING, ANY_INT -> ANY_STRING)
        FunctionSignature::stdlib(
            "LEFT",
            TypeName::from("ANY_STRING"),
            vec![input_param("IN", "ANY_STRING"), input_param("L", "ANY_INT")],
        ),
        // RIGHT: return rightmost L characters of IN
        // (ANY_STRING, ANY_INT -> ANY_STRING)
        FunctionSignature::stdlib(
            "RIGHT",
            TypeName::from("ANY_STRING"),
            vec![input_param("IN", "ANY_STRING"), input_param("L", "ANY_INT")],
        ),
        // MID: return L characters from IN starting at position P
        // (ANY_STRING, ANY_INT, ANY_INT -> ANY_STRING)
        FunctionSignature::stdlib(
            "MID",
            TypeName::from("ANY_STRING"),
            vec![
                input_param("IN", "ANY_STRING"),
                input_param("L", "ANY_INT"),
                input_param("P", "ANY_INT"),
            ],
        ),
        // CONCAT: concatenate IN1 and IN2
        // (ANY_STRING, ANY_STRING -> ANY_STRING)
        FunctionSignature::stdlib(
            "CONCAT",
            TypeName::from("ANY_STRING"),
            vec![
                input_param("IN1", "ANY_STRING"),
                input_param("IN2", "ANY_STRING"),
            ],
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
    functions.extend(get_int_to_bool_conversions());

    // Bit string type conversion functions
    functions.extend(get_bit_string_to_bit_string_conversions());
    functions.extend(get_bit_string_to_int_conversions());
    functions.extend(get_int_to_bit_string_conversions());
    functions.extend(get_bool_bit_string_conversions());
    functions.extend(get_bit_string_real_conversions());

    // Numeric functions
    functions.extend(get_numeric_functions());

    // Arithmetic functions (functional forms of +, -, *, /, MOD)
    functions.extend(get_arithmetic_functions());

    // Comparison functions (functional forms of >, >=, =, <=, <, <>)
    functions.extend(get_comparison_functions());

    // Boolean functions (functional forms of AND, OR, XOR, NOT)
    functions.extend(get_boolean_functions());

    // Truncation function
    functions.extend(get_trunc_function());

    // BCD conversion functions
    functions.extend(get_bcd_functions());

    // Selection functions
    functions.extend(get_selection_functions());

    // Assignment function (MOVE)
    functions.extend(get_assignment_functions());

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
        // Int-to-bool: 8 (SINT/INT/DINT/LINT/USINT/UINT/UDINT/ULINT to BOOL)
        // Bit-string-to-bit-string: 4 × 3 = 12
        // Bit-string-to-int: 4 × 8 = 32
        // Int-to-bit-string: 8 × 4 = 32
        // Bool-to/from-bit-string: 4 × 2 = 8
        // Bit-string-to/from-real: 4 × 2 × 2 = 16
        // Numeric functions: ABS, SQRT, MIN, MAX, LIMIT, SEL, LN, LOG, EXP, SIN, COS, TAN, ASIN, ACOS, ATAN = 15
        // Truncation function: TRUNC = 1
        // BCD conversion functions: BCD_TO_INT, INT_TO_BCD = 2
        // Arithmetic functions: ADD, SUB, MUL, DIV, MOD = 5
        // Comparison functions: GT, GE, EQ, LE, LT, NE = 6
        // Boolean functions: AND, OR, XOR, NOT = 4
        // Selection functions: MUX = 1
        // Assignment function: MOVE = 1
        // Bit shift/rotate functions: SHL, SHR, ROL, ROR = 4
        // String functions: LEN, FIND, REPLACE, INSERT, DELETE, LEFT, RIGHT, MID, CONCAT = 9
        // Total: 56 + 16 + 16 + 2 + 8 + 8 + 12 + 32 + 32 + 8 + 16 + 15 + 1 + 2 + 5 + 6 + 4 + 1 + 1 + 4 + 9 = 254
        assert_eq!(functions.len(), 254);
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
    fn get_int_to_bool_conversions_when_called_then_contains_all_sources() {
        let functions = get_int_to_bool_conversions();

        assert_eq!(functions.len(), 8);
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "SINT_TO_BOOL"));
        assert!(functions.iter().any(|f| f.name.original() == "INT_TO_BOOL"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "DINT_TO_BOOL"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "LINT_TO_BOOL"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "USINT_TO_BOOL"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "UINT_TO_BOOL"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "UDINT_TO_BOOL"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "ULINT_TO_BOOL"));
    }

    #[test]
    fn get_int_to_bool_conversions_when_called_then_has_correct_signature() {
        let functions = get_int_to_bool_conversions();
        let int_to_bool = functions
            .iter()
            .find(|f| f.name.original() == "INT_TO_BOOL")
            .unwrap();

        assert_eq!(int_to_bool.input_parameter_count(), 1);
        assert_eq!(int_to_bool.parameters[0].name.original(), "IN");
        assert_eq!(int_to_bool.parameters[0].param_type, TypeName::from("INT"));
        assert_eq!(int_to_bool.return_type, Some(TypeName::from("BOOL")));
        assert!(int_to_bool.is_stdlib());
    }

    #[test]
    fn get_bit_string_to_bit_string_conversions_when_called_then_contains_expected() {
        let functions = get_bit_string_to_bit_string_conversions();

        assert_eq!(functions.len(), 12);
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "BYTE_TO_WORD"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "WORD_TO_BYTE"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "DWORD_TO_LWORD"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "LWORD_TO_BYTE"));
    }

    #[test]
    fn get_bit_string_to_bit_string_conversions_when_called_then_has_correct_signature() {
        let functions = get_bit_string_to_bit_string_conversions();
        let byte_to_word = functions
            .iter()
            .find(|f| f.name.original() == "BYTE_TO_WORD")
            .unwrap();

        assert_eq!(byte_to_word.input_parameter_count(), 1);
        assert_eq!(byte_to_word.parameters[0].name.original(), "IN");
        assert_eq!(
            byte_to_word.parameters[0].param_type,
            TypeName::from("BYTE")
        );
        assert_eq!(byte_to_word.return_type, Some(TypeName::from("WORD")));
        assert!(byte_to_word.is_stdlib());
    }

    #[test]
    fn get_bit_string_to_int_conversions_when_called_then_contains_expected() {
        let functions = get_bit_string_to_int_conversions();

        assert_eq!(functions.len(), 32);
        assert!(functions.iter().any(|f| f.name.original() == "BYTE_TO_INT"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "WORD_TO_DINT"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "DWORD_TO_UINT"));
    }

    #[test]
    fn get_int_to_bit_string_conversions_when_called_then_contains_expected() {
        let functions = get_int_to_bit_string_conversions();

        assert_eq!(functions.len(), 32);
        assert!(functions.iter().any(|f| f.name.original() == "INT_TO_BYTE"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "DINT_TO_WORD"));
        assert!(functions
            .iter()
            .any(|f| f.name.original() == "UINT_TO_DWORD"));
    }

    #[test]
    fn get_assignment_functions_when_move_then_has_one_input() {
        let functions = get_assignment_functions();
        let move_fn = functions
            .iter()
            .find(|f| f.name.original() == "MOVE")
            .unwrap();

        assert_eq!(move_fn.input_parameter_count(), 1);
        assert_eq!(move_fn.parameters[0].name.original(), "IN");
        assert!(move_fn.is_stdlib());
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
