//! Standard library function definitions for IEC 61131-3.
//!
//! This module defines the standard library functions specified in
//! IEC 61131-3 Section 2.5.1, including:
//! - Numeric functions (ABS, SQRT, MIN, MAX, LIMIT)
//!
//! The type conversion functions (INT_TO_REAL, REAL_TO_INT, etc.) are in
//! `stdlib_conversion_function` and the time and date functions in
//! `stdlib_time_function`. Each signature names the [`BuiltinFunction`] it stands
//! for.
//!
//! These functions are automatically available in the function environment
//! and do not need to be declared by the user.

use ironplc_dsl::common::{ElementaryTypeName, GenericTypeName, TypeName};
use ironplc_dsl::core::Id;

use crate::builtin_function::{BitShift, BuiltinFunction, NumericFunction, StringFunction};
use crate::function_environment::FunctionSignature;
use crate::intermediates::operator_function_form;
use crate::intermediates::stdlib_conversion_function;
use crate::intermediates::stdlib_time_function;
use crate::semantic_type::{ParameterType, SemanticFunctionParameter};
use crate::type_id;

/// Helper to create an input parameter of the generic category or
/// elementary type `param_type_name`.
pub(super) fn input_param(name: &str, param_type_name: &str) -> SemanticFunctionParameter {
    SemanticFunctionParameter {
        name: Id::from(name),
        param_type: stdlib_parameter_type(param_type_name),
        is_input: true,
        is_output: false,
        is_inout: false,
        is_reference: false,
    }
}

/// The type a standard function's parameter is declared as `name`: a generic
/// category or an elementary type, both of which the type environment knows
/// without a library.
pub(crate) fn stdlib_parameter_type(name: &str) -> ParameterType {
    let id = Id::from(name);
    if let Ok(generic) = GenericTypeName::try_from(&id) {
        return ParameterType::Generic(generic);
    }
    match ElementaryTypeName::try_from(&id) {
        Ok(elementary) => ParameterType::Type(type_id::elementary(&elementary)),
        Err(()) => ParameterType::Unresolved(TypeName::from(name)),
    }
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
            BuiltinFunction::Numeric(NumericFunction::Abs),
            TypeName::from("ANY_NUM"),
            vec![input_param("IN", "ANY_NUM")],
        ),
        // SQRT: square root (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "SQRT",
            BuiltinFunction::Numeric(NumericFunction::Sqrt),
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // MIN: minimum of two values (ANY_NUM, ANY_NUM -> ANY_NUM)
        FunctionSignature::stdlib(
            "MIN",
            BuiltinFunction::Numeric(NumericFunction::Min),
            TypeName::from("ANY_NUM"),
            vec![input_param("IN1", "ANY_NUM"), input_param("IN2", "ANY_NUM")],
        ),
        // MAX: maximum of two values (ANY_NUM, ANY_NUM -> ANY_NUM)
        FunctionSignature::stdlib(
            "MAX",
            BuiltinFunction::Numeric(NumericFunction::Max),
            TypeName::from("ANY_NUM"),
            vec![input_param("IN1", "ANY_NUM"), input_param("IN2", "ANY_NUM")],
        ),
        // LIMIT: clamp value to range (ANY_NUM, ANY_NUM, ANY_NUM -> ANY_NUM)
        FunctionSignature::stdlib(
            "LIMIT",
            BuiltinFunction::Numeric(NumericFunction::Limit),
            TypeName::from("ANY_NUM"),
            vec![
                input_param("MN", "ANY_NUM"),
                input_param("IN", "ANY_NUM"),
                input_param("MX", "ANY_NUM"),
            ],
        ),
        // SEL: binary selection (BOOL, ANY, ANY -> ANY)
        FunctionSignature::stdlib(
            "SEL",
            BuiltinFunction::Numeric(NumericFunction::Sel),
            TypeName::from("ANY"),
            vec![
                input_param("G", "BOOL"),
                input_param("IN0", "ANY"),
                input_param("IN1", "ANY"),
            ],
        ),
        // LN: natural logarithm (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "LN",
            BuiltinFunction::Numeric(NumericFunction::Ln),
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // LOG: base-10 logarithm (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "LOG",
            BuiltinFunction::Numeric(NumericFunction::Log),
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // EXP: natural exponential (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "EXP",
            BuiltinFunction::Numeric(NumericFunction::Exp),
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // SIN: sine (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "SIN",
            BuiltinFunction::Numeric(NumericFunction::Sin),
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // COS: cosine (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "COS",
            BuiltinFunction::Numeric(NumericFunction::Cos),
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // TAN: tangent (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "TAN",
            BuiltinFunction::Numeric(NumericFunction::Tan),
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // ASIN: arc sine (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "ASIN",
            BuiltinFunction::Numeric(NumericFunction::Asin),
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // ACOS: arc cosine (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "ACOS",
            BuiltinFunction::Numeric(NumericFunction::Acos),
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // ATAN: arc tangent (ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "ATAN",
            BuiltinFunction::Numeric(NumericFunction::Atan),
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        // ATAN2: two-argument arc tangent (ANY_REAL, ANY_REAL -> ANY_REAL)
        FunctionSignature::stdlib(
            "ATAN2",
            BuiltinFunction::Numeric(NumericFunction::Atan2),
            TypeName::from("ANY_REAL"),
            vec![
                input_param("IN1", "ANY_REAL"),
                input_param("IN2", "ANY_REAL"),
            ],
        ),
        // EXPT: exponentiation (ANY_NUM, ANY_NUM -> ANY_NUM)
        FunctionSignature::stdlib(
            "EXPT",
            BuiltinFunction::Numeric(NumericFunction::Expt),
            TypeName::from("ANY_NUM"),
            vec![input_param("IN1", "ANY_NUM"), input_param("IN2", "ANY_NUM")],
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
/// 2 inputs), MUX uses an ANY_INT selector and supports 2..16 inputs. Its
/// inputs are of any type (`ANY`), as SEL's are.
///
/// The declared parameters define the minimum (K + 2 IN values = 3 args).
/// Additional IN arguments are accepted because the signature is extensible.
fn get_selection_functions() -> Vec<FunctionSignature> {
    vec![
        // MUX: multiplexer (ANY_INT, ANY, ANY, ... -> ANY)
        // MUX supports K + 2..16 IN values = 3..17 total input arguments
        FunctionSignature::stdlib_extensible(
            "MUX",
            BuiltinFunction::Mux,
            TypeName::from("ANY"),
            vec![
                input_param("K", "ANY_INT"),
                input_param("IN0", "ANY"),
                input_param("IN1", "ANY"),
            ],
            Some(17),
        ),
    ]
}

// =============================================================================
// Assignment Function (IEC 61131-3 Section 2.5.1.5.4)
// =============================================================================

/// Returns the MOVE standard function definition.
///
/// MOVE copies the input value to the output, equivalent to assignment, and
/// IEC 61131-3 declares it on any type (`ANY`).
fn get_assignment_functions() -> Vec<FunctionSignature> {
    vec![
        // MOVE: assignment (ANY -> ANY)
        FunctionSignature::stdlib(
            "MOVE",
            BuiltinFunction::Move,
            TypeName::from("ANY"),
            vec![input_param("IN", "ANY")],
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
        BuiltinFunction::Trunc,
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
            BuiltinFunction::BcdToInt,
            TypeName::from("ANY_INT"),
            vec![input_param("IN", "ANY_BIT")],
        ),
        FunctionSignature::stdlib(
            "INT_TO_BCD",
            BuiltinFunction::IntToBcd,
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
            BuiltinFunction::BitShift(BitShift::Shl),
            TypeName::from("ANY_BIT"),
            vec![input_param("IN", "ANY_BIT"), input_param("N", "ANY_INT")],
        ),
        FunctionSignature::stdlib(
            "SHR",
            BuiltinFunction::BitShift(BitShift::Shr),
            TypeName::from("ANY_BIT"),
            vec![input_param("IN", "ANY_BIT"), input_param("N", "ANY_INT")],
        ),
        FunctionSignature::stdlib(
            "ROL",
            BuiltinFunction::BitShift(BitShift::Rol),
            TypeName::from("ANY_BIT"),
            vec![input_param("IN", "ANY_BIT"), input_param("N", "ANY_INT")],
        ),
        FunctionSignature::stdlib(
            "ROR",
            BuiltinFunction::BitShift(BitShift::Ror),
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
            BuiltinFunction::String(StringFunction::Len),
            TypeName::from("INT"),
            vec![input_param("IN", "ANY_STRING")],
        ),
        // FIND: find first occurrence of IN2 within IN1 (ANY_STRING, ANY_STRING -> INT)
        FunctionSignature::stdlib(
            "FIND",
            BuiltinFunction::String(StringFunction::Find),
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
            BuiltinFunction::String(StringFunction::Replace),
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
            BuiltinFunction::String(StringFunction::Insert),
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
            BuiltinFunction::String(StringFunction::Delete),
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
            BuiltinFunction::String(StringFunction::Left),
            TypeName::from("ANY_STRING"),
            vec![input_param("IN", "ANY_STRING"), input_param("L", "ANY_INT")],
        ),
        // RIGHT: return rightmost L characters of IN
        // (ANY_STRING, ANY_INT -> ANY_STRING)
        FunctionSignature::stdlib(
            "RIGHT",
            BuiltinFunction::String(StringFunction::Right),
            TypeName::from("ANY_STRING"),
            vec![input_param("IN", "ANY_STRING"), input_param("L", "ANY_INT")],
        ),
        // MID: return L characters from IN starting at position P
        // (ANY_STRING, ANY_INT, ANY_INT -> ANY_STRING)
        FunctionSignature::stdlib(
            "MID",
            BuiltinFunction::String(StringFunction::Mid),
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
            BuiltinFunction::String(StringFunction::Concat),
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

    // Type conversion functions (IEC 61131-3 Section 2.5.1.5)
    functions.extend(stdlib_conversion_function::get_conversion_functions());

    // Numeric functions
    functions.extend(get_numeric_functions());

    // Function forms of operators (+, -, *, /, MOD, comparisons, AND, OR, XOR, NOT)
    functions.extend(operator_function_form::signatures());

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

    // Time functions (IEC 61131-3 Section 2.5.1.5.8, Table 35)
    functions.extend(stdlib_time_function::get_time_functions());

    // Compiler intrinsics (reserved `__` namespace)
    functions.extend(get_compiler_intrinsic_functions());

    functions
}

// =============================================================================
// Compiler intrinsics (reserved `__` namespace)
// =============================================================================

/// Returns the `__`-prefixed compiler intrinsic function definitions.
///
/// The `__` prefix is the established compiler-provided namespace (like
/// `__SYSTEM_UP_TIME`): names only the compiler can provide, visibly
/// non-portable, and colliding with no IEC 61131-3 or vendor name. These
/// exist for behavior IEC 61131-3 source cannot express, and their intended
/// callers are bundled compatibility-library bodies (e.g. `Tc2_Math`'s
/// `LTRUNC := __TRUNC(IN);`). They are seeded unconditionally because
/// library bodies are analyzed under the *user's* options, so a flag gate
/// would break every library that uses them.
///
/// - `__TRUNC(IN: ANY_REAL): ANY_REAL` — truncation toward zero that stays
///   in the input's real type (unlike `TRUNC`, whose `ANY_INT` result clamps
///   values beyond the integer range).
/// - `__MOD(IN1, IN2: ANY_REAL): ANY_REAL` — IEEE-754 floating remainder
///   with the sign of the dividend (unlike `MOD`, which is integer-only);
///   `__MOD(x, 0.0)` is NaN, never a runtime error.
///
/// **Why named intrinsics rather than a manifest binding.** The alternative
/// was to let a library manifest bind a vendor name straight to an unnamed
/// VM builtin, which is what [ADR-0042](../../../../specs/adrs/0042-library-functions-over-compiler-intrinsics.md)
/// rule 3 still describes. It was rejected on security grounds: it makes an
/// on-disk data file an input to code *emission*, and nothing structurally
/// guarantees a library's declared signature matches the builtin's stack
/// behaviour — a mismatched binding would corrupt the operand stack. With
/// `__TRUNC`/`__MOD` as ordinary typed intrinsics, every `BUILTIN` emission
/// originates from a compiler-owned table, manifests stay pure metadata, and
/// the signature is type-checked like any other stdlib function, so that
/// mismatch cannot exist. No manifest binding mechanism was ever built.
pub fn get_compiler_intrinsic_functions() -> Vec<FunctionSignature> {
    vec![
        FunctionSignature::stdlib(
            "__TRUNC",
            BuiltinFunction::Numeric(NumericFunction::TruncReal),
            TypeName::from("ANY_REAL"),
            vec![input_param("IN", "ANY_REAL")],
        ),
        FunctionSignature::stdlib(
            "__MOD",
            BuiltinFunction::Numeric(NumericFunction::ModReal),
            TypeName::from("ANY_REAL"),
            vec![
                input_param("IN1", "ANY_REAL"),
                input_param("IN2", "ANY_REAL"),
            ],
        ),
    ]
}

// =============================================================================
// SIZEOF (Language Extension)
// =============================================================================

/// Returns the SIZEOF function definition.
///
/// SIZEOF returns the size in bytes of a variable or type. It is not part of
/// IEC 61131-3 but is a CODESYS/TwinCAT/RuSTy extension used in buffer
/// management functions. It is registered conditionally based on the
/// `allow_sizeof` compiler option.
pub fn get_sizeof_function() -> FunctionSignature {
    FunctionSignature::stdlib(
        "SIZEOF",
        BuiltinFunction::Sizeof,
        TypeName::from("ANY_INT"),
        vec![input_param("IN", "ANY")],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin_function::TimeFunction;
    use crate::intermediates::operator_function_form::FormOf;
    use ironplc_dsl::common::{ElementaryTypeName, FunctionReturnType};
    use ironplc_dsl::textual::Operator;
    use rstest::rstest;

    #[test]
    fn get_numeric_functions_when_called_then_contains_all_functions() {
        let functions = get_numeric_functions();

        assert_eq!(functions.len(), 17);

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
        assert!(functions.iter().any(|f| f.name.original() == "ATAN2"));
        assert!(functions.iter().any(|f| f.name.original() == "EXPT"));
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
    fn stdlib_parameter_type_when_generic_category_then_generic() {
        assert_eq!(
            stdlib_parameter_type("ANY_NUM"),
            ParameterType::Generic(GenericTypeName::AnyNum)
        );
    }

    #[test]
    fn stdlib_parameter_type_when_elementary_then_type_id_of_elementary() {
        assert_eq!(
            stdlib_parameter_type("TIME_OF_DAY"),
            ParameterType::Type(type_id::elementary(&ElementaryTypeName::TimeOfDay))
        );
    }

    #[test]
    fn stdlib_parameter_type_when_unknown_name_then_unresolved() {
        assert_eq!(
            stdlib_parameter_type("NOPE"),
            ParameterType::Unresolved(TypeName::from("NOPE"))
        );
    }

    #[test]
    fn stdlib_functions_when_any_parameter_then_type_or_generic_category() {
        let functions = get_all_stdlib_functions()
            .into_iter()
            .chain(get_compiler_intrinsic_functions());
        for function in functions {
            for parameter in &function.parameters {
                assert!(
                    !matches!(parameter.param_type, ParameterType::Unresolved(_)),
                    "{}.{} does not resolve",
                    function.name,
                    parameter.name
                );
            }
        }
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

    #[test]
    fn get_compiler_intrinsic_functions_when_called_then_any_real_signatures() {
        let functions = get_compiler_intrinsic_functions();

        let trunc = functions
            .iter()
            .find(|f| f.name.original() == "__TRUNC")
            .unwrap();
        assert!(trunc.is_stdlib());
        assert_eq!(trunc.input_parameter_count(), 1);
        assert_eq!(
            trunc.parameters[0].param_type,
            ParameterType::Generic(GenericTypeName::AnyReal)
        );
        assert_eq!(
            trunc.return_type,
            Some(FunctionReturnType::Named(TypeName::from("ANY_REAL")))
        );

        let fmod = functions
            .iter()
            .find(|f| f.name.original() == "__MOD")
            .unwrap();
        assert!(fmod.is_stdlib());
        assert_eq!(fmod.input_parameter_count(), 2);
        assert_eq!(
            fmod.parameters[0].param_type,
            ParameterType::Generic(GenericTypeName::AnyReal)
        );
        assert_eq!(
            fmod.parameters[1].param_type,
            ParameterType::Generic(GenericTypeName::AnyReal)
        );
        assert_eq!(
            fmod.return_type,
            Some(FunctionReturnType::Named(TypeName::from("ANY_REAL")))
        );
    }

    #[test]
    fn get_all_stdlib_functions_when_called_then_includes_compiler_intrinsics() {
        let functions = get_all_stdlib_functions();
        assert!(functions.iter().any(|f| f.name.original() == "__TRUNC"));
        assert!(functions.iter().any(|f| f.name.original() == "__MOD"));
    }

    /// Every family of standard function names its built-in function, so codegen
    /// never has to recognize a function by its spelling.
    #[rstest]
    #[case::numeric("ABS", BuiltinFunction::Numeric(NumericFunction::Abs))]
    #[case::compiler_intrinsic("__MOD", BuiltinFunction::Numeric(NumericFunction::ModReal))]
    #[case::bit_shift("ROR", BuiltinFunction::BitShift(BitShift::Ror))]
    #[case::string("CONCAT", BuiltinFunction::String(StringFunction::Concat))]
    #[case::selection("MUX", BuiltinFunction::Mux)]
    #[case::bcd("INT_TO_BCD", BuiltinFunction::IntToBcd)]
    #[case::operator_form("ADD", BuiltinFunction::Operator(FormOf::Arithmetic(Operator::Add)))]
    #[case::conversion(
        "INT_TO_REAL",
        BuiltinFunction::Conversion {
            source: ElementaryTypeName::INT,
            target: ElementaryTypeName::REAL,
        }
    )]
    #[case::time(
        "SUB_LTIME",
        BuiltinFunction::Time { function: TimeFunction::SubTime, long: true }
    )]
    fn get_all_stdlib_functions_when_registered_then_names_its_builtin_function(
        #[case] name: &str,
        #[case] builtin: BuiltinFunction,
    ) {
        let sig = get_all_stdlib_functions()
            .into_iter()
            .find(|f| f.name == Id::from(name));

        assert_eq!(sig.and_then(|sig| sig.builtin_function), Some(builtin));
    }

    #[test]
    fn get_sizeof_function_when_called_then_names_sizeof() {
        assert_eq!(
            get_sizeof_function().builtin_function,
            Some(BuiltinFunction::Sizeof)
        );
    }

    #[test]
    fn get_all_stdlib_functions_when_called_then_registers_every_operator_form() {
        let names: Vec<Id> = get_all_stdlib_functions()
            .into_iter()
            .map(|f| f.name)
            .collect();
        for name in ["ADD", "GT", "AND", "NOT"] {
            assert!(names.contains(&Id::from(name)), "{name} missing");
        }
    }
}
