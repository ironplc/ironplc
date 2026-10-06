//! The standard type conversion functions (IEC 61131-3 Section 2.5.1.5):
//! `<SOURCE>_TO_<TARGET>` between the elementary types, including to and
//! from `STRING`.
//!
//! Each conversion takes one input of its source type, returns its target
//! type, and stands for [`Intrinsic::Conversion`] of the two. The tables
//! below give each type with the spelling function names use for it, so the
//! intrinsic's types are stated here rather than parsed back out of a name.

use ironplc_dsl::common::{ElementaryTypeName, TypeName};

use super::stdlib_function::input_param;
use crate::function_environment::FunctionSignature;
use crate::intrinsic::Intrinsic;

/// A type a conversion function converts from or to: how the function's
/// name spells it, and the type it is.
///
/// A time or date type is spelled both by its full name and by its short
/// alias (`TIME_OF_DAY` and `TOD`), and each spelling names a function of
/// its own.
type Spelling = (&'static str, ElementaryTypeName);

/// Returns every standard type conversion function.
pub(super) fn get_conversion_functions() -> Vec<FunctionSignature> {
    let mut functions = Vec::new();

    functions.extend(get_int_to_int_conversions());
    functions.extend(get_int_to_real_conversions());
    functions.extend(get_real_to_int_conversions());
    functions.extend(get_real_to_real_conversions());
    functions.extend(get_bool_to_int_conversions());
    functions.extend(get_int_to_bool_conversions());

    functions.extend(get_bit_string_to_bit_string_conversions());
    functions.extend(get_bit_string_to_int_conversions());
    functions.extend(get_int_to_bit_string_conversions());
    functions.extend(get_bool_bit_string_conversions());
    functions.extend(get_bit_string_real_conversions());

    functions.extend(get_time_date_conversions());

    functions.extend(get_string_conversion_functions());

    functions
}

/// Creates a type conversion function signature.
///
/// Type conversion functions follow the naming convention `<SOURCE>_TO_<TARGET>`
/// and take a single input parameter of the source type, returning the target type.
fn build_conversion_function(source: &Spelling, target: &Spelling) -> FunctionSignature {
    let (source_name, source_type) = source;
    let (target_name, target_type) = target;
    FunctionSignature::stdlib(
        &format!("{source_name}_TO_{target_name}"),
        Intrinsic::Conversion {
            source: source_type.clone(),
            target: target_type.clone(),
        },
        TypeName::from(target_name),
        vec![input_param("IN", source_name)],
    )
}

const BOOL: Spelling = ("BOOL", ElementaryTypeName::BOOL);

const REAL: Spelling = ("REAL", ElementaryTypeName::REAL);

const STRING: Spelling = ("STRING", ElementaryTypeName::STRING);

/// Signed integer types for conversion functions.
const SIGNED_INT_TYPES: &[Spelling] = &[
    ("SINT", ElementaryTypeName::SINT),
    ("INT", ElementaryTypeName::INT),
    ("DINT", ElementaryTypeName::DINT),
    ("LINT", ElementaryTypeName::LINT),
];

/// Unsigned integer types for conversion functions.
const UNSIGNED_INT_TYPES: &[Spelling] = &[
    ("USINT", ElementaryTypeName::USINT),
    ("UINT", ElementaryTypeName::UINT),
    ("UDINT", ElementaryTypeName::UDINT),
    ("ULINT", ElementaryTypeName::ULINT),
];

/// Real (floating-point) types for conversion functions.
const REAL_TYPES: &[Spelling] = &[REAL, ("LREAL", ElementaryTypeName::LREAL)];

/// All integer types (signed + unsigned) for BOOL conversion targets.
const ALL_INT_TYPES: &[Spelling] = &[
    ("SINT", ElementaryTypeName::SINT),
    ("INT", ElementaryTypeName::INT),
    ("DINT", ElementaryTypeName::DINT),
    ("LINT", ElementaryTypeName::LINT),
    ("USINT", ElementaryTypeName::USINT),
    ("UINT", ElementaryTypeName::UINT),
    ("UDINT", ElementaryTypeName::UDINT),
    ("ULINT", ElementaryTypeName::ULINT),
];

/// Bit string types (excluding BOOL) for conversion functions.
const BIT_STRING_TYPES: &[Spelling] = &[
    ("BYTE", ElementaryTypeName::BYTE),
    ("WORD", ElementaryTypeName::WORD),
    ("DWORD", ElementaryTypeName::DWORD),
    ("LWORD", ElementaryTypeName::LWORD),
];

/// All time and date types for conversion functions, under every spelling.
///
/// Includes both canonical names (TIME_OF_DAY, DATE_AND_TIME) and their
/// short aliases (TOD, DT) since function lookup is by exact name.
const ALL_TIME_DATE_TYPES: &[Spelling] = &[
    ("TIME", ElementaryTypeName::TIME),
    ("LTIME", ElementaryTypeName::LTIME),
    ("DATE", ElementaryTypeName::DATE),
    ("LDATE", ElementaryTypeName::LDATE),
    ("TOD", ElementaryTypeName::TimeOfDay),
    ("TIME_OF_DAY", ElementaryTypeName::TimeOfDay),
    ("LTOD", ElementaryTypeName::LTimeOfDay),
    ("LTIME_OF_DAY", ElementaryTypeName::LTimeOfDay),
    ("DT", ElementaryTypeName::DateAndTime),
    ("DATE_AND_TIME", ElementaryTypeName::DateAndTime),
    ("LDT", ElementaryTypeName::LDateAndTime),
    ("LDATE_AND_TIME", ElementaryTypeName::LDateAndTime),
];

/// Integer, real, and bit string types that time/date types convert to/from.
const TIME_DATE_TARGETS: &[Spelling] = &[
    ("SINT", ElementaryTypeName::SINT),
    ("INT", ElementaryTypeName::INT),
    ("DINT", ElementaryTypeName::DINT),
    ("LINT", ElementaryTypeName::LINT),
    ("USINT", ElementaryTypeName::USINT),
    ("UINT", ElementaryTypeName::UINT),
    ("UDINT", ElementaryTypeName::UDINT),
    ("ULINT", ElementaryTypeName::ULINT),
    REAL,
    ("LREAL", ElementaryTypeName::LREAL),
    ("BYTE", ElementaryTypeName::BYTE),
    ("WORD", ElementaryTypeName::WORD),
    ("DWORD", ElementaryTypeName::DWORD),
    ("LWORD", ElementaryTypeName::LWORD),
];

/// Numeric types that convert to STRING (W32 signed).
const SIGNED_INT_TO_STRING_TYPES: &[Spelling] = &[
    ("SINT", ElementaryTypeName::SINT),
    ("INT", ElementaryTypeName::INT),
    ("DINT", ElementaryTypeName::DINT),
];

/// Numeric types that convert to STRING (W32 unsigned / bit-string).
const UNSIGNED_INT_TO_STRING_TYPES: &[Spelling] = &[
    ("USINT", ElementaryTypeName::USINT),
    ("UINT", ElementaryTypeName::UINT),
    ("UDINT", ElementaryTypeName::UDINT),
    ("BYTE", ElementaryTypeName::BYTE),
    ("WORD", ElementaryTypeName::WORD),
    ("DWORD", ElementaryTypeName::DWORD),
];

/// Integer and bit-string types that can be parsed from STRING. A bit-string
/// target converts as the unsigned integer of its width.
const STRING_TO_INT_TYPES: &[Spelling] = &[
    ("SINT", ElementaryTypeName::SINT),
    ("INT", ElementaryTypeName::INT),
    ("DINT", ElementaryTypeName::DINT),
    ("LINT", ElementaryTypeName::LINT),
    ("USINT", ElementaryTypeName::USINT),
    ("UINT", ElementaryTypeName::UINT),
    ("UDINT", ElementaryTypeName::UDINT),
    ("ULINT", ElementaryTypeName::ULINT),
    ("BYTE", ElementaryTypeName::BYTE),
    ("WORD", ElementaryTypeName::WORD),
    ("DWORD", ElementaryTypeName::DWORD),
    ("LWORD", ElementaryTypeName::LWORD),
];

/// Generates all integer-to-integer conversion functions.
///
/// Creates functions like INT_TO_DINT, DINT_TO_INT, SINT_TO_LINT, etc.
fn get_int_to_int_conversions() -> Vec<FunctionSignature> {
    let mut functions = Vec::new();

    // All signed integer types
    for source in SIGNED_INT_TYPES {
        for target in SIGNED_INT_TYPES {
            if source != target {
                functions.push(build_conversion_function(source, target));
            }
        }
    }

    // All unsigned integer types
    for source in UNSIGNED_INT_TYPES {
        for target in UNSIGNED_INT_TYPES {
            if source != target {
                functions.push(build_conversion_function(source, target));
            }
        }
    }

    // Signed to unsigned conversions
    for source in SIGNED_INT_TYPES {
        for target in UNSIGNED_INT_TYPES {
            functions.push(build_conversion_function(source, target));
        }
    }

    // Unsigned to signed conversions
    for source in UNSIGNED_INT_TYPES {
        for target in SIGNED_INT_TYPES {
            functions.push(build_conversion_function(source, target));
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
    for source in SIGNED_INT_TYPES {
        for target in REAL_TYPES {
            functions.push(build_conversion_function(source, target));
        }
    }

    // Unsigned integer to real
    for source in UNSIGNED_INT_TYPES {
        for target in REAL_TYPES {
            functions.push(build_conversion_function(source, target));
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
    for source in REAL_TYPES {
        for target in SIGNED_INT_TYPES {
            functions.push(build_conversion_function(source, target));
        }
    }

    // Real to unsigned integer
    for source in REAL_TYPES {
        for target in UNSIGNED_INT_TYPES {
            functions.push(build_conversion_function(source, target));
        }
    }

    functions
}

/// Generates all real-to-real conversion functions.
///
/// Creates functions like REAL_TO_LREAL, LREAL_TO_REAL.
fn get_real_to_real_conversions() -> Vec<FunctionSignature> {
    let mut functions = Vec::new();

    for source in REAL_TYPES {
        for target in REAL_TYPES {
            if source != target {
                functions.push(build_conversion_function(source, target));
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
        .map(|target| build_conversion_function(&BOOL, target))
        .collect()
}

/// Generates integer-to-BOOL conversion functions.
///
/// Creates functions like SINT_TO_BOOL, INT_TO_BOOL, DINT_TO_BOOL, etc.
/// 0 converts to FALSE, any non-zero value converts to TRUE.
fn get_int_to_bool_conversions() -> Vec<FunctionSignature> {
    ALL_INT_TYPES
        .iter()
        .map(|source| build_conversion_function(source, &BOOL))
        .collect()
}

/// Generates bit-string-to-bit-string conversion functions.
///
/// Creates functions like BYTE_TO_WORD, WORD_TO_DWORD, DWORD_TO_LWORD, etc.
fn get_bit_string_to_bit_string_conversions() -> Vec<FunctionSignature> {
    let mut functions = Vec::new();

    for source in BIT_STRING_TYPES {
        for target in BIT_STRING_TYPES {
            if source != target {
                functions.push(build_conversion_function(source, target));
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

    for source in BIT_STRING_TYPES {
        for target in ALL_INT_TYPES {
            functions.push(build_conversion_function(source, target));
        }
    }

    functions
}

/// Generates integer-to-bit-string conversion functions.
///
/// Creates functions like INT_TO_BYTE, DINT_TO_WORD, LINT_TO_DWORD, etc.
fn get_int_to_bit_string_conversions() -> Vec<FunctionSignature> {
    let mut functions = Vec::new();

    for source in ALL_INT_TYPES {
        for target in BIT_STRING_TYPES {
            functions.push(build_conversion_function(source, target));
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
        functions.push(build_conversion_function(&BOOL, bit_type));
        functions.push(build_conversion_function(bit_type, &BOOL));
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

/// Generates time/date type conversion functions.
///
/// Creates bidirectional conversions between all time/date types (TIME, DATE,
/// TOD, DT and their long and alias forms) and integer, real, and bit string
/// types. For example: TIME_TO_DWORD, DWORD_TO_TIME, DATE_TO_UDINT, etc.
fn get_time_date_conversions() -> Vec<FunctionSignature> {
    let mut functions = Vec::new();

    for time_type in ALL_TIME_DATE_TYPES {
        for target_type in TIME_DATE_TARGETS {
            functions.push(build_conversion_function(time_type, target_type));
            functions.push(build_conversion_function(target_type, time_type));
        }
    }

    functions
}

/// Returns string ↔ numeric conversion function definitions.
///
/// Covers all W32 numeric types: signed integers (SINT, INT, DINT),
/// unsigned integers and bit-strings (USINT, UINT, UDINT, BYTE, WORD, DWORD),
/// and REAL. Each type gets a *_TO_STRING function, and a subset gets
/// STRING_TO_* functions.
fn get_string_conversion_functions() -> Vec<FunctionSignature> {
    let mut functions = Vec::new();

    // Signed integer → STRING
    for source in SIGNED_INT_TO_STRING_TYPES {
        functions.push(build_conversion_function(source, &STRING));
    }

    // Unsigned integer / bit-string → STRING
    for source in UNSIGNED_INT_TO_STRING_TYPES {
        functions.push(build_conversion_function(source, &STRING));
    }

    // REAL → STRING
    functions.push(build_conversion_function(&REAL, &STRING));

    // STRING → integer and bit-string types
    for target in STRING_TO_INT_TYPES {
        functions.push(build_conversion_function(&STRING, target));
    }

    // STRING → real types
    for target in REAL_TYPES {
        functions.push(build_conversion_function(&STRING, target));
    }

    functions
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::common::FunctionReturnType;
    use rstest::rstest;

    #[rstest]
    #[case::sint("STRING_TO_SINT", "SINT")]
    #[case::int("STRING_TO_INT", "INT")]
    #[case::dint("STRING_TO_DINT", "DINT")]
    #[case::usint("STRING_TO_USINT", "USINT")]
    #[case::uint("STRING_TO_UINT", "UINT")]
    #[case::udint("STRING_TO_UDINT", "UDINT")]
    #[case::byte("STRING_TO_BYTE", "BYTE")]
    #[case::word("STRING_TO_WORD", "WORD")]
    #[case::dword("STRING_TO_DWORD", "DWORD")]
    #[case::lint("STRING_TO_LINT", "LINT")]
    #[case::ulint("STRING_TO_ULINT", "ULINT")]
    #[case::lword("STRING_TO_LWORD", "LWORD")]
    #[case::real("STRING_TO_REAL", "REAL")]
    #[case::lreal("STRING_TO_LREAL", "LREAL")]
    fn get_string_conversion_functions_when_string_to_integer_then_registered_with_target_return(
        #[case] name: &str,
        #[case] target: &str,
    ) {
        let functions = get_string_conversion_functions();
        let sig = functions
            .iter()
            .find(|f| f.name.original() == name)
            .unwrap();
        assert_eq!(sig.parameters.len(), 1);
        assert_eq!(sig.parameters[0].param_type, TypeName::from("STRING"));
        assert_eq!(
            sig.return_type,
            Some(FunctionReturnType::Named(TypeName::from(target)))
        );
    }

    #[test]
    fn build_conversion_function_when_called_then_has_correct_signature() {
        let sig = build_conversion_function(&("INT", ElementaryTypeName::INT), &REAL);

        assert_eq!(sig.name.original(), "INT_TO_REAL");
        assert!(sig.is_stdlib());
        assert_eq!(sig.parameters.len(), 1);
        assert_eq!(sig.parameters[0].name.original(), "IN");
        assert!(sig.parameters[0].is_input);
        // Parameter type is now TypeName, not SemanticType
        assert_eq!(sig.parameters[0].param_type, TypeName::from("INT"));
        // Return type is now TypeName, not SemanticType
        assert_eq!(
            sig.return_type,
            Some(FunctionReturnType::Named(TypeName::from("REAL")))
        );
        assert_eq!(
            sig.intrinsic,
            Some(Intrinsic::Conversion {
                source: ElementaryTypeName::INT,
                target: ElementaryTypeName::REAL,
            })
        );
    }

    /// A conversion names its types with the elementary types they spell,
    /// so both spellings of a time or date type stand for the same one.
    #[rstest]
    #[case::alias("TOD_TO_DINT", ElementaryTypeName::TimeOfDay, ElementaryTypeName::DINT)]
    #[case::full_name(
        "TIME_OF_DAY_TO_DINT",
        ElementaryTypeName::TimeOfDay,
        ElementaryTypeName::DINT
    )]
    #[case::to_alias(
        "LWORD_TO_LDT",
        ElementaryTypeName::LWORD,
        ElementaryTypeName::LDateAndTime
    )]
    #[case::to_string("BYTE_TO_STRING", ElementaryTypeName::BYTE, ElementaryTypeName::STRING)]
    #[case::from_string(
        "STRING_TO_LREAL",
        ElementaryTypeName::STRING,
        ElementaryTypeName::LREAL
    )]
    #[case::from_bool("BOOL_TO_WORD", ElementaryTypeName::BOOL, ElementaryTypeName::WORD)]
    fn get_conversion_functions_when_conversion_then_intrinsic_names_its_types(
        #[case] name: &str,
        #[case] source: ElementaryTypeName,
        #[case] target: ElementaryTypeName,
    ) {
        let functions = get_conversion_functions();
        let sig = functions.iter().find(|f| f.name.original() == name);

        assert_eq!(
            sig.and_then(|sig| sig.intrinsic.clone()),
            Some(Intrinsic::Conversion { source, target })
        );
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
        assert_eq!(
            bool_to_int.return_type,
            Some(FunctionReturnType::Named(TypeName::from("INT")))
        );
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
        assert_eq!(
            int_to_bool.return_type,
            Some(FunctionReturnType::Named(TypeName::from("BOOL")))
        );
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
        assert_eq!(
            byte_to_word.return_type,
            Some(FunctionReturnType::Named(TypeName::from("WORD")))
        );
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
}
